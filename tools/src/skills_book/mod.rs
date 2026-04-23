use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
    define_call::tool_define::{FunctionDefinition, Tool, ToolDefinition},
    tool_response::ToolResponse,
};

#[derive(Default, Debug, Serialize, Deserialize)]
struct Args {
    mode: String,
    title: Option<String>,
    content: Option<String>,
}

///技能书记录工具
#[derive(Debug)]
pub(crate) struct SkillsBook;

impl Tool for SkillsBook {
    fn definition(&self) -> crate::define_call::tool_define::ToolDefinition {
        let parameters = serde_json::json!({
            "type": "object",
            "properties": {
                "mode": {
                    "type": "string",
                    "description": "指定命令，提供add,remove,read"
                },
                "title": {
                    "type": "string",
                    "description": "技能的标题，add,remove,read时必须"
                },
                "content": {
                    "type": "string",
                    "description": "add时的技能内容，详细的技能描述，必须包含description行来简要介绍使用。add时必须"
                }
            },
            "required": ["mode","title"]
        });

        let name = "skills_book".to_string();
        let description = "这是一个技能书记录工具，记录agent的skills，提供add,remove,read操作：
            1. add: 添加新技能，需要title和content参数
            2. remove: 删除指定技能的记录
            3. read: 读取指定技能的内容"
            .to_string();

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
            None => return ToolResponse::Error("Function skills_book lacks arguments".to_string()),
        };
        let args: Args = match serde_json::from_str(arguments) {
            Ok(s) => s,
            Err(e) => return ToolResponse::Error(format!("Function skills_book failed: {}", e)),
        };

        match args.mode.as_str() {
            "add" => {
                if let (Some(title), Some(content)) = (args.title, args.content) {
                    match self.add(&title, &content) {
                        Ok(_) => ToolResponse::SkillsBook {
                            mode: "add".to_string(),
                            content: format!("Added skill {}", title),
                        },
                        Err(e) => ToolResponse::Error(e.to_string()),
                    }
                } else {
                    ToolResponse::Error(
                        "Function skills_book mode add requires both title and content".to_string(),
                    )
                }
            }
            "remove" => {
                if let Some(title) = args.title {
                    match self.remove(&title) {
                        Ok(_) => ToolResponse::SkillsBook {
                            mode: "remove".to_string(),
                            content: format!("Removed skill {}", title),
                        },
                        Err(e) => ToolResponse::Error(e.to_string()),
                    }
                } else {
                    ToolResponse::Error(
                        "Function skills_book mode remove requires title".to_string(),
                    )
                }
            }
            "read" => {
                if let Some(title) = args.title {
                    match self.read(&title) {
                        Ok(content) => ToolResponse::SkillsBook {
                            mode: "read".to_string(),
                            content,
                        },
                        Err(e) => ToolResponse::Error(e.to_string()),
                    }
                } else {
                    ToolResponse::Error("Function skills_book mode read requires title".to_string())
                }
            }
            _ => ToolResponse::Error(format!("Function skills_book unknown mode: {}", &args.mode)),
        }
    }
}

impl SkillsBook {
    fn get_skills_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or(PathBuf::from("./"))
            .join("synapcore")
            .join("skills")
    }

    fn add(&self, title: &str, content: &str) -> Result<(), std::io::Error> {
        let dir = Self::get_skills_dir();
        if !dir.exists() {
            std::fs::create_dir_all(&dir)?;
        }

        let path = dir.join(format!("{}.md", title));
        std::fs::write(path, content)
    }

    fn remove(&self, title: &str) -> Result<(), std::io::Error> {
        let path = Self::get_skills_dir().join(format!("{}.md", title));
        if path.exists() {
            std::fs::remove_file(path)
        } else {
            Ok(())
        }
    }

    fn read(&self, title: &str) -> Result<String, std::io::Error> {
        let path = Self::get_skills_dir().join(format!("{}.md", title));
        std::fs::read_to_string(path)
    }

    ///获取所有技能标题列表
    pub(crate) fn get_skills(&self) -> Vec<String> {
        let dir = Self::get_skills_dir();
        if !dir.exists() {
            return Vec::new();
        }

        let mut skills = Vec::new();
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(file_name) = path.file_stem()
                    && path.is_file()
                {
                    let title = file_name.to_str().unwrap_or("unkown");
                    skills.push(title.to_string());
                }
            }
        }
        skills
    }
}
