use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf}, rc::Rc, sync::Arc,
};

use crate::{
    define_call::{
        tool_call::ToolCall,
        tool_define::{Tool, ToolDefinition},
    }, error::ToolErr, files_write::FileWriter, outer::{OuterTools, config::Outer}, search_tools::ToolsManager, tool_response::ToolResponse
};

pub mod define_call;
pub mod error;
mod fetch_url;
mod files_extract;
mod files_system;
mod files_write;
mod note_book;
mod outer;
mod search_tools;
pub mod tool_response;
mod web_search;

use files_extract::ExtractTool;
use serde::{Deserialize, Serialize};

///内部工具
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Inner {
    name: String,
    enable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<HashMap<String, String>>,
}

///工具配置
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct Tools {
    sandbox_path: PathBuf,
    sandbox_dyn: bool,
    inner: Vec<Inner>,
    #[serde(skip_serializing_if ="Vec::is_empty")]
    outer:Arc<Vec<Outer>>,
    #[serde(skip)]
    character: String, // #[serde(skip)]
    #[serde(skip)]
    manager: ToolsManager,
    // all_tools:Vec<ToolDefinition>,
    #[serde(skip)]
    active_tools: Vec<ToolDefinition>,
    // map: Rc<RefCell<HashMap<String,Box<dyn Tool>>>>
}

impl Default for Tools {
    fn default() -> Self {
        let extract = Inner {
            name: "files_extract".to_string(),
            enable: true,
            params: None,
        };

        let write = Inner {
            name: "files_write".to_string(),
            enable: true,
            params: None,
        };

        let mut map = HashMap::new();
        map.insert(
            "base_url".to_string(),
            "BASE URL FOR WEB SEARCH".to_string(),
        );
        map.insert(
            "api_key".to_string(),
            "YOUR API KEY FOR WEB SEARCH".to_string(),
        );
        let params = Some(map);
        let web = Inner {
            name: "web_search".to_string(),
            enable: true,
            params,
        };

        let sys = Inner {
            name: "files_system".to_string(),
            enable: true,
            params: None,
        };

        let fetch = Inner {
            name: "fetch_url".to_string(),
            enable: true,
            params: None,
        };

        let note = Inner {
            name: "note_book".to_string(),
            enable: true,
            params: None,
        };

        let inner = vec![extract, write, web, sys, fetch, note];
        let outer =vec![Outer::default()] ;

        Self {
            sandbox_path: std::env::current_dir().unwrap_or_default(),
            sandbox_dyn: true,
            inner,
            outer:Arc::new(outer),
            character: "none".to_string(), // map: Rc::new(RefCell::new(HashMap::new())),
            manager: ToolsManager::default(),
            active_tools: Vec::new(),
        }
    }
}

impl Tools {
    pub async fn call(&mut self, tool: ToolCall) -> Result<ToolResponse, ToolErr> {
        let response = match tool.function.name.clone().unwrap_or_default().as_str() {
            "tools_manager" => {
                let res = self.manager.execute(&tool.function).await;
                if let ToolResponse::Manager { mode, definations } = &res {
                    match mode.as_str() {
                        "search" => ToolResponse::ManagerAdd(res.to_string()),
                        "add" => {
                            let names: Vec<String> = definations
                                .iter()
                                .map(|d| d.function.name.to_string())
                                .collect();
                            self.active_tools.extend(definations.clone());
                            ToolResponse::ManagerAdd(format!("add function :{:?}", names))
                        }
                        _ => unreachable!(),
                    }
                } else {
                    ToolResponse::Error("Function tools_manager error occured".to_string())
                }
            }
            "files_extract" => {
                let extract = ExtractTool;
                extract.execute(&tool.function).await
            }
            "files_write" => {
                let write = FileWriter::new(&self.sandbox_path);
                write.execute(&tool.function).await
            }
            "web_search" => {
                let params = self
                    .inner
                    .iter()
                    .find(|i| i.name == "web_search")
                    .map(|w| w.params.clone().unwrap_or_default())
                    .unwrap_or_default();
                let search = web_search::WebSearch { params };
                search.execute(&tool.function).await
            }
            "files_system" => {
                use files_system::FileSystem;
                let sys = FileSystem::new(&self.sandbox_path);
                // FileSystem::execute(FileSystem {  }, &tool.function).await
                sys.execute(&tool.function).await
            }
            "fetch_url" => {
                use fetch_url::FetchUrl;
                let fetch = FetchUrl {};
                fetch.execute(&tool.function).await
            }
            "note_book" => {
                use note_book::NoteBook;
                let mut note = NoteBook::new();
                note.character = self.character.clone();
                // println!("note:{:#?}",note);
                note.execute(&tool.function).await
            }
            _ => {
                if self.outer.is_empty() {
                    
                
                
                return Err(ToolErr::Unkown)
                }

                // let outers = self.outer.as_ref();

                let tools = OuterTools{
                    outers:Arc::clone(&self.outer)
                };

                tools.execute(&tool.function).await
            },
        };
        Ok(response)
    }
    pub fn init(root: &Path, character: &str) -> Result<Tools, ToolErr> {
        // let mut list = Vec::new();
        let path = root.join("tools").join("tools.toml");
        Tools::confirm_path(&path)?;
        let mut tools = Tools::loading_tools(&path)?;
        tools.character = character.to_string();
        //处理一下动态的沙盒路径
        if tools.sandbox_dyn {
            tools.sandbox_path = std::env::current_dir().unwrap_or_default();
        }
        // println!("Self:{:#?}",self);

        // let inner: &Vec<Inner> = tools.inner.as_ref();
        let inner_enabled = tools
            .inner
            .iter()
            .filter(|t| t.enable)
            .map(|t| t.name.to_string())
            .collect();
        tools.get_enabled_inner(inner_enabled);
        tools.get_outer_enable();
        // let list =tools.active_tools.clone() ;

        Ok(tools)
    }
    ///路径确认
    fn confirm_path(path: &Path) -> Result<(), ToolErr> {
        if path.exists() {
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(ToolErr::ReadConfigError)?;
        }

        let tools = Tools::default();
        let content = toml::to_string_pretty(&tools).map_err(ToolErr::TomlError)?;

        fs::File::create_new(path).map_err(ToolErr::ReadConfigError)?;

        fs::write(path, content).map_err(ToolErr::ReadConfigError)?;

        Ok(())
    }

    ///解析可用的工具
    fn loading_tools(path: &Path) -> Result<Tools, ToolErr> {
        let content = std::fs::read_to_string(path).map_err(ToolErr::ReadConfigError)?;
        // println!("content:{}",&content);

        let tools: Tools = toml::from_str(&content).map_err(ToolErr::SerdeError)?;
        // println!("tools:{:#?}",&tools);

        Ok(tools)
    }

    ///解析内部可用工具
    fn get_enabled_inner(&mut self, list: Vec<String>) {
        let mut enabled_list = Vec::new();
        let list: Vec<&str> = list.iter().map(|s| s.as_str()).collect();

        if list.contains(&"files_extract") {
            let files_extract = files_extract::ExtractTool;
            let extract_de = files_extract.definition();
            enabled_list.push(extract_de);
            // self.map.borrow_mut().insert("files_extract".to_string(), Box::new(files_extract));
        }
        if list.contains(&"files_write") {
            let files_write = FileWriter {
                sand_box: self.sandbox_path.clone(),
            };
            let write_de = files_write.definition();
            enabled_list.push(write_de);
            // self.map.borrow_mut().insert("files_write".to_string(), Box::new(files_write));
        }
        if list.contains(&"web_search") {
            let web_search = web_search::WebSearch::default();
            let search_de = web_search.definition();
            enabled_list.push(search_de);
        }
        if list.contains(&"fetch_url") {
            let fetch_url = fetch_url::FetchUrl {};
            let desription = fetch_url.definition();
            enabled_list.push(desription);
        }
        if list.contains(&"files_system") {
            let files_system = files_system::FileSystem::new(&self.sandbox_path);
            let description = files_system.definition();
            enabled_list.push(description);
        }
        //笔记工具提前保留，让模型可以习惯使用
        if list.contains(&"note_book") {
            let note = note_book::NoteBook::new();
            let note_description = note.definition();
            self.active_tools.push(note_description);
            // enabled_list.push(note_description);
        }
        self.manager = ToolsManager {
            enabled: enabled_list,
        };

        self.active_tools.push(self.manager.definition());
    }

    ///解析外部可用工具
    fn get_outer_enable(&mut self) {
        if self.outer.is_empty() {
            return;
        }

        let tools = OuterTools{
            outers:Arc::clone(&self.outer)
        };

        let list = tools.defination();
        self.manager.enabled.extend(list);
    }

    ///获取最新的note
    pub fn get_last_note(&self) -> String {
        let mut note = note_book::NoteBook::new();
        note.character = self.character.clone();
        note.get_last().unwrap_or_default()
    }
    pub fn get_active(&self) -> Vec<ToolDefinition> {
        self.active_tools.clone()
    }
}

mod test {
    use std::path::PathBuf;

    use docx_rs::Level;

    use crate::{
        Tools,
        define_call::tool_call::{self, Function}, outer::{OuterTools, config::Outer},
    };

    #[tokio::test]
    async fn test() {
        let root = "/home/mypenfly/.config/synapcore";
        // let mut tools = Tools::default();
        let path = PathBuf::from(root);
        let mut tools = Tools::init(&path, "Yore").unwrap();
        // // tools = new_t;
        // // println!("{:#?}",&list);

        // // let args = "{\"query\": \"生命科学竞赛 大学生 含金量\", \"count\": 5}".to_string();
        // // let args ="{\"command\":\"ls\",\"path\":\"~/projects/rs-musicdog\"}".to_string() ;
        // // let args ="{\"command\":\"ls\",\"path\":\"~/projects/rs-musicdog\",\"pattern\":\"music\",\"depth\":3,\"target_path\":\"./test/flake.lock\"}".to_string() ;
        // let args = "{\"mode\":\"find\",\"title\":\"test\",\"content\":\"just a test for note book\",\"key_words\":\"test\"}".to_string();
        let args = "{\"source\":\"douban\"}".to_string();
        let function = Function {
            name: Some("get_hits".to_string()),
            arguments: Some(args),
        };
        let call = tool_call::ToolCall {
            index: 0,
            id: Some("test".to_string()),
            tool_type: Some("function".to_string()),
            function,
        };
        let response = tools.call(call).await.unwrap();
        println!("{}", response);
        // println!("{:#?}", tools);
        //
        // let outers =vec![Outer::default(),Outer::default()];
        
        // let last = tools.get_last_note();
        // println!("last:{}", last);
    }
}
