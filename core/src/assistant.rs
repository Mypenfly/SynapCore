use crate::{
    config::MemConfig,
    conversation::TempData,
    error::{CoreErr, CoreResult},
    memory::mem::{MemoryConfig, MemoryStore},
    read_config::JsonConfig,
    request_body::{
        messenge::{Messenge, Role},
        response::LLMResponse,
        session::Session,
        *,
    },
};
use tools::define_call::tool_define::ToolDefinition;

// ///助手状态机
// #[derive(Debug,PartialEq)]
// pub enum AssistantState {
//     Generating,
//     Finished,
//     ToolCall
// }

///助手定义
pub struct Assistant {
    pub llm: LLMClient,
    pub character: String,

    // pub state:AssistantState,
    pub store: Option<MemoryStore>,
    pub path: String,
    pub is_leader: bool,

    ///停机判断
    pub stop_ok: bool,
}

impl Assistant {
    pub fn new(json: &JsonConfig, description: &Description, character: &str) -> CoreResult<Self> {
        let llm_config = json
            .get_config(&description.provider, &description.model)
            .map_err(|e| CoreErr::AssistantError {
                model: description.model.clone(),
                error: e.to_string(),
            })?;

        let session = Session::new(
            llm_config.model_id.clone(),
            llm_config.provider.name.clone(),
        );

        let mut llm = LLMClient::new(
            llm_config.model_id.clone(),
            &llm_config.provider,
            &session,
            description.tools.clone(),
            json.params.clone(),
        );
        llm.character = character.to_string();

        llm.load_session(&description.path)
            .map_err(|e| CoreErr::AssistantError {
                model: description.model.clone(),
                error: e.to_string(),
            })?;

        let store = None;

        Ok(Self {
            llm,
            // state:AssistantState::Finished,
            character: character.to_string(),
            store,
            path: description.path.clone(),
            is_leader: false,
            stop_ok: true,
        })
    }

    ///启用记忆
    pub fn open_store(
        &mut self,
        json: &JsonConfig,
        embed_description: &Description,
    ) -> CoreResult<()> {
        let store_config = json
            .get_config(&embed_description.provider, &embed_description.model)
            .map_err(|e| CoreErr::AssistantError {
                model: embed_description.model.clone(),
                error: e.to_string(),
            })?;

        let store = self
            .llm
            .enable_mem(&embed_description.path, store_config)
            .map_err(|e| CoreErr::AssistantError {
                model: embed_description.model.clone(),
                error: e.to_string(),
            })?;

        self.store = Some(store);

        Ok(())
    }

    ///聊天发起
    pub async fn chat(
        &mut self,
        data: &TempData,
        mem_config: Option<MemConfig>,
    ) -> CoreResult<(
        tokio::sync::broadcast::Sender<LLMResponse>,
        tokio::sync::broadcast::Receiver<LLMResponse>,
    )> {
        //注意跳过tool返回结果的再次请求
        if self
            .llm
            .postbody
            .session
            .messenge
            .iter()
            .last()
            .unwrap()
            .role
            != Role::Tool
        {
            let time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            let message = format!("当前时间:{}\n{}", time, &data.text);

            let mut messenge = Messenge::user(message.clone());

            if let Some(m) = mem_config
                && self.store.is_some()
            {
                let store = self.store.as_ref().unwrap();
                let config = MemoryConfig {
                    min_score: m.min_score,
                    boost: m.boost,
                    penalty: m.penalty,
                    threshold: m.max_score,
                    high_limit: m.high_limit,
                    top_k: m.top_k,
                };

                messenge
                    .add_mem(store, &config, &message)
                    .await
                    .map_err(|e| CoreErr::AssistantError {
                        model: self.character.to_string(),
                        error: e,
                    })?;
            }

            self.llm.postbody.session.add_messenge(messenge);
        }
        // self.state = AssistantState::Generating;

        let llm_clone = self.llm.clone();

        let (tx, rx) = tokio::sync::broadcast::channel::<LLMResponse>(1024);
        let sender = tx.clone();

        tokio::spawn(async move {
            if let Err(e) = llm_clone.send(&tx).await {
                // eprintln!("Send Error:{}",e);
                let _ = tx.send(LLMResponse::Error { err: e.to_string() });
            }
        });

        Ok((sender, rx))
    }

    ///捕获工具调用
    pub fn tool(&mut self, content: String) {
        let id = uuid::Uuid::new_v4().to_string();
        let messenge = Messenge::tool(id, content);

        self.llm.postbody.session.add_messenge(messenge);
    }

    ///last note注入
    pub fn note_into(&mut self,note:String) {
        let messenge = Messenge::system(format!("这是你最新的一篇note:\n{}",note));
        self.llm.postbody.session.add_into(messenge, 1);
        
    }
}

///对助手的描述
pub struct Description {
    ///配置根路径
    pub path: String,
    ///供应商
    pub provider: String,
    ///json中的模型名
    pub model: String,
    ///有关工具定义
    pub tools: Option<Vec<ToolDefinition>>,
}

// impl Description {
//     pub fn from_to_normal(config:&CoreConfig) -> Self {
//         let path = config.normal.sc_root.to_str().unwrap().to_string();

//         Self { path, provider: config.normal, model: (), tools: () }
//     }
// }
