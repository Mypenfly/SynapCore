use serde::{Deserialize, Serialize};

use crate::{define_call::tool_define::{FunctionDefinition, Tool, ToolDefinition}, tool_response::ToolResponse};

#[derive(Debug,Serialize,Deserialize)]
struct Args{
    action:String,
    query:String
}

#[derive(Default,Clone,Debug)]
pub(crate) struct  ToolsManager{
    pub(crate) enabled:Vec<ToolDefinition>
}

impl Tool for ToolsManager {
    fn definition(&self)->crate::define_call::tool_define::ToolDefinition {
        let description = "查找，加载现有的工具。用于获得工具的列表和详细定义，和加载可用工具。（建议结合用户需求和实际需要进行使用）".to_string();
        let name ="tools_manager".to_string() ;
        let list:Vec<String> = self.enabled
            .iter()
            .map(|e|e.function.name.clone())
            .collect();

        let sub_description =format!("提供search,add两个命令，现有的工具列表和简介:{:?}",list) ;
        let parameters = serde_json::json!({
            "type":"object",
            "properties":{
                "action":{
                    "type":"string",
                    "description":sub_description
                },
                "query":{
                    "type":"string",
                    "description":"search 时这项是用于搜索的关键词（按工具名模糊搜索,当此项为all时获取全部定义，不推荐），获得完整定义。add 时这项是用于载入工具（工具名）"
                }
                
            },
            "required":["action"]
        });
        let function =FunctionDefinition{
            name,description,parameters
        } ;

        ToolDefinition { tool_type: "function".to_string(), function }
    }

    async fn execute(&self,function:&crate::define_call::tool_call::Function)->crate::tool_response::ToolResponse {
        let arguments =match &function.arguments{
            Some(a)=>a,
            None => return ToolResponse::Error("Function tools_manager lacks arguments".to_string())
        };        

        let result:Result<Args,serde_json::Error> = serde_json::from_str(arguments);

        if let Err(e) =result  {
            return ToolResponse::Error(format!("Function tools_manager failed: {}",e));
        };
        let args =result.unwrap() ;

        match args.action.as_str() {
            "search"=> self.search(&args.query),
            "add" => {
                let query = args.query;
                if query == "all" {
                    ToolResponse::Manager { mode: "add".to_string(), definations: self.enabled.clone() }
                }else {
                    self.add(&query)
                }
            },
            _ => ToolResponse::Error(format!("Function tools_manager unkown action : {}",&args.action))
        }
        
    }
}

impl ToolsManager {
    fn search(&self,query:&str)->ToolResponse{
        let definations=self.enabled.iter().filter(|e|e.function.name.contains(query)).cloned().collect()
        ;ToolResponse::Manager { mode: "search".to_string(), definations }
    }
    fn add(&self,query:&str)->ToolResponse {
        let definations = self.enabled.iter().filter(|e|e.function.name.contains(query)).cloned().collect();
        ToolResponse::Manager { mode: "add".to_string(), definations }
    }
}
