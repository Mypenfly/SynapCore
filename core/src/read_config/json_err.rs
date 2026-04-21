use std::fmt::Display;

#[derive(Debug)]
pub enum JsonErr {
    Read(std::io::Error),
    Convert(serde_json::Error),
    FileOpen(std::io::Error),
    GetConfig(String),
}

impl Display for JsonErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JsonErr::Read(e) => write!(f, "读取失败：{}", e),
            JsonErr::Convert(e) => write!(f, "json转化失败：{}", e),
            JsonErr::FileOpen(e) => write!(f, "文件打开失败：{}", e),
            JsonErr::GetConfig(msg) => write!(f, "无法检索并返回目标配置:{}", msg),
        }
    }
}
impl std::error::Error for JsonErr {}

pub(crate) type JsonResult<T> = Result<T, JsonErr>;
