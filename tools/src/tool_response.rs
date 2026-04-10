use std::fmt::Display;

use crate::{files_extract::ExtractRes, web_search::SearchValue};

#[derive(Debug)]
pub enum ToolResponse {
    Extract(Vec<ExtractRes>),
    Write{path:String,content:String},
    WebSearch(Vec<SearchValue>),
    FileSystem(String),
    FetchUrl{url:String,content:String},
    Error(String)
}

impl Display for ToolResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Extract(list)=>{
                let mut content = String::new();
                list.iter().for_each(|v|{
                    content.push_str(&v.to_string());
                });
                write!(f,"{}",content)
            },
            Self::Write { path, content }=>write!(f,"{} :\n{}",path,content),
            Self::WebSearch(list)=>{
                
                    let mut content = String::new();
                    list.iter().for_each(|v| {
                        content.push_str(&format!(
                            "id:{},url:{}\ntitle:{}\nsnippet:{}\nsummary:{}\nsite:{},publishData:{},updataData:{}\n\n",
                            v.id,
                            v.url,
                            v.name,
                            v.snippet,
                            v.summary.clone().unwrap_or_default(),
                            v.site_name,
                            v.data_last_published.clone().unwrap_or_default(),
                            v.data_last_crawled.clone().unwrap_or_default()
                        ));
                    });
                    write!(f,"{}",content)
            }
            Self::FileSystem(s)=>write!(f,"{}",s),
            Self::FetchUrl { url, content }=>write!(f,"{} :\n{}",url,content),
            Self::Error(e)=> write!(f,"{}",e)
        }
    }
}
