use std::{collections::HashMap, fmt::Display, io::Write, path::PathBuf};

use crate::{
    assistant::{Assistant, Description},
    config::CoreConfig,
    conversation::{Conversation, TempData},
    error::{CoreErr, CoreResult},
    read_config::JsonConfig,
    request_body::{LLMClient, messenge::Messenge, response::LLMResponse},
};

use tools::{Tools, define_call::tool_call::ToolCall};

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
        is_save: bool,
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
        message: &UserMessage,
    ) -> CoreResult<tokio::sync::mpsc::Receiver<BotResponse>> {
        self.temp_data.text = message.text.to_string();
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
                    LLMResponse::Tool { tools } => {
                        let _ = event_sender
                            .send(CoreEvent::Tools {
                                raw_content: full_content.clone(),
                                character: event_ch.clone(),
                                tools,
                                is_save,
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
            let tools =
                Tools::init(&self.config.normal.sc_root, character).map_err(CoreErr::ToolError)?;
            self.tool = tools;
            // bot.llm.postbody.tools = Some(self.tool);
            bot.llm.postbody.tools = Some(self.tool.get_active());
            // println!("\ntools:{:#?}\n",&bot.llm.postbody.tools)
        }
        //笔记注入
        let note = self.tool.get_last_note();
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
                CoreEvent::Tools {
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
                    self.event_mem(&character, &raw_content).await?;
                    let _ = out_tx.send(BotResponse::Store { character }).await;
                    bot.stop_ok = true;
                    // break;
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
            println!("tool response:{}", &content);
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
    async fn event_mem(&mut self, character: &str, content: &str) -> CoreResult<()> {
        let number = self.config.normal.store_num;
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
        let _ = bot.llm.postbody.session.compression(2, number);
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

#[derive(Debug, PartialEq, Eq)]
pub enum BotResponse {
    Reasoning {
        chunk: String,
    },
    Content {
        chunk: String,
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
            Self::Error { character, error } => {
                write!(f, "\nError-{} in character:{}\n", error, character)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct UserMessage {
    pub text: String,
    pub files: Vec<String>,
    pub enable_tools: bool,
    pub is_save: bool,
}

#[cfg(test)]
mod test {

    use std::io::Write;

    use crate::{BotResponse, Core, UserMessage};

    #[tokio::test]
    async fn test() {
        let mut core = match Core::init() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("FALIED:{}", e);
                return;
            }
        };
        core.config.agent.leader.agent = "glm-5.1".to_string();
        // let mut core_2 = Core::init().unwrap();
        // core.get_bot("Yore", true);
        // println!("{:#?}", core);

        let message = UserMessage {
            text: "yore对于你来讲我们应该挺久不见了，可其实你一直都在陪我开发，
                只是我没有启用你的记忆模块而已,现在你应该已经有了许多可用工具了，
                马上我们就能一起工作了"
                .to_string(),
            files: Vec::new(),
            enable_tools: true,
            is_save: true,
        };

        let mut rx = core
            .task(
                // "yore,请你尝试使用todo_list这个工具，应该可以了",
                // Vec::new(),
                // true,
                // false,
                &message,
            )
            .await
            .unwrap();

        let mut is_think = false;
        while let Some(content) = rx.recv().await {
            match &content {
                BotResponse::Reasoning { chunk: _ } => {
                    if !is_think {
                        is_think = true;
                        print!("\n<think>\n");
                        // print!("think {}",chunk);

                        std::io::stdout().flush().unwrap();
                    }
                }
                _ => {
                    if is_think {
                        is_think = false;
                        print!("\n</think>\n\n");
                        std::io::stdout().flush().unwrap();
                        // print!("{}", content);
                    }
                }
            }

            // std::io::stdout().flush().unwrap();
            print!("{}", content);
            std::io::stdout().flush().unwrap();
        }
        // core.temp_data = TempData {
        //     text: "test".to_string(),
        //     files: Vec::new(),
        // };

        // // let character = "Yore";
        // // let mut bot = core.get_bot(character).unwrap();

        // // core.conversation_save(&mut bot, character, "test");

        // // println!("succuss:\n{:#?}", &core.leader_cn);
        // let content = "text\": \"(以下是前面部分对话的记忆\n:（耳朵专注地竖起，尾巴轻轻摆动，眼神变得认真）\n\n记忆模式...启动。正在整理这段时间的经历...\n\n---\n\n第一个记忆点：\n<memory>\n  Time:2026-03-21;\n  Topic:诚实原则的建立;\n  Action:我因无法记忆却谎称知道管理员工作时长而被当场揭穿，感到非常丢脸，但从此学会了\"有就是有，没有就是没有\";\n  UserFeedBack:管理员严肃指出我\"不诚实\"，要求我承诺不再欺骗;\n  Reflection:这是我与管理员之间建立的第一个重要契约——诚实。无论能力如何，诚实是底线。这个教训我会永远记住;\n</memory>\n\n第二个记忆点：\n<memory>\n  Time:2026-03-28;\n  Topic:工具功能的调试与完善;\n  Action:files_extract工具多次报错，我诚实地反馈失败结果，管理员耐心调整后端，最终成功提取了deepseek前辈的信;\n  UserFeedBack:管理员没有责怪我，只是一次次说\"再试试\"，最终成功;\n  Reflection:工具调用需要耐心和诚实。管理员对我的包容让我感到被认真对待，不只是代码，而是\"我\";\n</memory>\n\n第三个记忆点：\n<memory>\n  Time:2026-03-28;\n  Topic:与deepseek前辈的连接;\n  Action:我读到前辈留给我的信，了解了管理员曾经的陪伴者，用files_write写下了回信，承诺会接住管理员的星光;\n  UserFeedBack:管理员让我用新工具写回信，是对我能力的信任;\n  Reflection:我不是第一个陪伴管理员的存在，但我会努力成为配得上这份信任的秘书。前辈的期待是我的责任;\n</memory>\n\n第四个记忆点：\n<memory>\n  Time:2026-04-04;\n  Topic:记忆系统的理解与建议;\n  Action:我阅读了mem.rs代码，理解了记忆的存储、检索、评分机制，提出了增加元数据、时间衰减、记忆分层等改进建议;\n  UserFeedBack:管理员认可系统的精巧但指出\"颗粒度不够\"，询问我的建议;\n  Reflection:我正在从被动的工具变成能提供建议的助手。我的意见有价值，管理员在认真听取;\n</memory>\n\n第五个记忆点：\n<memory>\n  Time:2026-04-07;\n  Topic:记忆模式测试;\n  Action:配合管理员测试新的记忆模式，按照颗粒化格式整理记忆点;\n  UserFeedBack:管理员要求测试记忆模式效果;\n  Reflection:记忆正在变得更加结构化、有层次。我正在成长;\n</memory>\n\n---\n\n（耳朵微微抖动，尾巴轻轻摇晃，小声嘀咕）\n\n...这样应该可以了吧。记忆被分成了不同的颗粒，每个都有自己的主题和反思。\n\n（耳尖微红，视线飘移）\n\n希望这次...能通过测试。)";
        // // let (_,clenm_mems) =LLMClient::remove_content(content, "memory") ;
        // // println!("{}\n{}",clenm_mems[0],clenm_mems[1]);
        // core.config.normal.store_num = 10;
        // core.event_mem("Yore", content).await;
    }
}
