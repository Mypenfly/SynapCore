use std::{fmt::Display, io, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::{
    define_call::tool_define::{FunctionDefinition, Tool, ToolDefinition},
    todo_list::error::TodoListErr,
    tool_response::ToolResponse,
};

mod error;

#[derive(Debug, Serialize, Deserialize)]
struct Args {
    action: String,
    list: Option<Vec<String>>,
    id: Option<usize>,
    update_state: Option<TaskState>,
}

///todo_list结构体定义
pub(crate) struct TodoList {
    ///模型角色
    character: String,
}

impl Tool for TodoList {
    fn definition(&self) -> crate::define_call::tool_define::ToolDefinition {
        let name = "todo_list".to_string();
        let description = "长期规划的用的待做事项表，请通过这个工具实现合理的任务规划".to_string();

        let parameters = serde_json::json!({
            "type":"object",
            "properties":{
                "action":{
                    "type":"string",
                    "description":"指定操作。支持类型：
                    create(创建新的todo_list,会覆盖以前的list，使用时需要传入list)，
                    read(读取目前的todo_list)，
                    update(更新某项任务的状态,需要传入指定的任务id,和要更新的状态)"
                },
                "list":{
                    "type":"array",
                    "items":{"type":"string"},
                    "description":"create操作时必须。创建的任务列表内容"
                },
                "id":{
                    "type":"number",
                    "description":"update时要必须,表示某项任务，索引从0开始"
                },
                "update_state":{
                    "type":"string",
                    "description":"update时必须，表示要将状态改至。只有三种状态可选：
                    wait (等待中),
                    error (执行失败),
                    success (执行成功)"
                }
            },
            "required":["action"]
        });

        let function = FunctionDefinition {
            name,
            description,
            parameters,
        };

        ToolDefinition {
            tool_type: "function".to_string(),
            function,
        }
    }

    async fn execute(
        &self,
        function: &crate::define_call::tool_call::Function,
    ) -> crate::tool_response::ToolResponse {
        let arguments = match &function.arguments {
            Some(s) => s,
            None => return ToolResponse::Error("Function todo_list lacks arguments".to_string()),
        };

        let args: Result<Args, serde_json::Error> = serde_json::from_str(arguments);

        if let Err(e) = args {
            return ToolResponse::Error(format!("Function todo_lsit failed : {}", e));
        }

        let args = args.unwrap();
        match self.take_action(args) {
            Ok(r) => r,
            Err(e) => ToolResponse::Error(format!("Function todo list failed : {}", e)),
        }
    }
}

impl TodoList {
    pub(crate) fn new(character: String) -> Self {
        Self { character }
    }

    ///处理action,分流
    fn take_action(&self, args: Args) -> Result<ToolResponse, TodoListErr> {
        let response = match args.action.as_ref() {
            "create" => match args.list.is_none() {
                true => ToolResponse::TodoList {
                    action: "create".to_string(),
                    content: "TodoList action create lacks list argument".to_string(),
                },
                false => self.create(args.list.unwrap())?,
            },
            "read" => self.read()?,
            "update" if args.id.is_none() => ToolResponse::TodoList {
                action: "update".to_string(),
                content: "TodoList action update lacks task id".to_string(),
            },
            "update" if args.update_state.is_none() => ToolResponse::TodoList {
                action: "update".to_string(),
                content: "TodoList action update lacks update state (eg. wait,error,success)"
                    .to_string(),
            },
            "update" => {
                let id = args.id.unwrap();
                self.update(id, &args.update_state.unwrap())?
            }
            _ => ToolResponse::TodoList {
                action: args.action.clone(),
                content: "unkown action".to_string(),
            },
        };

        Ok(response)
    }

    ///创建todo_list
    fn create(&self, list: Vec<String>) -> Result<ToolResponse, TodoListErr> {
        let path = self.get_file().map_err(TodoListErr::Io)?;
        let mut tasks = Vec::new();
        for (id, content) in list.iter().enumerate() {
            let task = Task {
                id,
                content: content.to_string(),
                state: TaskState::Wait,
            };
            tasks.push(task);
        }

        let save = SaveList { tasks };
        let json = serde_json::to_string_pretty(&save).map_err(TodoListErr::Serde)?;

        std::fs::write(path, json).map_err(TodoListErr::Io)?;
        Ok(ToolResponse::TodoList {
            action: "create".to_string(),
            content: save.to_string(),
        })
    }

    ///读取
    fn read(&self) -> Result<ToolResponse, TodoListErr> {
        let path = self.get_file().map_err(TodoListErr::Io)?;

        let content = std::fs::read_to_string(path).map_err(TodoListErr::Io)?;

        let list: SaveList = serde_json::from_str(&content).map_err(TodoListErr::Serde)?;

        Ok(ToolResponse::TodoList {
            action: "read".to_string(),
            content: list.to_string(),
        })
    }

    ///更新list
    fn update(&self, id: usize, update_state: &TaskState) -> Result<ToolResponse, TodoListErr> {
        let path = self.get_file().map_err(TodoListErr::Io)?;

        let content = std::fs::read_to_string(&path).map_err(TodoListErr::Io)?;

        let mut list: SaveList = serde_json::from_str(&content).map_err(TodoListErr::Serde)?;

        let task = list.tasks.iter_mut().find(|t| t.id == id);

        if task.is_none() {
            return Ok(ToolResponse::TodoList {
                action: "update".to_string(),
                content: format!("not found id {} in the list", id),
            });
        }

        let task = task.unwrap();
        //记录原始状态
        let raw_state = task.state;
        let content = task.content.clone();

        task.state = *update_state;

        //覆盖写入
        let json = serde_json::to_string_pretty(&list).map_err(TodoListErr::Serde)?;
        std::fs::write(path, json).map_err(TodoListErr::Io)?;

        //组织标识语言
        // let now_state = task.state;

        let content = format!(
            "id:{}\nstate:{} -> {}\n{}\n\n",
            id, raw_state, update_state, content
        );

        Ok(ToolResponse::TodoList {
            action: "update".to_string(),
            content,
        })
    }

    ///获取文件
    fn get_file(&self) -> Result<PathBuf, io::Error> {
        let path = dirs::cache_dir()
            .unwrap_or_default()
            .join("synapcore_cache")
            .join("todo_list")
            .join(format!("{}.json", &self.character));
        if !path.exists() {
            let parent = path.parent().unwrap();

            std::fs::create_dir_all(parent)?;

            std::fs::File::create(&path)?;
        }
        Ok(path)
    }
}

///保存的结构
#[derive(Deserialize, Serialize, Debug)]
struct SaveList {
    tasks: Vec<Task>,
}

impl Display for SaveList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut content = "todo list tasks :\n".to_string();

        for task in &self.tasks {
            content.push_str(&task.to_string());
        }
        write!(f, "{}", content)
    }
}

///任务的结构
#[derive(Deserialize, Serialize, Debug)]
struct Task {
    id: usize,
    content: String,
    state: TaskState,
}

impl Display for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "id:{}\nstate:{}\n{}\n\n",
            self.id, self.state, self.content
        )
    }
}

///任务状态
#[derive(Deserialize, Serialize, Debug, Default, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum TaskState {
    #[default]
    Wait,
    Error,
    Success,
}

impl Display for TaskState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Wait => write!(f, "wait"),
            Self::Error => write!(f, "error"),
            Self::Success => write!(f, "success"),
        }
    }
}
