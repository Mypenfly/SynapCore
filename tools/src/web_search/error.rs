use thiserror::Error;
#[derive(Debug,Error)]
pub enum WebSearchErr {
    #[error("网络错误:{0}")]
    Network(reqwest::Error),
    #[error("Api failed:({code}){message}")]
    Api{code:usize,message:String},
    #[error("Analyze failed:{0}")]
    Analyze(reqwest::Error),
    #[error("Serde failed:{0}")]
    Serde(serde_json::Error),
}
