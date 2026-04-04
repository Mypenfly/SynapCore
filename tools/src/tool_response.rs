use crate::web_search::SearchValue;

#[derive(Debug)]
pub enum ToolResponse {
    Extract(String),
    Write(String),
    WebSearch(Vec<SearchValue>),
    Error(String)
}
