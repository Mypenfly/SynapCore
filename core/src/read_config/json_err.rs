use std::fmt::Display;

#[derive(Debug)]
pub enum JsonErr {
    ReadError(std::io::Error),
    ConvertError(serde_json::Error),
    FileOpenError(std::io::Error),
    GetConfigError(String)
}

impl Display for JsonErr  {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JsonErr::ReadError(e) => write!(f,"读取失败：{}",e),
            JsonErr::ConvertError(e) => write!(f,"json转化失败：{}",e),
            JsonErr::FileOpenError(e) => write!(f,"文件打开失败：{}",e),
            JsonErr::GetConfigError(msg) => write!(f,"无法检索并返回目标配置:{}",msg),
        }
    }
}
impl std::error::Error for JsonErr {
    
}

pub(crate) type JsonResult<T> = Result<T, JsonErr>;

