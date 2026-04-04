use std::{collections::HashMap, io::Write, path::PathBuf};

use crate::{
    assistant::{Assistant, Description},
    config::CoreConfig,
    conversation::{Conversation, TempData},
    error::{CoreErr, CoreResult},
    read_config::JsonConfig,
    request_body::{messenge::Messenge, response::LLMResponse},
};

use tools::define_call::tool_call::ToolCall;

mod assistant;
mod config;
mod conversation;
pub mod error;
mod memory;
mod read_config;
mod request_body;

///核心状态机
#[derive(Debug, Clone)]
enum CoreEvent {
    Streaming {
        chunk: String,
    },
    Reasoning {
        chunk: String,
    },
    Completed {
        character: String,
        content: String,
        is_save: bool,
    },
    Tools {
        raw_content: String,
        character: String,
        tools: Vec<ToolCall>,
    },
    Store {
        character: String,
        raw_content: String,
    },
    Error {
        character: String,
        error: String,
    },
    Finshed,
}

///核心，使用接口
#[derive(Debug, Default, Clone)]
pub struct Core {
    pub config: CoreConfig,
    pub api_json: JsonConfig,
    pub leader_cn: Vec<Conversation>,
    pub sub_cn: HashMap<String, Vec<Conversation>>,
    pub temp_data: TempData,
    pub tool: tools::Tools,
    events_tx: Option<tokio::sync::mpsc::Sender<CoreEvent>>,
}

impl Core {
    pub fn init() -> CoreResult<Self> {
        let config = CoreConfig::init()?;
        let mut api_json = JsonConfig::default();

        if !config.normal.api_path.exists() {
            api_json
                .rewrite_config(&config.normal.api_path)
                .map_err(|e| CoreErr::InitError(format!("Json create failed {}", e)))?;
        } else {
            api_json = JsonConfig::from_file(&config.normal.api_path)
                .map_err(|e| CoreErr::InitError(format!("Json get failed {}", e)))?;
        }
        let leader_cn = Vec::new();
        let sub_cn = HashMap::new();

        let mut core = Core {
            config,
            api_json,
            leader_cn,
            sub_cn,
            temp_data: TempData::default(),
            tool: tools::Tools::default(),
            events_tx: None,
        };

        core.conversation_init()?;

        Ok(core)
    }

    fn conversation_init(&mut self) -> CoreResult<()> {
        let cache = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("synapcore_cache");

        let leader_path = cache.join(format!("{}.jsonl", &self.config.agent.leader.character));

        if Conversation::create_file(&leader_path).is_none() {
            return Err(CoreErr::InitError("对话初始化失败".to_string()));
        }
        self.leader_cn = Conversation::load(&leader_path).unwrap_or_default();

        if let Some(subs) = &self.config.agent.subagents {
            subs.iter().for_each(|sub| {
                let sub_path = cache.join(format!("{}.jsonl", &sub.character));
                let _ = Conversation::create_file(&sub_path);
                let messages = Conversation::load(&sub_path).unwrap_or_default();
                self.sub_cn.insert(sub.character.clone(), messages);
            });
        }

        Ok(())
    }

    ///任务派发
    pub async fn task(
        &mut self,
        text: &str,
        files: Vec<&str>,
        enable_tools: bool,
        is_save: bool,
    ) -> CoreResult<tokio::sync::mpsc::Receiver<String>> {
        self.temp_data.text = text.to_string();
        self.temp_data.files = files.iter().map(PathBuf::from).collect();

        let leader = self.config.agent.leader.character.clone();

        let (event_tx, event_rx) = tokio::sync::mpsc::channel::<CoreEvent>(1024);
        let (out_tx, out_rx) = tokio::sync::mpsc::channel::<String>(1024);

        self.events_tx = Some(event_tx);

        let mut bot = self.get_bot(&leader, enable_tools)?;
        self.bot(&mut bot, is_save).await?;

        let core = self.clone();
        tokio::spawn(async move {
            // let mut core = Core::init().unwrap();
            // core.temp_data = data;
            let _ = core.event_loop(event_rx, out_tx, bot).await;
        });

        Ok(out_rx)
    }

    ///启动核心
    async fn bot(&mut self, bot: &mut Assistant, is_save: bool) -> CoreResult<()> {
        let character = bot.character.clone();

        // let (tx,rx) = tokio::sync::mpsc::channel::<String>(32);

        let (tx, mut rx) = bot.chat(&self.temp_data, Some(self.config.memory)).await?;
        // println!("\ntest,test\n");

        let _ = tx;

        let event_sender = self
            .events_tx
            .clone()
            .ok_or_else(|| CoreErr::AssistantError {
                model: character.to_string(),
                error: "event sender is none".to_string(),
            })?;
        let event_ch = character.to_string();

        tokio::spawn(async move {
            let mut is_complete = false;
            let mut is_reasoning = false;
            let mut full_content = String::new();
            let mut resoning_content = String::new();
            while let Ok(content) = rx.recv().await {
                // full_content.push_str(&content);
                match content {
                    LLMResponse::Nothing => break,
                    LLMResponse::Reasoning { chunk } => {
                        // full_content.push_str(&chunk);
                        if !is_reasoning {
                            let chunk = format!("\n<think>\n{}", chunk);
                            is_reasoning = true;
                            resoning_content.push_str(&chunk);
                        } else {
                            resoning_content.push_str(&chunk);
                        }
                        let _ = event_sender
                            .send(CoreEvent::Reasoning {
                                chunk: resoning_content.clone(),
                            })
                            .await;
                        resoning_content.clear();
                    }
                    LLMResponse::Content { chunk } => {
                        if is_reasoning {
                            is_reasoning = false;

                            resoning_content.push_str("\n</think>\n");

                            let _ = event_sender
                                .send(CoreEvent::Reasoning {
                                    chunk: resoning_content.clone(),
                                })
                                .await;
                            resoning_content.clear();
                        }
                        full_content.push_str(&chunk);
                        let _ = event_sender.send(CoreEvent::Streaming { chunk }).await;
                    }
                    LLMResponse::Error { err } => {
                        let _ = event_sender
                            .send(CoreEvent::Error {
                                character: event_ch.clone(),
                                error: err,
                            })
                            .await;
                    }
                    LLMResponse::Tool { tools } => {
                        let _ = event_sender
                            .send(CoreEvent::Tools {
                                raw_content: full_content.clone(),
                                character: event_ch.clone(),
                                tools,
                            })
                            .await;
                        is_complete = true;
                    }
                }
            }
            if !is_complete {
                let _ = event_sender
                    .send(CoreEvent::Completed {
                        character: event_ch,
                        content: full_content,
                        is_save,
                    })
                    .await;
            }
        });
        Ok(())
    }

    ///获取bot
    fn get_bot(&mut self, character: &str, enable_tools: bool) -> CoreResult<Assistant> {
        let mut is_leader = false;
        let role =
            if self.config.agent.leader.character == character {
                is_leader = true;
                &self.config.agent.leader
            } else {
                let subagents = self.config.agent.subagents.as_ref().ok_or_else(|| {
                    CoreErr::AssistantError {
                        model: character.to_string(),
                        error: "no such character in config".to_string(),
                    }
                })?;
                subagents.iter().find(|s| s.character == character).ok_or(
                    CoreErr::AssistantError {
                        model: character.to_string(),
                        error: "no such character in config".to_string(),
                    },
                )?
            };

        let bot_des = Description {
            path: self.config.normal.sc_root.to_str().unwrap().to_string(),
            provider: role.provider.clone(),
            model: role.agent.clone(),
            tools: None,
        };

        let mut bot = Assistant::new(&self.api_json, &bot_des, &role.character)?;
        bot.is_leader = is_leader;

        if let Some(embed) = &self.config.agent.embed {
            let embed_des = Description {
                path: self.config.normal.sc_root.to_str().unwrap().to_string(),
                provider: embed.provider.clone(),
                model: embed.agent.clone(),
                tools: None,
            };

            bot.open_store(&self.api_json, &embed_des)?;
        }

        if enable_tools {
            let list = self
                .tool
                .init(&self.config.normal.sc_root)
                .map_err(CoreErr::ToolError)?;
            bot.llm.postbody.tools = Some(list);
            // println!("\ntools:{:#?}\n",&bot.llm.postbody.tools)
        }
        Ok(bot)
    }

    ///核心事件循环
    async fn event_loop(
        mut self,
        mut event_rx: tokio::sync::mpsc::Receiver<CoreEvent>,
        out_tx: tokio::sync::mpsc::Sender<String>,
        mut bot: Assistant,
    ) -> CoreResult<()> {
        while let Some(event) = event_rx.recv().await {
            match event {
                CoreEvent::Streaming { chunk } => {
                    let _ = out_tx.send(chunk).await;
                }
                CoreEvent::Reasoning { chunk } => {
                    let _ = out_tx.send(chunk).await;
                }
                CoreEvent::Completed {
                    character,
                    content,
                    is_save,
                } => {
                    if is_save {
                        use crate::request_body::messenge::Messenge;
                        //conversation保存
                        let _ = self.conversation_save(&mut bot, &character, &content);

                        // assistant.state = AssistantState::Finished;
                        // let (content, _) = LLMClient::remove_content(&content, "think");

                        let assitant_message = Messenge::assistant(content.clone());
                        bot.llm.postbody.session.add_messenge(assitant_message);

                        let is_store = self.save(content, &character, &mut bot).await?;

                        if is_store {
                            bot.stop_ok = false;
                        }

                        let _ = out_tx
                            .send(format!("\n<Saved>{}</Saved>\n", &character))
                            .await;
                    } else if !is_save && bot.stop_ok {
                        break;
                    }
                }
                CoreEvent::Tools {
                    raw_content,
                    character,
                    tools,
                } => {
                    //避免被截断
                    bot.stop_ok = false;
                    // let (content, _) = LLMClient::remove_content(&raw_content, "think");
                    // println!("\nRaw_content:{}\n",&raw_content);
                    let mut messenge = Messenge::assistant(raw_content);
                    // println!("Messenge:{:#?}\n",&messenge);
                    messenge.tool_call = Some(tools.clone());
                    bot.llm.postbody.session.add_messenge(messenge);

                    for tool in tools {
                        let name = tool.function.name.clone().unwrap_or("unkown".to_string());
                        let _ = out_tx
                            .send(format!("\n<Tool>({}):{}</Tool>\n", &character, name))
                            .await;

                        self.tool(&mut bot, tool).await?;

                        self.bot(&mut bot, true).await?;
                    }
                    //恢复
                    bot.stop_ok = true;
                }
                CoreEvent::Store {
                    character,
                    raw_content,
                } => {
                    self.event_mem(&character, &raw_content).await?;
                    let _ = out_tx
                        .send(format!("\n<Stored>{}</Stored>\n", &character))
                        .await;
                    bot.stop_ok = true;
                    // break;
                }
                CoreEvent::Error { character, error } => {
                    // let (_, e) = LLMClient::remove_content(&error, "Error");
                    let _ = out_tx
                        .send(format!("EOF:\n({}){}\nEOF", character, error))
                        .await;
                    break;
                }
                CoreEvent::Finshed => {
                    //判断是否 可停
                    if bot.stop_ok {
                        break;
                    }
                }
            }
        }
        Ok(())
    }

    async fn tool(&self, bot: &mut Assistant, tool: ToolCall) -> CoreResult<()> {
        use tools::tool_response::ToolResponse;
        let response = self.tool.call(tool).await;
        // println!("\ntool-content:{}\n", &content);
        // bot.tool(content);
        if let Ok(content) = response {
            match content {
                ToolResponse::Extract(s) => bot.tool(s),
                ToolResponse::Write(s) => bot.tool(s),
                ToolResponse::WebSearch(list) => {
                    let mut content = String::new();
                    list.iter().for_each(|v| {
                        content.push_str(&format!(
                            "
                            id:{}\n
                            url:{}\n
                            title:{}\n
                            snippet:{}\n
                            summary:{}\n
                            site:{}\n
                            publishData:{}\n
                            updataData:{}\n",
                            v.id,
                            v.url,
                            v.name,
                            v.snippet,
                            v.summary.clone().unwrap_or_default(),
                            v.site_name,
                            v.data_last_published,
                            v.data_last_crawled
                        ));
                    });
                    bot.tool(content);
                }
                ToolResponse::Error(s) => bot.tool(s),
            }
        }
        Ok(())
    }

    ///保存核心
    async fn save(
        &mut self,
        content: String,
        character: &str,
        bot: &mut Assistant,
    ) -> CoreResult<bool> {
        //处理思考中调用工具的问题
        if content.starts_with("<think>") {
            return Ok(false);
        }

        let path = bot.path.clone();
        bot.llm
            .save_session(&path)
            .map_err(|e| CoreErr::AssistantError {
                model: character.to_string(),
                error: e.to_string(),
            })?;

        // let core = Core::init()?;

        let mut is_store = false;
        if bot.llm.postbody.session.messenge.len() >= self.config.normal.store_num {
            is_store = true;
            self.store_mem(character, bot).await?;
        } else {
            //发送结束指令
            let sender = self
                .events_tx
                .clone()
                .ok_or_else(|| CoreErr::AssistantError {
                    model: character.to_string(),
                    error: "event sender is none".to_string(),
                })?;
            let _ = sender.send(CoreEvent::Finshed).await;
            // bot.stop_ok = true;
        }

        Ok(is_store)
    }

    ///conversation保存
    fn conversation_save(
        &mut self,
        bot: &mut Assistant,
        character: &str,
        content: &str,
    ) -> Option<()> {
        let cache = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("synapcore_cache");

        let num = self.config.normal.cache_num;

        let conversation = Conversation::append(&self.temp_data, content);

        if bot.is_leader {
            self.leader_cn.push(conversation);
            if self.leader_cn.len() >= num {
                self.leader_cn = self.leader_cn.drain(0..num - 20).collect();
            }

            let leader_path = cache.join(format!("{}.jsonl", &self.config.agent.leader.character));
            let jsonl_content = serde_json::to_string_pretty(&self.leader_cn).ok()?;

            let mut file = std::fs::File::create(leader_path).ok()?;
            file.write_all(jsonl_content.as_bytes()).ok()?;
        } else {
            let list = self.sub_cn.get_mut(character)?;
            list.push(conversation);

            // let mut list = list.clone();
            if list.len() >= num {
                list.drain(0..num - 20);
            }

            let jsonl_content = serde_json::to_string_pretty(&list).ok()?;

            let sub_path = cache.join(format!("{}.jsonl", character));
            let mut file = std::fs::File::create(sub_path).ok()?;
            file.write_all(jsonl_content.as_bytes()).ok()?;
        }
        Some(())
    }

    ///记忆预备处理
    async fn store_mem(&mut self, character: &str, bot: &mut Assistant) -> CoreResult<()> {
        let number = self.config.normal.store_num;

        if bot.llm.postbody.session.messenge.len() >= number {
            let mut temp_data = TempData {
                text: format!("（system command:{}, 总结至今为止的对话）", character),
                files: Vec::new(),
            };

            let prompt_path = self.config.normal.mem_prompt.as_path();
            if prompt_path.exists() {
                let prompt = std::fs::read_to_string(prompt_path)
                    .map_err(|e| CoreErr::InitError(e.to_string()))?;

                temp_data.text = format!("(system command:{},{})", character, prompt);
            }

            //处理一次，避免提示词被覆盖
            bot.llm
                .postbody
                .session
                .add_messenge(Messenge::assistant(String::new()));

            let (tx, mut rx) = bot.chat(&temp_data, Some(self.config.memory)).await?;
            // println!("\ntest,test\n");

            let _ = tx;

            let event_sender = self
                .events_tx
                .clone()
                .ok_or_else(|| CoreErr::AssistantError {
                    model: character.to_string(),
                    error: "event sender is none".to_string(),
                })?;
            let event_ch = character.to_string();

            tokio::spawn(async move {
                let mut raw_content = String::new();
                while let Ok(response) = rx.recv().await {
                    match response {
                        LLMResponse::Nothing => break,
                        LLMResponse::Content { chunk } => {
                            raw_content.push_str(&chunk);
                        }
                        LLMResponse::Error { err } => {
                            let _ = event_sender
                                .send(CoreEvent::Error {
                                    character: event_ch.clone(),
                                    error: err,
                                })
                                .await;
                        }
                        _ => continue,
                    }
                }
                if !raw_content.is_empty() {
                    let _ = event_sender
                        .send(CoreEvent::Store {
                            character: event_ch,
                            raw_content,
                        })
                        .await;
                }
            });
        }
        Ok(())
    }

    ///记忆保存核心
    async fn event_mem(&mut self, character: &str, content: &str) -> CoreResult<()> {
        let number = self.config.normal.store_num;
        let mut bot = self.get_bot(character, false)?;

        // let (content, _) = LLMClient::remove_content(raw_content, "think");

        bot.store
            .as_mut()
            .unwrap()
            .store(content)
            .await
            .map_err(|e| CoreErr::AssistantError {
                model: bot.character.clone(),
                error: e.to_string(),
            })?;

        //填充回对话
        let _ = bot.llm.postbody.session.compression(number);
        let messenge = Messenge::user(format!("(以下是前面部分对话的记忆\n:{})", &content));
        bot.llm.postbody.session.add_messenge(messenge);

        //覆盖
        let path = bot.path.clone();
        bot.llm
            .save_session(&path)
            .map_err(|e| CoreErr::AssistantError {
                model: character.to_string(),
                error: e.to_string(),
            })?;

        let event_sender = self
            .events_tx
            .clone()
            .ok_or_else(|| CoreErr::AssistantError {
                model: character.to_string(),
                error: "event sender is none".to_string(),
            })?;
        let _ = event_sender.send(CoreEvent::Finshed).await;

        Ok(())
    }
}

#[cfg(test)]
mod test {

    use std::io::Write;

    use crate::{Core, conversation::TempData};

    #[tokio::test]
    async fn test() {
        let mut core = match Core::init() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("FALIED:{}", e);
                return;
            }
        };
        core.config.agent.leader.agent = "glm".to_string();
        // let mut core_2 = Core::init().unwrap();

        let mut rx = core
            .task(
                "是精巧，但是颗粒度不够，你有啥建议吗？",
                Vec::new(),
                true,
                true,
            )
            .await
            .unwrap();

        while let Some(content) = rx.recv().await {
            print!("{}", content);
            // std::io::stdout().flush().unwrap();
            std::io::stdout().flush().unwrap();
        }
        core.temp_data = TempData {
            text: "test".to_string(),
            files: Vec::new(),
        };

        // let character = "Yore";
        // let mut bot = core.get_bot(character).unwrap();

        // core.conversation_save(&mut bot, character, "test");

        // println!("succuss:\n{:#?}", &core.leader_cn);
    }
}
