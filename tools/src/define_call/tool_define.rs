
use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::{define_call::tool_call::Function, tool_response::ToolResponse};

#[derive(Deserialize,Debug,Default,Serialize,Clone)]
pub struct ToolDefinition{
    #[serde(rename="type")]
    pub tool_type:String, //"function"
    pub function:FunctionDefinition,
}

impl Display for ToolDefinition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let content =serde_json::to_string_pretty(self).unwrap_or_default() ;
        write!(f,"{}",content)
    }
}

#[derive(Debug,Clone,Default,Serialize,Deserialize)]
pub struct FunctionDefinition {
    pub name:String,
    pub description: String,
    pub parameters:serde_json::Value
}


pub trait Tool {
    fn definition(&self)->ToolDefinition ;
    fn execute(&self,function:&Function)->impl  Future<Output = ToolResponse> + Send ; 
        
    
}
