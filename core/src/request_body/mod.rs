use crate::{
    memory::mem::{MemoryErr, MemoryStore},
    read_config::{LLMConfig, Params, Provider},
    request_body::{messenge::Messenge, response::LLMResponse, session::Session},
};

use serde::{Deserialize, Serialize};
use tools::define_call::{
    tool_call::{Function, ToolCall},
    tool_define::ToolDefinition,
};

use futures_util::StreamExt;
use regex::Regex;

mod agent;
pub mod messenge;
mod post;
pub mod session;
// pub mod tool_call;
// pub mod tool_define;
pub mod response;

use post::PostBody;
use reqwest::{Client, Response};
use serde_json::Value;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

///API错误定义
#[derive(Debug)]
pub enum APIErr {
    Network(String),
    Api { code: usize, message: String },
    Streaming(reqwest::Error),
    Json { chunk: String, e: serde_json::Error },
    SendError(tokio::sync::broadcast::error::SendError<LLMResponse>),
    SessionError(String),
    StoreOpenError(MemoryErr),
    FileError(std::io::Error),
}

pub type APIResult<T> = Result<T, APIErr>;

impl std::fmt::Display for APIErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Network(s) => write!(f, "Network falied: {}", s),
            Self::Api { code, message } => write!(f, "API falied : ({}) {}", code, message),
            Self::Streaming(e) => write!(f, "Streaming falied:{}", e),
            Self::Json { chunk, e } => write!(f, "Json failed:{}\n{}", e, chunk),
            Self::SendError(e) => write!(f, "Tokio Sender falied: {}", e),
            Self::SessionError(e) => write!(f, "Session load falied:{}", e),
            Self::StoreOpenError(e) => write!(f, "Memory init failed:{}", e),
            Self::FileError(e) => write!(f, "file system failed:{}", e),
        }
    }
}

impl std::error::Error for APIErr {}

///客户端定义
#[derive(Clone, Debug)]
pub struct LLMClient {
    pub client: Client,

    pub postbody: PostBody,

    pub character: String,
}

impl LLMClient {
    //创建

    pub fn new(
        model: String,
        provider: &Provider,
        session: &Session,
        tools: Option<Vec<ToolDefinition>>,
        params: Params,
    ) -> Self {
        let client = Client::new();
        let postbody = PostBody::build(model, provider, session, tools, params);
        Self {
            client,
            postbody,
            character: "default".to_string(),
        }
    }

    pub fn enable_mem(&mut self, root: &str, config: LLMConfig) -> APIResult<MemoryStore> {
        let root_path = PathBuf::from(root);

        let path = root_path.join(format!("memory/{}.db", self.character));
        if !path.exists() {
            if let Some(parent) = path.parent()
                && !parent.exists()
            {
                std::fs::create_dir_all(parent).map_err(APIErr::FileError)?;
            }
            std::fs::File::create_new(&path).map_err(APIErr::FileError)?;
        }

        let store = MemoryStore::open(path, config).map_err(APIErr::StoreOpenError)?;

        Ok(store)
    }

    pub async fn send(&self, tx: &tokio::sync::broadcast::Sender<LLMResponse>) -> APIResult<()> {
        let url = self.postbody.base_url.clone();
        let api_key = self.postbody.api_key.clone();

        let body = self.rebuild_body();

        // println!("body:{}", serde_json::to_string_pretty(&body).unwrap());

        let response = self
            .client
            .post(format!("{}/chat/completions", url))
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| APIErr::Network(e.to_string()))?;

        let status = response.status();

        if !status.is_success() {
            let text = response
                .text()
                .await
                .map_err(|e| APIErr::Network(e.to_string()))?;
            return Err(APIErr::Api {
                code: status.as_u16() as usize,
                message: text,
            });
        }

        self.stream_out(response, tx).await?;
        Ok(())
    }
    ///重建请求体
    fn rebuild_body(&self) -> serde_json::Map<String, Value> {
        use serde_json::{Map, json};
        let mut body_map = Map::new();
        body_map.insert("model".to_string(), json!(self.postbody.model));
        body_map.insert(
            "messages".to_string(),
            json!(self.postbody.session.format_api()),
        );

        if self.postbody.tools.is_some() {
            body_map.insert("tools".to_string(), json!(self.postbody.tools.as_ref()));
        }

        body_map.insert("stream".to_string(), json!(self.postbody.streaming));
        // body_map.insert("stream".to_string(), json!(false));
        body_map.insert(
            "temperature".to_string(),
            json!(self.postbody.params.temperature),
        );
        body_map.insert(
            "max_tokens".to_string(),
            json!(self.postbody.params.max_tokens),
        );
        body_map.insert("tool_choices".to_string(), json!("auto"));
        body_map.insert("top_p".to_string(), json!(self.postbody.params.top_p));
        body_map.insert(
            "enable_thinking".to_string(),
            json!(self.postbody.params.enable_thinking),
        );
        body_map
    }
    ///流式响应
    async fn stream_out(
        &self,
        reponse: Response,
        tx: &tokio::sync::broadcast::Sender<LLMResponse>,
    ) -> APIResult<()> {
        let mut stream = reponse.bytes_stream();
        // let mut buffer = String::new();

        //留出之后解析用
        let mut content_buf = String::new();
        let mut reasoning_buf = String::new();
        let mut tool_acc: HashMap<usize, ToolCallAcc> = HashMap::new();

        //标记思考
        let mut in_reasoning = false;

        //缓存时长
        let mut last_flush = tokio::time::Instant::now();
        const FLUSH_INTERAL: std::time::Duration = std::time::Duration::from_millis(20);

        while let Some(chunk) = stream.next().await {
            let raw = chunk.map_err(APIErr::Streaming)?;
            let raw_str = String::from_utf8_lossy(&raw);
            // println!(
            //     "\n==========raw==========\n{:#?}\n====================\n",
            //     raw
            // );
            for line in raw_str.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                let json_str = line.strip_prefix("data:").unwrap_or(line);
                // println!(
                //     "\n=======json_str=======\n{}\n=========================\n",
                //     json_str
                // );
                //流式结束
                if json_str.contains("[DONE]") {
                    let _ = tx;
                    // println!("STOP");

                    if in_reasoning {
                        // reasoning_buf.push_str("\n</think,>\n");
                        tx.send(LLMResponse::Reasoning {
                            chunk: reasoning_buf.clone(),
                        })
                        .map_err(APIErr::SendError)?;
                    } else {
                        tx.send(LLMResponse::Content {
                            chunk: content_buf.clone(),
                        })
                        .map_err(APIErr::SendError)?;
                    }
                    continue;
                }

                let chunk: StreamChunk =
                    serde_json::from_str(json_str).map_err(|e| APIErr::Json {
                        chunk: json_str.to_string(),
                        e,
                    })?;

                let choice = match chunk.choices.first() {
                    Some(c) => c,
                    None => continue,
                };

            
                if let Some(delta) = &choice.delta {
                    if let Some(reasoning) = &delta.reasoning_content {
                        // println!("{}",&reasoning);
                        if !in_reasoning {
                            in_reasoning = true;
                            // reasoning_buf.push_str(&format!("\n<think>\n{}", reasoning));

                            tx.send(LLMResponse::Content {
                                chunk: content_buf.clone(),
                            })
                            .map_err(APIErr::SendError)?;

                            content_buf.clear();
                            reasoning_buf.push_str(reasoning);
                        } else {
                            reasoning_buf.push_str(reasoning);
                        }
                    }

                    if let Some(content) = &delta.content {
                        if in_reasoning {
                            in_reasoning = false;

                            content_buf.push_str(content);

                            // reasoning_buf.push_str("\n</think>\n");

                            tx.send(LLMResponse::Reasoning {
                                chunk: reasoning_buf.clone(),
                            })
                            .map_err(APIErr::SendError)?;

                            reasoning_buf.clear();
                        } else {
                            content_buf.push_str(content);
                        }
                    }

                    if let Some(tool_calls) = &delta.tool_calls {
                        for tc in tool_calls {
                            let idx = tc.index;
                            let acc = tool_acc.entry(idx).or_default();

                            if let Some(id) = &tc.id {
                                acc.id = id.clone();
                            }
                            if let Some(name) = &tc.function.name {
                                acc.name = name.clone();
                            }
                            if let Some(args) = &tc.function.arguments {
                                acc.arguments.push_str(args);
                            }
                        }
                    }
                }

                let now = tokio::time::Instant::now();
                if now.duration_since(last_flush) >= FLUSH_INTERAL {
                    last_flush = now;

                    if in_reasoning && !reasoning_buf.is_empty() {
                        tx.send(LLMResponse::Reasoning {
                            chunk: reasoning_buf.clone(),
                        })
                        .map_err(APIErr::SendError)?;
                        reasoning_buf.clear();
                    }

                    if !in_reasoning && !content_buf.is_empty() {
                        tx.send(LLMResponse::Content {
                            chunk: content_buf.clone(),
                        })
                        .map_err(APIErr::SendError)?;

                        content_buf.clear();
                    }
                }

                if let Some(reason) = &choice.finish_reason {
                    match reason.as_str() {
                        "tool_calls" => {
                            let tools: Vec<ToolCall> = tool_acc
                                .iter()
                                .map(|(idx, acc)| ToolCall {
                                    index: *idx,
                                    id: Some(acc.id.clone()),
                                    tool_type: Some("function".to_string()),
                                    function: Function {
                                        name: Some(acc.name.clone()),
                                        arguments: Some(acc.arguments.clone()),
                                    },
                                })
                                .collect();

                            tx.send(LLMResponse::Tool { tools })
                                .map_err(APIErr::SendError)?;
                        }
                        _ => break,
                    }
                }
            }
        }
        Ok(())
    }

    pub fn load_session(&mut self, root: &str) -> APIResult<()> {
        let root_path = PathBuf::from(root);

        let session_path = root_path.join(format!("data/{}.json", self.character));

        confirm_file(&session_path).map_err(|e| APIErr::SessionError(e.to_string()))?;

        let session = Session::load_from_file(session_path.to_str().unwrap_or("default.json"))
            .map_err(|e| APIErr::SessionError(e.to_string()))?;

        //只转移聊天记录
        self.postbody.session.messenge = session.messenge;

        //提示词的热重载？
        self.load_prompt(root);

        Ok(())
    }

    pub fn save_session(&mut self, root: &str) -> APIResult<()> {
        let root_path = PathBuf::from(root);

        let session_path = root_path.join(format!("data/{}.json", self.character));

        confirm_file(&session_path).map_err(|e| APIErr::SessionError(e.to_string()))?;

        self.postbody
            .session
            .save_to_file(session_path.to_str().unwrap_or("default.json"))
            .map_err(|e| APIErr::SessionError(e.to_string()))?;

        Ok(())
    }

    pub fn remove_content(content: &str, tag: &str) -> (String, Vec<String>) {
        let pattern = format!(r"(?s)<{}>(.*?)</{}>", tag, tag);

        let re = match Regex::new(&pattern) {
            Ok(re) => re,
            Err(e) => {
                println!("re falied:{:#?}", e);
                return (String::new(), Vec::new());
            }
        };

        let mut extracted = Vec::new();

        let result = re
            .replace_all(content, |caps: &regex::Captures| {
                if let Some(con) = caps.get(1) {
                    // println!("extracted: {}",&con.as_str().to_string());

                    extracted.push(con.as_str().to_string());
                }
                ""
            })
            .to_string();

        // println!("result: {}",&result);

        (result, extracted)
    }

    //加载提示词
    fn load_prompt(&mut self, root: &str) {
        let root_path = PathBuf::from(root);

        let prompt_path = root_path.join(format!("prompts/{}.md", self.character));

        let content = match std::fs::read_to_string(prompt_path) {
            Ok(con) => con,
            Err(e) => {
                eprintln!("prompt file load failed: {}", e);
                return;
            }
        };

        // println!("\nCONTENT{}\n",&content);

        let messenge = Messenge::system(content);

        if !self.postbody.session.messenge.is_empty() {
            self.postbody.session.messenge[0] = messenge;
        } else {
            self.postbody.session.messenge.push_back(messenge);
        }
    }
}

///确认文件
fn confirm_file(path: &Path) -> APIResult<()> {
    if !path.exists() {
        if let Some(parent) = path.parent()
            && !parent.exists()
        {
            std::fs::create_dir_all(parent).map_err(APIErr::FileError)?;
        }
        std::fs::File::create_new(path).map_err(APIErr::FileError)?;

        let session = Session::default();

        let _ = session.save_to_file(path.to_str().unwrap_or("default.json"));
    }

    Ok(())
}

#[derive(Debug, Default)]
struct ToolCallAcc {
    id: String,
    name: String,
    arguments: String,
}

///定义解析结构
#[derive(Debug, Deserialize, Serialize)]
struct StreamChunk {
    choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize, Serialize)]
struct StreamChoice {
    index: usize,
    delta: Option<Delta>,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Delta {
    content: Option<String>,
    reasoning_content: Option<String>,
    role: Option<String>,
    tool_calls: Option<Vec<ToolCall>>,
}

// mod test{
//     use super::StreamChunk;
//     #[test]
//     fn test() {
//         let json1 = r#"{"id":"019d3437924b80033d9d13a907fff991","object":"chat.completion.chunk","created":1774697550,"model":"Pro/moonshotai/Kimi-K2.5","choices":[{"index":0,"delta":{"content":null,"reasoning_content":null,"role":"assistant","tool_calls":[{"index":0,"id":"functions.files_extract:0","type":"function","function":{"name":"files_extract","arguments":""}}]},"finish_reason":null}],"system_fingerprint":"","usage":{"prompt_tokens":12795,"completion_tokens":46,"total_tokens":12841,"completion_tokens_details":{"reasoning_tokens":19},"prompt_tokens_details":{"cached_tokens":11712},"prompt_cache_hit_tokens":11712,"prompt_cache_miss_tokens":1083}}"#;

// let json2 = r#"{"id":"019d3437924b80033d9d13a907fff991","object":"chat.completion.chunk","created":1774697550,"model":"Pro/moonshotai/Kimi-K2.5","choices":[{"index":0,"delta":{"content":null,"reasoning_content":null,"role":"assistant","tool_calls":[{"index":0,"id":null,"type":null,"function":{"arguments":"{\""}}]},"finish_reason":null}],"system_fingerprint":"","usage":{"prompt_tokens":12795,"completion_tokens":48,"total_tokens":12843,"completion_tokens_details":{"reasoning_tokens":19},"prompt_tokens_details":{"cached_tokens":11712},"prompt_cache_hit_tokens":11712,"prompt_cache_miss_tokens":1083}}"#;

// let chunk1: StreamChunk = serde_json::from_str(json1).unwrap();
// let chunk2: StreamChunk = serde_json::from_str(json2).unwrap();

// println!("chunk1: {:?}", chunk1.choices[0].delta.as_ref().unwrap().tool_calls);
// println!("chunk2: {:?}", chunk2.choices[0].delta.as_ref().unwrap().tool_calls);
//     }
// }
