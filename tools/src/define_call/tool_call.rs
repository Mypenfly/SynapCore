use serde::{Deserialize, Serialize};

#[derive(Debug,Default,PartialEq,Serialize,Deserialize,Clone)]
pub struct ToolCall{
    pub id:Option<String>,
    pub index:usize,
    #[serde(rename="type")]
    pub tool_type:Option<String>,
    pub function: Function,
}

#[derive(Debug,Default,PartialEq, Serialize,Deserialize,Clone)]
pub struct Function{
    pub name:Option<String>,
    pub arguments: Option<String>,
}

