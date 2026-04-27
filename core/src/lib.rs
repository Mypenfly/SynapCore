use std::{collections::HashMap, fmt::Display, io::Write, path::PathBuf};

use crate::{
    assistant::{Assistant, Description},
    config::CoreConfig,
    conversation::{Conversation, TempData},
    read_config::JsonConfig,
    request_body::{LLMClient, Usage, messenge::Messenge, response::LLMResponse},
};

use tools::{Tools, define_call::tool_call::ToolCall};

mod assistant;
mod config;
mod conversation;
pub mod error;
pub use error::{CoreErr, CoreResult};
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
    ToolPreparing {
        character: String,
        name: String,
    },
    ToolCalls {
        raw_content: String,
        character: String,
        tools: Vec<ToolCall>,
        is_save: bool,
    },
    Store {
        character: String,
        raw_content: String,
    },
    Usage(Usage),
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
        message: &UserMessage,
    ) -> CoreResult<tokio::sync::mpsc::Receiver<BotResponse>> {
        self.temp_data.text = format!(
            "(这是一个关键的长期任务，建议按照任务类型优先读取有用skill并严格参考),一下是任务要求:\n{}\n推荐使用工具todo_list,skills_book",
            message.text
        );
        self.temp_data.files = message.files.clone();

        let leader = self.config.agent.leader.character.clone();

        let (event_tx, event_rx) = tokio::sync::mpsc::channel::<CoreEvent>(1024);
        let (out_tx, out_rx) = tokio::sync::mpsc::channel::<BotResponse>(1024);

        self.events_tx = Some(event_tx);

        let mut bot = self.get_bot(&leader, message.enable_tools)?;
        self.bot(&mut bot, message.is_save).await?;

        let core = self.clone();
        tokio::spawn(async move {
            // let mut core = Core::init().unwrap();
            // core.temp_data = data;
            let _ = core.event_loop(event_rx, out_tx, bot).await;
        });

        Ok(out_rx)
    }
    ///一般交流
    pub async fn chat(
        &mut self,
        character: &str,
        message: &UserMessage,
    ) -> CoreResult<tokio::sync::mpsc::Receiver<BotResponse>> {
        self.temp_data.text = message.text.to_string();
        self.temp_data.files = message.files.clone();

        let (event_tx, event_rx) = tokio::sync::mpsc::channel::<CoreEvent>(1024);
        let (out_tx, out_rx) = tokio::sync::mpsc::channel::<BotResponse>(1024);

        self.events_tx = Some(event_tx);

        let mut bot = self.get_bot(character, message.enable_tools)?;
        self.bot(&mut bot, message.is_save).await?;

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
            let mut full_content = String::new();
            while let Ok(content) = rx.recv().await {
                // full_content.push_str(&content);
                match content {
                    LLMResponse::Nothing => break,
                    LLMResponse::Reasoning { chunk } => {
                        let _ = event_sender.send(CoreEvent::Reasoning { chunk }).await;
                    }
                    LLMResponse::Content { chunk } => {
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
                    LLMResponse::ToolCall { tools } => {
                        let _ = event_sender
                            .send(CoreEvent::ToolCalls {
                                raw_content: full_content.clone(),
                                character: event_ch.clone(),
                                tools,
                                is_save,
                            })
                            .await;
                        is_complete = true;
                    }
                    LLMResponse::ToolPreparing { name } => {
                        let _ = event_sender
                            .send(CoreEvent::ToolPreparing {
                                character: character.clone(),
                                name,
                            })
                            .await;
                    }
                    LLMResponse::TokensUsage { usage } => {
                        let _ = event_sender.send(CoreEvent::Usage(usage)).await;
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

        //工具激活是脱离工具启用判断的
        let tools =
            Tools::init(&self.config.normal.sc_root, character).map_err(CoreErr::ToolError)?;
        self.tool = tools;

        if enable_tools {
            // bot.llm.postbody.tools = Some(self.tool);
            bot.llm.postbody.tools = Some(self.tool.get_active());
            self.temp_data
                .text
                .push_str("\n（system hint :用户已经开放了工具调用权限，请合理使用）\n");
            // println!("\ntools:{:#?}\n",&bot.llm.postbody.tools)
        } else {
            self.temp_data
                .text
                .push_str("\n（system hint :请不要尝试调用工具，用户没有开放工具权限）\n");
        }
        //reflection注入
        let reflection = self.read_reflection(character)?;
        if !reflection.is_empty() {
            bot.reflection_into(format!("Reflection Content/Action Guide:\n{}", reflection));
        }

        //skills注入
        if is_leader {
            let skills = self.tool.get_skills_list();
            bot.skills_list_into(skills);
        }

        //笔记注入
        let note = self.tool.get_last_note();
        // println!("Note:{}", &note);
        bot.note_into(note);

        Ok(bot)
    }

    ///核心事件循环
    async fn event_loop(
        mut self,
        mut event_rx: tokio::sync::mpsc::Receiver<CoreEvent>,
        out_tx: tokio::sync::mpsc::Sender<BotResponse>,
        mut bot: Assistant,
    ) -> CoreResult<()> {
        while let Some(event) = event_rx.recv().await {
            match event {
                CoreEvent::Streaming { chunk } => {
                    let _ = out_tx.send(BotResponse::Content { chunk }).await;
                }
                CoreEvent::Reasoning { chunk } => {
                    let _ = out_tx.send(BotResponse::Reasoning { chunk }).await;
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

                        let _ = out_tx.send(BotResponse::Save { character }).await;
                    } else if !is_save && bot.stop_ok {
                        break;
                    }
                }
                CoreEvent::ToolPreparing { character, name } => {
                    let _ = out_tx
                        .send(BotResponse::ToolPreparing {
                            charater: character,
                            name,
                        })
                        .await;
                }
                CoreEvent::ToolCalls {
                    raw_content,
                    character,
                    tools,
                    is_save,
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
                        let args_raw = tool.function.arguments.clone().unwrap_or_default();
                        let max_len = 120;
                        let arguments = if args_raw.len() >= max_len {
                            let suffix = "......";
                            let available_chars = max_len.saturating_sub(suffix.chars().count());

                            let content: String = args_raw.chars().take(available_chars).collect();

                            format!("{} {}", content, suffix)
                        } else {
                            args_raw
                        };

                        let _ = out_tx
                            .send(BotResponse::ToolCall {
                                character: character.clone(),
                                name,
                                arguments,
                            })
                            .await;

                        self.tool(&mut bot, tool).await?;
                    }
                    self.bot(&mut bot, is_save).await?;
                    //恢复
                    bot.stop_ok = true;
                }
                CoreEvent::Store {
                    character,
                    raw_content,
                } => {
                    let number = self.config.normal.store_num;
                    self.event_mem(&character, &raw_content, number).await?;
                    let _ = out_tx.send(BotResponse::Store { character }).await;
                    bot.stop_ok = true;
                    // break;
                }
                CoreEvent::Usage(usage) => {
                    let _ = out_tx.send(BotResponse::Usage { usage }).await;
                }
                CoreEvent::Error { character, error } => {
                    // let (_, e) = LLMClient::remove_content(&error, "Error");
                    let _ = out_tx.send(BotResponse::Error { character, error }).await;
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

    async fn tool(&mut self, bot: &mut Assistant, tool: ToolCall) -> CoreResult<()> {
        // use tools::tool_response::ToolResponse;
        let response = self.tool.call(tool).await;
        // println!("\ntool-content:{}\n", &content);
        // bot.tool(content);
        if let Ok(content) = response {
            // println!("tool response:{}", &content);
            bot.tool(content.to_string());
        }
        bot.llm.postbody.tools = Some(self.tool.get_active());
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
            self.store_mem(character).await?;
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
    async fn store_mem(&mut self, character: &str) -> CoreResult<()> {
        // println!("\n\n===================STORE test======================");
        let number = self.config.normal.store_num;
        const FORMAT: &str = include_str!("./memory/memoryFormat.md");

        let mut bot = self.get_bot(character, false)?;

        if bot.llm.postbody.session.messenge.len() >= number {
            let mut temp_data = TempData {
                text: format!(
                    "（system command:{}, 总结至今为止的对话,格式要求format:{}）",
                    character, FORMAT
                ),
                files: Vec::new(),
            };

            let prompt_path = self.config.normal.mem_prompt.as_path();
            if prompt_path.exists() {
                let prompt = std::fs::read_to_string(prompt_path)
                    .map_err(|e| CoreErr::InitError(e.to_string()))?;

                temp_data.text = format!(
                    "(system command:You are{},format:{},{})",
                    character, FORMAT, prompt
                );
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
    async fn event_mem(&mut self, character: &str, content: &str, number: usize) -> CoreResult<()> {
        // let number = self.config.normal.store_num;
        let mut bot = self.get_bot(character, false)?;

        // let (content, _) = LLMClient::remove_content(raw_content, "think");
        // 提取记忆标签
        let (_, clean_mems) = LLMClient::remove_content(content, "memory");
        for mem in clean_mems {
            bot.store
                .as_mut()
                .unwrap()
                .store(&mem)
                .await
                .map_err(|e| CoreErr::AssistantError {
                    model: bot.character.clone(),
                    error: e.to_string(),
                })?;
            // println!("{}",mem);
        }
        //填充回对话
        let _ = bot.llm.postbody.session.compression(3, number);
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
    ///注入reflection
    fn read_reflection(&self, character: &str) -> Result<String, CoreErr> {
        use std::fs;
        let path = self
            .config
            .normal
            .sc_root
            .join("data")
            .join(format!("{}_reflection.md", character));
        if path.exists() {
            fs::read_to_string(&path).map_err(|e| CoreErr::InitError(e.to_string()))
        } else {
            Ok(String::new())
        }
    }

    ///退出操作
    pub fn exit(&self) -> Result<(), CoreErr> {
        self.tool
            .exit(&self.config.normal.sc_root)
            .map_err(CoreErr::ToolError)?;
        self.config.save()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum BotResponse {
    Reasoning {
        chunk: String,
    },
    Content {
        chunk: String,
    },
    ToolPreparing {
        charater: String,
        name: String,
    },
    ToolCall {
        character: String,
        name: String,
        arguments: String,
    },
    Save {
        character: String,
    },
    Store {
        character: String,
    },
    Usage {
        usage: Usage,
    },
    Error {
        character: String,
        error: String,
    },
}

impl Display for BotResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Reasoning { chunk } => write!(f, "{}", chunk),
            Self::Content { chunk } => write!(f, "{}", chunk),
            Self::ToolPreparing { charater, name } => {
                writeln!(f, "{} preparing tool : {}", charater, name)
            }
            Self::ToolCall {
                character,
                name,
                arguments,
            } => {
                write!(
                    f,
                    "\n<Tool ch={}>\n{}:\n{}\n</Tool>\n\n",
                    character, name, arguments
                )
            }
            Self::Save { character } => write!(f, "\n{}-Saved\n", character),
            Self::Store { character } => write!(f, "\n{}-Stored\n", character),
            Self::Usage { usage } => writeln!(f, "\ntokens usage :\n {}", usage),
            Self::Error { character, error } => {
                write!(f, "\nError-{} in character:{}\n", error, character)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendMode {
    Task,
    Chat,
}

#[derive(Debug, Clone)]
pub struct UserMessage {
    pub text: String,
    pub files: Vec<String>,
    pub enable_tools: bool,
    pub is_save: bool,
    pub mode: SendMode,
    pub character: String,
}

impl UserMessage {
    pub fn task(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            files: Vec::new(),
            enable_tools: true,
            is_save: true,
            mode: SendMode::Task,
            character: String::new(),
        }
    }

    pub fn chat(character: impl Into<String>) -> Self {
        Self {
            text: String::new(),
            files: Vec::new(),
            enable_tools: false,
            is_save: true,
            mode: SendMode::Chat,
            character: character.into(),
        }
    }
}
