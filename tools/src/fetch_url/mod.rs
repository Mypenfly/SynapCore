use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::{
    define_call::tool_define::{FunctionDefinition, Tool, ToolDefinition},
    fetch_url::error::FetchErr,
    tool_response::ToolResponse,
};

mod error;

#[derive(Debug, Serialize, Deserialize, Default)]
struct Args {
    url: String,
}

pub(crate) struct FetchUrl {}

impl Tool for FetchUrl {
    fn definition(&self) -> crate::define_call::tool_define::ToolDefinition {
        let name = "fetch_url".to_string();
        let description = "对指定的url的网页内容抓取，如文章内容,最后输出的是纯文本".to_string();

        let parameters = serde_json::json!({
            "type":"object",
            "properties":{
                "url":{
                    "type":"string",
                    "description":"指定的url"
                }
            },
            "required":["url"]
        });

        let function = FunctionDefinition {
            name,
            description,
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
        let arguments = match &function.arguments {
            Some(s) => s,
            None => return ToolResponse::Error("Function fetch_url lacks arguments".to_string()),
        };

        let args: Args = serde_json::from_str(arguments).unwrap_or_default();

        match FetchUrl::fetch(&args).await {
            Ok(s) => ToolResponse::FetchUrl {
                url: args.url.clone(),
                content: s,
            },
            Err(e) => ToolResponse::Error(e.to_string()),
        }
    }
}

impl FetchUrl {
    ///执行抓取
    async fn fetch(args: &Args) -> Result<String, FetchErr> {
        let client = Client::new();

        let url = &args.url;

        let response = client.get(url).send().await.map_err(FetchErr::NetWork)?;

        let status = response.status();
        let text = response.text().await.map_err(FetchErr::NetWork)?;
        if !status.is_success() {
            return Err(FetchErr::Url {
                code: status.as_u16() as usize,
                message: text,
            });
        }
        let content = html2text::from_read(text.as_bytes(), 80);

        Ok(content)
    }
}
