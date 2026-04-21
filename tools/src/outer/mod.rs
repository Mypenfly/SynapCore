use std::sync::Arc;

use crate::{
    define_call::{
        tool_call::Function,
        tool_define::{Tool, ToolDefinition},
    },
    outer::config::Outer,
    tool_response::ToolResponse,
};

pub(crate) mod config;

///outertools对每个外部工具进行识别和分流处理，数据拿指针，不可变
#[derive(Debug)]
pub(crate) struct OuterTools {
    pub(crate) outers: Arc<Vec<Outer>>,
}

impl OuterTools {
    pub(crate) async fn execute(&self, function: &Function) -> ToolResponse {
        let name = function.name.as_ref().unwrap();

        let outer = self.outers.iter().find(|v| v.name == *name);

        if outer.is_none() {
            return ToolResponse::Error(format!("unkown tool : {}", name));
        }

        outer.unwrap().execute(function).await
    }
    pub(crate) fn defination(&self) -> Vec<ToolDefinition> {
        let mut list = Vec::new();

        if self.outers.is_empty() {
            return list;
        }

        for outer in self.outers.iter() {
            if !outer.enable {
                continue;
            }
            list.push(outer.definition());
        }
        list
    }
}
