use std::{io::Read, path::PathBuf, time::SystemTime};

use serde::{Deserialize, Serialize};

use crate::{
    define_call::tool_define::{FunctionDefinition, Tool, ToolDefinition},
    tool_response::ToolResponse,
};

#[derive(Default, Serialize, Deserialize)]
struct Args {
    mode: String,
    title: Option<String>,
    content: Option<String>,
    key_words: Option<String>,
}

///记事本工具
#[derive(Debug)]
pub(crate) struct NoteBook {
    path: PathBuf,
    pub(crate) character: String,
}

impl Tool for NoteBook {
    fn definition(&self) -> crate::define_call::tool_define::ToolDefinition {
        let note_fmt = include_str!("./fmt.md");
        let content = format!(
            "write时写入的内容。write时必须。格式以及注意:\n{}",
            note_fmt
        );

        let name = "note_book".to_string();
        let description ="这是一个记事本工具(也是一个你的专属日记)，提供 read,write,find 权限：
            1.read: 当你认为你需要从你曾经写的note中读取内容时调用。
            2.write: 当你觉得有哪些信息,想法，感受不能忘记,或者学习到新的知识，十分重要以供你未来查询时，或者你的情绪波动较大时，调用。
            3.find: 根据关键词查找note".to_string() ;
        let parameters = serde_json::json!({
            "type":"object",
            "properties":{
                "mode":{
                    "type":"string",
                    "description":"指定命令，提供read,write,find"
                },
                "title":{
                    "type":"string",
                    "description":"note的标题,read时作为指定的note。（read,write时必须）"
                },
                "content":{
                    "type":"string",
                    "description":content
                },
                "key_words":{
                    "type":"string",
                    "description":"find时的查询关键词。find时必须"
                }
            },
            "required":["command"]
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
            None => return ToolResponse::Error("Function note_book lacks arguments".to_string()),
        };
        let args: Args = serde_json::from_str(arguments).unwrap_or_default();

        match args.mode.as_str() {
            "read" => {
                if args.title.is_none() {
                    return ToolResponse::Error(
                        "Function note_book mode read lacks argument title".to_string(),
                    );
                }
                match self.read(&args.title.unwrap()) {
                    Ok(s) => ToolResponse::NoteBook {
                        mode: "read".to_string(),
                        content: s,
                    },
                    Err(e) => ToolResponse::Error(e.to_string()),
                }
            }
            "write" => {
                if args.title.is_none() {
                    return ToolResponse::Error(
                        "Function note_book mode write lacks argument title".to_string(),
                    );
                }

                if args.content.is_none() {
                    return ToolResponse::Error(
                        "Function note_book mode write lacks argument content".to_string(),
                    );
                }
                match self.write(&args.title.unwrap(), &args.content.unwrap()) {
                    Ok(_) => ToolResponse::NoteBook {
                        mode: "write".to_string(),
                        content: "success".to_string(),
                    },
                    Err(e) => ToolResponse::Error(e.to_string()),
                }
            }
            "find" => {
                if args.key_words.is_none() {
                    return ToolResponse::Error(
                        "Function note_book mode find lacks argument key_words".to_string(),
                    );
                }
                ToolResponse::NoteBook {
                    mode: "find".to_string(),
                    content: self.find(&args.key_words.unwrap()),
                }
            }
            _ => ToolResponse::Error(format!("Function note_book unkown mode:{}", &args.mode)),
        }
    }
}

impl NoteBook {
    pub(crate) fn new() -> Self {
        let path = dirs::cache_dir()
            .unwrap_or(PathBuf::from("./"))
            .join("synapcore_cache")
            .join("notes");

        Self {
            path,
            character: "none".to_string(),
        }
    }

    fn read(&self, title: &str) -> Result<String, std::io::Error> {
        // println!("title:{}",title);
        let path = self.path.join(&self.character);
        let path = path.join(format!("{}.md", title));
        let mut content = format!("title:{}\n", title);
        content.push_str(&std::fs::read_to_string(path)?);
        Ok(content)
    }

    fn write(&self, title: &str, content: &str) -> Result<(), std::io::Error> {
        let path = self.path.join(&self.character);
        if !path.exists() {
            std::fs::create_dir_all(&path)?;
            // println!("ok");
        }

        let path = path.join(format!("{}.md", title));
        std::fs::write(path, content)
    }

    fn find(&self, key_words: &str) -> String {
        use super::files_system::FileSystem;

        let path = self.path.join(&self.character);
        let list = FileSystem::grep(&path, key_words, 1).unwrap_or_default();
        let mut content = String::new();

        list.iter().for_each(|v| {
            content.push_str(&v.to_string());
        });

        content
    }
    ///获取最新
    pub(crate) fn get_last(&self) -> Option<String> {
        let path = self.path.join(&self.character);
        // println!("path:{}",path.display());
        let entries = std::fs::read_dir(&path).ok()?;
        // println!("en{}",);

        let mut lastest: Option<PathBuf> = None;
        let mut lastest_time: Option<SystemTime> = None;

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let path = entry.path();
            println!("path:{}", path.display());

            if !path.is_file() {
                continue;
            }

            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };

            let time = match metadata.modified() {
                Ok(t) => t,
                Err(_) => continue,
            };

            if lastest_time.is_none() || time > lastest_time.unwrap() {
                lastest_time = Some(time);
                lastest = Some(path)
            }
        }

        let title = lastest?.file_stem()?.to_str()?.to_string();
        let content = self.read(&title).ok()?;
        Some(content)
    }
}
