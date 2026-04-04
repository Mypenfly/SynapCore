
use serde::{Deserialize, Serialize};

use crate::{define_call::tool_call::Function, tool_response::ToolResponse};

#[derive(Deserialize,Debug,Default,Serialize,Clone)]
pub struct ToolDefinition{
    #[serde(rename="type")]
    pub tool_type:String, //"function"
    pub function:FunctionDefinition,
}

#[derive(Debug,Clone,Default,Serialize,Deserialize)]
pub struct FunctionDefinition {
    pub name:String,
    pub description: String,
    pub parameters:serde_json::Value
}


pub trait Tool {
    fn definition(&self)->ToolDefinition ;
    fn execute(self,function:&Function)->impl  Future<Output = ToolResponse> + Send ; 
        
    
}
