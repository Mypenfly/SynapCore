use std::{collections::HashMap, fmt::Display, fs, path::Path};

use serde::{Deserialize, Serialize};

use base64::{Engine, engine::general_purpose::STANDARD};

use crate::memory::mem::{MemoryConfig, MemoryStore};

use tools::define_call::tool_call::ToolCall;

#[derive(Debug, Default, PartialEq, Clone, Deserialize, Serialize)]
pub struct Messenge {
    pub role: Role,
    pub content: Vec<Content>,
    pub tool_call: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Default)]
pub struct Content {
    #[serde(rename = "type")]
    pub content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    // #[serde(rename = "url")]
    pub image_url: Option<HashMap<String, String>>,
}

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    #[default]
    System,
    User,
    Assistant,
    Tool,
}

impl Messenge {
    pub fn new(role: Role, content: Vec<Content>) -> Self {
        Self {
            role,
            content,
            tool_call: None,
            tool_call_id: None,
        }
    }
    pub fn call_with_tool(mut self, tool: ToolCall) -> Self {
        self.tool_call = Some(vec![tool]);
        self
    }
    pub fn user(txt: String) -> Self {
        let content = Content {
            content_type: "text".to_string(),
            text: Some(txt),
            image_url: None,
        };

        Self::new(Role::User, vec![content])
    }
    pub fn assistant(txt: String) -> Self {
        let content = Content {
            content_type: "text".to_string(),
            text: Some(txt),
            image_url: None,
        };
        Self::new(Role::Assistant, vec![content])
    }
    pub fn system(txt: String) -> Self {
        let content = Content {
            content_type: "text".to_string(),
            text: Some(txt),
            image_url: None,
        };
        Self::new(Role::System, vec![content])
    }
    pub fn tool(id: String, txt: String) -> Self {
        let content = Content {
            content_type: "text".to_string(),
            text: Some(txt),
            image_url: None,
        };
        let mut msg = Self::new(Role::Tool, vec![content]);

        msg.tool_call_id = Some(id);
        msg
    }
    //添加图片
    pub fn add_imge(&mut self, path: &str) -> Result<(), std::io::Error> {
        let bytes = fs::read(path)?;
        let base = STANDARD.encode(&bytes);

        let file = Path::new(path);
        let ext = file.extension().and_then(|s| s.to_str()).unwrap_or("png");

        let url = format!("data:image/{};base64,{}", ext, &base);
        let mut map = HashMap::new();
        map.insert("url".to_string(), url);

        let con = Content {
            content_type: "image_url".to_string(),
            text: None,
            image_url: Some(map),
        };
        self.content.push(con);
        Ok(())
    }

    pub async  fn add_mem(&mut self,store:&MemoryStore,config:&MemoryConfig,text:&str)->Result<(),String> {
        let query =match store.embedding_client.embed(text).await{
            Ok(q) => q,
            Err(e) => return Err(e.to_string())
        };
        let mems =match store.search(&query, config){
            Ok(m) => m,
            Err(e) => return Err(e.to_string())
        };

        if mems.is_empty() {
            return Ok(());
        }
        let mut init_content = String::new() ;

        init_content.push_str("以下是你**记忆**的检索有关或者重要的记忆，请你参考：\n");

        for mem in mems {
            let s = format!("创建时间：{},有关性：{},内容：{}\n",mem.memory.created_time,mem.final_score,mem.memory.content);
            init_content.push_str(&s);
        }

        let content = Content{
            content_type:"text".to_string(),
            text:Some(init_content),
            image_url:None
        };

        self.content.push(content);

        Ok(())
        
        
    }

    ///转化成请求体格式
    pub fn format_api(&self) -> serde_json::Value {
        let mut obj = serde_json::Map::new();

        obj.insert("role".to_string(), serde_json::json!(self.role));
        obj.insert("content".to_string(), serde_json::json!(self.content));

        if let Some(ref tool_calls) = self.tool_call {
            obj.insert("tool_calls".to_string(), serde_json::json!(tool_calls));
        }
        if let Some(ref id) = self.tool_call_id {
            obj.insert("tool_call_id".to_string(), serde_json::json!(id));
        }

        serde_json::Value::Object(obj)
    }
}

impl Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::Tool => write!(f, "tool"),
            Role::Assistant => write!(f, "assistant"),
            Role::System => write!(f, "system"),
            Role::User => write!(f, "user"),
        }
    }
}
