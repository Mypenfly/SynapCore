use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::{define_call::tool_define::{FunctionDefinition, Tool, ToolDefinition}, tool_response::ToolResponse};

#[derive(Serialize,Deserialize,Debug)]
struct Args{
    command:String,
    args:Option<Vec<String>>
}

pub(crate) struct Executor{}

impl Tool for Executor {
    fn definition(&self)->crate::define_call::tool_define::ToolDefinition {
        let name = "executor".to_string();
        let description = "执行外部程序的工具，也即是shell命令执行工具（不过是一次性的）,该工具几乎可以调用除了sudo以外的指令，慎用！！！".to_string();
        let parameters =serde_json::json!({
            "type":"object",
            "properties":{
                "command":{
                    "type":"string",
                    "description":"命令（如ls,cd）或者是运行时（如python,nmp,cargo）。注意一般不支持sudo,rm之类的，有需要请使用files_system"
                },
                "args":{
                    "type":"array",
                    "item":{"type":"string"},
                    "description":"命令参数，例如：[\"--location\",\"China\"]"
                }
            },
            "required":["command"]
        }) ;

        let function = FunctionDefinition{
            name,
            description,
            parameters
        };

        ToolDefinition{
            tool_type:"function".to_string(),
            function
        }
    }

    async fn execute(&self,function:&crate::define_call::tool_call::Function)->crate::tool_response::ToolResponse {
        let arguments =match &function.arguments{
            Some(s)=>s,
            None => return ToolResponse::Error("Function executor lacks arguments".to_string())
        };
        let args:Result<Args,serde_json::Error> = serde_json::from_str(arguments);
        if let Err(e) = args {
          return ToolResponse::Error(format!("Function executor failed : {}",e));  
        };

        let args = args.unwrap();
        shell(&args)
        
    }
}

fn shell(args:&Args) -> ToolResponse {
    let program = &args.command;
    let mut cmd = Command::new(program);

    let mut choice = String::new();
    if let Some(arguments) = args.args.as_ref() {
        for a in arguments {
            cmd.arg(a);
            choice.push_str(a);
        }
    }

    let output =match cmd.output(){
        Ok(o) => o,
        Err(e) => return ToolResponse::Error(format!("Function executor failed in cmd {},error:{}",&program,e))
    };

    let command =format!("{} {}",program,choice) ;
    let text = String::from_utf8(output.stdout).unwrap_or_default();

    if text.is_empty() {
        let error =String::from_utf8(output.stderr).unwrap_or_default() ;
        return ToolResponse::Executor{command,content:error};
    }

    ToolResponse::Executor{command,content:text}
}
