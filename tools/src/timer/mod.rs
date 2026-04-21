mod error;

use std::fmt::Display;
use std::path::PathBuf;

use error::TimerToolErr;
use serde::{Deserialize, Serialize};

use crate::{
    define_call::tool_call::Function,
    define_call::tool_define::{FunctionDefinition, Tool, ToolDefinition},
    tool_response::ToolResponse,
};

#[derive(Debug, Serialize, Deserialize)]
struct Args {
    action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    character: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TimerEntry {
    id: String,
    time: String,
    character: String,
    prompt: String,
    done: bool,
}

impl Display for TimerEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "id:{}\ntime:{}\ncharacter:{}\nprompt:{}\ndone:{}\n\n",
            self.id, self.time, self.character, self.prompt, self.done
        )
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct TimerList {
    timers: Vec<TimerEntry>,
}

impl Display for TimerList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.timers.is_empty() {
            return write!(f, "no timer tasks");
        }
        for t in &self.timers {
            write!(f, "{}", t)?;
        }
        Ok(())
    }
}

pub(crate) struct TimerTool;

impl Tool for TimerTool {
    fn definition(&self) -> ToolDefinition {
        let parameters = serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "操作类型。支持：
                    add（添加定时任务，需传入 time、prompt、character），
                    list（列出所有未完成的定时任务），
                    remove（删除指定任务，需传入 id）"
                },
                "time": {
                    "type": "string",
                    "description": "目标时间，格式: YYYY-MM-DD-HH:mm，如 2026-04-22-09:00。add 时必填"
                },
                "prompt": {
                    "type": "string",
                    "description": "任务触发时发给角色的提示词(即作为user发送给目标character)。add 时必填"
                },
                "character": {
                    "type": "string",
                    "description": "执行任务的角色名,即你当前的身份（如yore）。add 时必填"
                },
                "id": {
                    "type": "string",
                    "description": "任务ID，remove 时必填"
                }
            },
            "required": ["action"]
        });

        let function = FunctionDefinition {
            name: "timer".to_string(),
            description: "定时任务工具，可以设置定时提醒，在指定时间让指定角色执行任务".to_string(),
            parameters,
        };

        ToolDefinition {
            tool_type: "function".to_string(),
            function,
        }
    }

    async fn execute(&self, function: &Function) -> ToolResponse {
        let arguments = match &function.arguments {
            Some(s) => s,
            None => return ToolResponse::Error("timer: lack arguments".to_string()),
        };

        let args: Args = match serde_json::from_str(arguments) {
            Ok(a) => a,
            Err(e) => return ToolResponse::Error(format!("timer: parse arguments failed: {e}")),
        };

        match args.action.as_str() {
            "add" => self.action_add(args),
            "list" => self.action_list(),
            "remove" => self.action_remove(args),
            _ => ToolResponse::Timer {
                action: args.action.clone(),
                content: "unknown action, supported: add, list, remove".to_string(),
            },
        }
    }
}

impl TimerTool {
    ///路径获取
    fn timer_path() -> Result<PathBuf, TimerToolErr> {
        let path = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("synapcore_cache")
            .join("timer.json");
        if let Some(parent) = path.parent()
            && !parent.exists()
        {
            std::fs::create_dir_all(parent)?;
        }
        Ok(path)
    }
    ///读取列表
    fn load_list() -> Result<TimerList, TimerToolErr> {
        let path = Self::timer_path()?;
        if !path.exists() {
            return Ok(TimerList::default());
        }
        let content = std::fs::read_to_string(&path)?;
        let list: TimerList = serde_json::from_str(&content)?;
        Ok(list)
    }
    ///保存
    fn save_list(list: &TimerList) -> Result<(), TimerToolErr> {
        let path = Self::timer_path()?;
        let content = serde_json::to_string_pretty(&list)?;
        std::fs::write(&path, content)?;
        Ok(())
    }
    ///加入
    fn action_add(&self, args: Args) -> ToolResponse {
        //输入处理
        let time = match args.time {
            Some(t) => t,
            None => {
                return ToolResponse::Timer {
                    action: "add".to_string(),
                    content: "timer add: missing required field 'time'".to_string(),
                };
            }
        };
        let prompt = match args.prompt {
            Some(p) => p,
            None => {
                return ToolResponse::Timer {
                    action: "add".to_string(),
                    content: "timer add: missing required field 'prompt'".to_string(),
                };
            }
        };
        let character = match args.character {
            Some(c) => c.to_lowercase(),
            None => {
                return ToolResponse::Timer {
                    action: "add".to_string(),
                    content: "timer add: missing required field 'character'".to_string(),
                };
            }
        };

        let mut list = match Self::load_list() {
            Ok(l) => l,
            Err(e) => {
                return ToolResponse::Error(format!("timer add: load failed: {e}"));
            }
        };

        //正式加入
        let id = uuid::Uuid::new_v4().to_string();
        let entry = TimerEntry {
            id,
            time,
            character,
            prompt,
            done: false,
        };
        let display = entry.to_string();
        list.timers.push(entry);

        if let Err(e) = Self::save_list(&list) {
            return ToolResponse::Error(format!("timer add: save failed: {e}"));
        }

        ToolResponse::Timer {
            action: "add".to_string(),
            content: display,
        }
    }
    ///列出tasks
    fn action_list(&self) -> ToolResponse {
        let list = match Self::load_list() {
            Ok(l) => l,
            Err(e) => {
                return ToolResponse::Error(format!("timer list: load failed: {e}"));
            }
        };
        //获取未完成的tasks
        let pending: Vec<&TimerEntry> = list.timers.iter().filter(|t| !t.done).collect();

        if pending.is_empty() {
            return ToolResponse::Timer {
                action: "list".to_string(),
                content: "no pending timer tasks".to_string(),
            };
        }

        let mut content = String::new();
        for t in &pending {
            content.push_str(&t.to_string());
        }

        ToolResponse::Timer {
            action: "list".to_string(),
            content,
        }
    }
    ///移除tasks
    fn action_remove(&self, args: Args) -> ToolResponse {
        let id = match args.id {
            Some(i) => i,
            None => {
                return ToolResponse::Timer {
                    action: "remove".to_string(),
                    content: "timer remove: missing required field 'id'".to_string(),
                };
            }
        };

        let mut list = match Self::load_list() {
            Ok(l) => l,
            Err(e) => {
                return ToolResponse::Error(format!("timer remove: load failed: {e}"));
            }
        };

        let before = list.timers.len();
        list.timers.retain(|t| t.id != id);

        if list.timers.len() == before {
            return ToolResponse::Timer {
                action: "remove".to_string(),
                content: format!("timer remove: id '{}' not found", id),
            };
        }

        if let Err(e) = Self::save_list(&list) {
            return ToolResponse::Error(format!("timer remove: save failed: {e}"));
        }

        ToolResponse::Timer {
            action: "remove".to_string(),
            content: format!("removed timer id: {}", id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::define_call::tool_define::Tool;

    #[test]
    fn test_timer_tool_definition() {
        let tool = TimerTool;
        let def = tool.definition();
        assert_eq!(def.function.name, "timer");
        assert!(!def.function.description.is_empty());
    }
}
