use std::{collections::HashMap, process::Command};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    define_call::tool_define::{FunctionDefinition, Tool, ToolDefinition},
    tool_response::ToolResponse,
};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct Outer {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) enable: bool,
    pub(crate) exec: Vec<String>,
    pub(crate) required: Vec<String>,
    pub(crate) parameters: Option<HashMap<String, Parameters>>,
}

impl Tool for Outer {
    fn definition(&self) -> crate::define_call::tool_define::ToolDefinition {
        let parameters = serde_json::json!({
            "type":"object",
            "properties":self.parameters,
            "required":self.required,
        });
        let function = FunctionDefinition {
            name: self.name.clone(),
            description: self.description.clone(),
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
        let name = function.name.as_ref().unwrap();
        let arguments = match &function.arguments {
            Some(s) => s,
            None => {
                if self.parameters.is_none() {
                    &String::new()
                } else {
                    return ToolResponse::Error(format!("Function {} lacks arguments", name));
                }
            }
        };
        let args: HashMap<String, Value> = serde_json::from_str(arguments).unwrap_or_default();

        // println!("exec:{}", &self.exec);
        if self.exec.is_empty() {
            return ToolResponse::OuterTool {
                name: name.clone(),
                output: "解析 exec 错误".to_string(),
            };
        }

        //解析选项和命令参数
        let mut cmd = Command::new(&self.exec[0]);
        //
        // 命令参数
        for (i, e) in self.exec.iter().enumerate() {
            if i == 0 {
                continue;
            }
            cmd.arg(e);
        }
        //选项
        for (k, v) in args {
            cmd.arg(format!("--{}", k));

            cmd.arg(v.as_str().unwrap());
        }

        // println!("CMD: {:#?}",&cmd);

        let output = match cmd.output() {
            Ok(o) => o,
            Err(e) => return ToolResponse::Error(format!("{} 执行异常 {}", name, e)),
        };

        // println!("ouput:{:#?}",&output);
        //
        //错误处理
        let text = String::from_utf8(output.stdout).unwrap_or_default();
        if text.is_empty() {
            let error = String::from_utf8(output.stderr).unwrap_or_default();
            return ToolResponse::Error(format!("{} 执行异常： {}", name, error));
        }

        ToolResponse::OuterTool {
            name: self.name.clone(),
            output: text,
        }
    }
}

impl Default for Outer {
    fn default() -> Self {
        let mut map = HashMap::new();
        map.insert("Paramter name".to_string(), Parameters::default());
        Self {
            name: "TOOL NAME".to_string(),
            description: "TOOL DESCRIPTION".to_string(),
            enable: true,
            exec: vec!["YOUR EXEC COMMADN".to_string()],
            required: Vec::new(),
            parameters: Some(map),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct Parameters {
    #[serde(rename = "type")]
    pub(crate) parameters_type: String,
    pub(crate) description: String,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            parameters_type: "string".to_string(),
            description: "YOUR PARAMTERS DESCRIPTION".to_string(),
        }
    }
}
