use std::fmt::Display;

use crate::{
    define_call::tool_define::ToolDefinition, files_extract::ExtractRes, web_search::SearchValue,
};

#[derive(Debug)]
pub enum ToolResponse {
    Manager {
        mode: String,
        definations: Vec<ToolDefinition>,
    },
    ManagerAdd(String),
    Extract(Vec<ExtractRes>),
    Write {
        path: String,
        content: String,
    },
    WebSearch(Vec<SearchValue>),
    FileSystem(String),
    FetchUrl {
        url: String,
        content: String,
    },
    NoteBook {
        mode: String,
        content: String,
    },
    OuterTool {
        name: String,
        output: String,
    },
    Executor {
        command: String,
        content: String,
    },
    Bash {
        command: String,
        output: String,
    },
    Error(String),
}

impl Display for ToolResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Manager { mode, definations } => {
                write!(f, "mode:{}\nlist:{:?}", mode, definations)
            }
            Self::ManagerAdd(s) => write!(f, "{}", s),
            Self::Extract(list) => {
                let mut content = String::new();
                list.iter().for_each(|v| {
                    content.push_str(&v.to_string());
                });
                write!(f, "{}", content)
            }
            Self::Write { path, content } => write!(f, "{} :\n{}", path, content),
            Self::WebSearch(list) => {
                let mut content = String::new();
                list.iter().for_each(|v| {
                        content.push_str(&format!(
                            "id:{},url:{}\ntitle:{}\nsnippet:{}\nsummary:{}\nsite:{},publishData:{},updataData:{}\n\n",
                            v.id,
                            v.url,
                            v.name,
                            v.snippet,
                            v.summary.clone().unwrap_or_default(),
                            v.site_name,
                            v.data_last_published.clone().unwrap_or_default(),
                            v.data_last_crawled.clone().unwrap_or_default()
                        ));
                    });
                write!(f, "{}", content)
            }
            Self::FileSystem(s) => write!(f, "{}", s),
            Self::FetchUrl { url, content } => write!(f, "{} :\n{}", url, content),
            Self::NoteBook { mode, content } => write!(f, "mode:{}\n{}", mode, content),
            Self::OuterTool { name, output } => write!(f, "name:{}\n{}", name, output),
            Self::Executor { command, content } => write!(f, "cmd:{}\n{}", command, content),
            Self::Bash { command, output } => write!(f, ">{}\n{}\n", command, output),
            Self::Error(e) => write!(f, "{}", e),
        }
    }
}
