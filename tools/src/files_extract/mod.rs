use std::{fmt::Display, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::{define_call::{tool_call::Function, tool_define::{FunctionDefinition, Tool, ToolDefinition}}, files_extract::extract_error::{ExtractErr, ExtractResult}, tool_response::ToolResponse};

pub mod extract_error;


#[derive(Debug,Serialize,Deserialize,Default)]
struct Args{
    path:Vec<String>,
}

pub struct ExtractTool;

impl Tool for ExtractTool {
    fn definition(&self)->crate::define_call::tool_define::ToolDefinition {
        let name ="files_extract".to_string() ;
        let description ="文件内容提取，支持pdf,docx,xml,和各类代码文件或纯文本文件"
            .to_string() ;

        let parameters = serde_json::json!({
            "type":"object",
            "properties":{
                "path":{
                    "type":"array",
                    "items": { "type":"string" },
                    "description":"要提取的文件路径列表"
                }
            },
            "required":["path"]
            
        });
        
        let function = FunctionDefinition{
            name,
            description,
            parameters
        };

        ToolDefinition{
            tool_type:"function".to_string(),
            function
        }
    }

    // fn execute(self,function:&Function)-> std::pin::Pin<Box<dyn Future<Output = String> + Send + '_>> {
    //     Box::pin(async move {
            
    //     match call_extract(function) {
    //         Ok(s) => s,
    //         Err(e) => format!("Function files_extract failed:{}",e)
    //     }
    //     })
    // }
    async fn execute(&self,function:&Function)-> ToolResponse {
    let arguments =match &function.arguments{
        Some(s) => s,
        None => return ToolResponse::Error("Function files_extract lacks arguments".to_string())
    };
    let args:Args =serde_json::from_str(arguments).unwrap_or_default();

    match extract(&args.path){
        Ok(list) => ToolResponse::Extract (list),
        Err(e)=>ToolResponse::Error(e.to_string())
    }
    }
}

///不可解析文件
const SKIP_EXT:&[&str] = &[
    "exe","dll","so","dylib","bin",
    "mp3","mp4","wav","avi","mkv","mox","flac","ogg",
    "zip","tar","gz","rar","7z","bz2","xz",
    "db","sqlite","sqlite3",
    "lock"
];

///二进制文件
const BINARY_EXT:&[&str] = &[
    "pdf","docx","xlsx","xls","ods"
];




///解析文件的入口
pub fn extract(files:&Vec<String>) ->ExtractResult<Vec<ExtractRes>> {
    let mut list = Vec::new();
    
    for file in files {
        let cow_path = shellexpand::tilde(file);
        let path = PathBuf::from(cow_path.as_ref());
        let ext = path.extension()
            .and_then(|e|e.to_str())
            .unwrap_or("txt");
        if SKIP_EXT.contains(&ext.to_lowercase().as_str()) {
            return Err(ExtractErr::Check(format!("不支持该类型:{}",ext)));
        }


        if BINARY_EXT.contains(&ext.to_lowercase().as_str()) {
            let result = match ext.to_lowercase().as_str() {
                "pdf" => extract_pdf(&path),
                "docx" => extract_docx(&path),
                "xml" => extract_xml(&path),
                _ => Err(ExtractErr::Check(format!("不支持该类型:{}",ext))),
            };
            list.push(ExtractRes { path, content: result? });
        }else {
            let result = extract_text(&path);
            list.push(ExtractRes { path, content: result? });
        }
                
    }
    
    Ok(list)
}

///pdf解析
fn extract_pdf(path:&PathBuf) ->ExtractResult<String> {
    use pdf_extract::extract_text;
    let text = extract_text(path).map_err(ExtractErr::Pdf)?;
    
    Ok(text)
}

///docx解析
fn extract_docx(path:&PathBuf) -> ExtractResult<String> {
    use docx_rs::{read_docx,DocumentChild,ParagraphChild,RunChild};

    let file = std::fs::read(path)
        .map_err(ExtractErr::File)?;
    let docx = read_docx(&file)
        .map_err(ExtractErr::Docx)?;

    let mut text = String::new();
    for child in &docx.document.children {
        if let DocumentChild::Paragraph(para) = child {
            for para_child in &para.children {
                if let ParagraphChild::Run(run) = para_child {
                    for run_child in &run.children {
                        if let RunChild::Text(t) = run_child {
                            text.push_str(&t.text);
                        }
                    }
                }
            }
            text.push('\n');
        }
    }
    
    Ok(text)
}

///xml解析
fn extract_xml(path:&PathBuf)-> ExtractResult<String> {
    use quick_xml::Reader;
    use quick_xml::events::Event;

    let mut reader = Reader::from_file(path)
        .map_err(|e|ExtractErr::Xml(e.to_string()))?;

    let mut text = String::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf)
            .map_err(|e|ExtractErr::Xml(e.to_string()))?{
            Event::Text(e) => {
                let content = e.unescape()
                    .map_err(|e|ExtractErr::Xml(e.to_string()))?;
                let trimmed = content.trim();

                if !trimmed.is_empty() {
                    text.push_str(trimmed);
                    text.push(' ');
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(text)
}

///其他文件--纯文本解析
fn extract_text(path:&PathBuf) ->ExtractResult<String> {
    let bytes = std::fs::read(path)
        .map_err(ExtractErr::File)?;

    if bytes.starts_with(&[0xEF,0xBB,0xBF]) {
        return Ok(String::from_utf8_lossy(&bytes[3..]).to_string());
    }

    Ok(String::from_utf8_lossy(&bytes).to_string())
}

#[derive(Debug)]
pub(crate) struct ExtractRes{
    path:PathBuf,
    content:String
}

impl Display for ExtractRes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f,"{} :\n{}\n",self.path.display(),self.content)
    }
}
