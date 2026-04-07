pub(crate) mod error;
use std::collections::HashMap;

use error::WebSearchErr;
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
// use serde_json::Value;

use crate::{
    define_call::tool_define::{FunctionDefinition, Tool, ToolDefinition},
    tool_response::ToolResponse,
};

pub type WebSearchResult<T> = Result<T, WebSearchErr>;

///参数设置
#[derive(Serialize, Deserialize, Debug, Default)]
struct Args {
    query: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    freshness: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    count: Option<usize>,
}

impl Args {
    fn check_none(&mut self) {
        if self.summary.is_none() {
            self.summary = Some(true);
        }
        if self.freshness.is_none() {
            self.freshness = Some("noLimit".to_string());
        }
        if self.count.is_none() {
            self.count = Some(10);
        }
    }
}

///联网搜索
#[derive(Default)]
pub struct WebSearch {
    pub params: HashMap<String, String>,
}

impl Tool for WebSearch {
    fn definition(&self) -> crate::define_call::tool_define::ToolDefinition {
        let parameters = serde_json::json!({
            "type":"object",
            "properties":{
                "query":{
                    "type":"array",
                    "description":"你要搜索的问题，将会调用一次搜索的api"
                },
                "summary":{
                    "type":"bool",
                    "description":"是否显示文本摘要(由博查提供)，默认是true"
                },
                "freshness":{
                    "type":"string",
                    "description":"搜索指定时间范围内的网页。
                                可填值：
                                - noLimit，不限（默认）
                                - oneDay，一天内
                                - oneWeek，一周内
                                - oneMonth，一个月内
                                - oneYear，一年内
                                - YYYY-MM-DD..YYYY-MM-DD，搜索日期范围，例如：\"2025-01-01..2025-04-06\"
                                - YYYY-MM-DD，搜索指定日期，例如：\"2025-04-06\"
                                "
                },
                "count":{
                    "type":"number",
                    "description":"
                    返回结果的条数（实际返回结果数量可能会小于count指定的数量）。
                    - 可填范围：1-50，最大单次搜索返回50条
                    - 默认为10
                    "
                }
            },
            "required":["query"]
        });

        let function = FunctionDefinition {
            name: "web_search".to_string(),
            description: "联网搜索工具".to_string(),
            parameters,
        };

        ToolDefinition {
            tool_type: "function".to_string(),
            function,
        }
    }
    async fn execute(
        self,
        function: &crate::define_call::tool_call::Function,
    ) -> crate::tool_response::ToolResponse {
        let arguments = match &function.arguments {
            Some(s) => s,
            None => return ToolResponse::Error("lack arguments".to_string()),
        };
        let mut args: Args = serde_json::from_str(arguments).unwrap_or_default();
        args.check_none();
        let responsse = self.send(&args).await;

        if let Err(e) = responsse {
            return ToolResponse::Error(format!("Function web_search failed :\n{}\n", e));
        }

        ToolResponse::WebSearch(responsse.unwrap_or_default())
    }
}

impl WebSearch {
    async fn send(&self, args: &Args) -> WebSearchResult<Vec<SearchValue>> {
        let client = Client::new();

        let url = self.params.get("base_url").unwrap();
        let api_key = self.params.get("api_key").unwrap();
        // println!("url:{}",url);

        let response = client
            .post(url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(args)
            .send()
            .await
            .map_err(WebSearchErr::Network)?;
        let status = response.status();

        if !status.is_success() {
            let text = response.text().await.map_err(WebSearchErr::Network)?;
            return Err(WebSearchErr::Api {
                code: status.as_u16() as usize,
                message: text,
            });
        }

        // println!("{:#?}", &response);
        let list = self.get_response(response).await?;
        Ok(list)
    }

    ///解析响应
    async fn get_response(&self, response: Response) -> WebSearchResult<Vec<SearchValue>> {
        let bytes = response.bytes().await.map_err(WebSearchErr::Analyze)?;

        let raw = String::from_utf8_lossy(&bytes);

        // let path =PathBuf::from("./test.json") ;
        // let _ =fs::File::create_new(&path) ;
        // let _ =fs::write(path, raw.to_string()) ;
        //  println!("===============raw================\n{:#?}\n",raw);
        // println!("================serde==================\n");
        let content: SearchResult = serde_json::from_str(&raw).map_err(WebSearchErr::Serde)?;

        Ok(content.data.web_pages.value)
    }

    // fn build_body(&self,args:&Args) ->serde_json::Map<String,Value> {
    //     use serde_json::{Map,json};
    //     let mut body_map =Map::new() ;
    //     body_map.insert("query".to_string(), json!(args.query));
    //     body_map.insert("summary".to_string(), json!(args.summary));
    //     body_map.insert("freshness".to_string(), json!(args.freshness));
    //     body_map.insert("count".to_string(), json!(args.count));

    //     body_map
    // }
}

#[derive(Serialize, Deserialize, Debug)]
struct SearchResult {
    code: usize,
    log_id: String,
    msg: Option<String>,
    data: Data,
}

#[derive(Serialize, Deserialize, Debug)]
struct Data {
    #[serde(rename = "_type")]
    data_type: String,
    #[serde(rename = "webPages")]
    web_pages: WebPages,
}

#[derive(Serialize, Deserialize, Debug)]
struct WebPages {
    #[serde(rename = "webSearchUrl")]
    web_search_url: String,
    #[serde(rename = "totalEstimatedMatches")]
    total_estimated_matches: usize,
    value: Vec<SearchValue>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct SearchValue {
    pub id: String,
    pub name: String,
    pub url: String,
    #[serde(rename = "displayUrl")]
    pub display_url: String,
    pub snippet: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(rename = "siteName")]
    pub site_name: String,
    #[serde(rename = "siteIcon")]
    pub site_icon: String,
    #[serde(rename = "datePublished")]
    pub data_last_published: String,
    #[serde(rename = "dateLastCrawled")]
    pub data_last_crawled: String,
}
