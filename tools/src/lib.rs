use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use crate::{
    define_call::{
        tool_call::ToolCall,
        tool_define::{Tool, ToolDefinition},
    },
    error::ToolErr,
    files_write::FileWriter,
    tool_response::ToolResponse,
};

pub mod define_call;
pub mod error;
mod files_extract;
mod files_write;
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
    // #[serde(skip)]
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

        let inner = vec![extract, write, web];

        Self {
            sandbox_path: std::env::current_dir().unwrap_or_default(),
            sandbox_dyn: true,
            inner,
            // map: Rc::new(RefCell::new(HashMap::new())),
        }
    }
}

impl Tools {
    pub async fn call(&self, tool: ToolCall) -> Result<ToolResponse, ToolErr> {
        let response = match tool.function.name.clone().unwrap_or_default().as_str() {
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
            _ => return Err(ToolErr::Unkown),
        };
        Ok(response)
    }
    pub fn init(&mut self, root: &Path) -> Result<Vec<ToolDefinition>, ToolErr> {
        // let mut list = Vec::new();
        let path = root.join("tools").join("tools.toml");
        Tools::confirm_path(&path)?;

        let tools = Tools::loading_tools(&path)?;
        self.sandbox_path = tools.sandbox_path;
        self.sandbox_dyn = tools.sandbox_dyn;
        self.inner = tools.inner;

        
        // println!("Self:{:#?}",self);

        let inner: &Vec<Inner> = self.inner.as_ref();
        let inner_enabled = inner
            .iter()
            .filter(|t| t.enable)
            .map(|t| t.name.as_str())
            .collect();
        let list = self.get_enabled_inner(inner_enabled);

        Ok(list)
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
    fn get_enabled_inner(&self, list: Vec<&str>) -> Vec<ToolDefinition> {
        let mut enabled_list = Vec::new();

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
        enabled_list
    }
}

mod test {
    use std::path::{Path, PathBuf};

    use crate::{Tools, define_call::tool_call::{self, Function}};

    #[tokio::test]
    async fn test() {
        let root = "/home/mypenfly/.config/synapcore";
        let mut tools = Tools::default();
        let path = PathBuf::from(root);
        let _ = tools.init(&path);
        // println!("{:#?}",&tools);

        let args = "{\"query\":\"fulutter rust FFI flutter_rust_bridge 最佳实践\"}".to_string();
        let function = Function{
            name:Some("web_search".to_string()),
            arguments:Some(args)
            
        };
        let call = tool_call::ToolCall{
            index:0,
            id:Some("test".to_string()),
            tool_type:Some("function".to_string()),
            function
        };
        let response =tools.call(call).await.unwrap();
        println!("{:#?}",response);
    }
}
