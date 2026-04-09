use std::{fmt::Display,io::{BufRead, BufReader}, path::PathBuf};
mod error;
use error::FileSystemErr;
use pdf_extract::Path;
use serde::{Deserialize, Serialize};

use crate::{define_call::tool_define::{FunctionDefinition, Tool, ToolDefinition}, tool_response::ToolResponse};


#[derive(Default,Serialize,Deserialize,Debug)]
struct Args{
    command:String,
    path:String,
    pattern:Option<String>,
    depth:Option<usize>,
    target_path:Option<String>
}

pub struct FileSystem{
    sand_box:PathBuf,
}

impl Tool for FileSystem {
    fn definition(&self)->crate::define_call::tool_define::ToolDefinition {
        let name ="files_system".to_string() ;
        let description = format!("文件系统操作,部分操作如rm的路径和cp的路径只能局限在沙盒中。当前沙盒路径为：{}",&self.sand_box.display());

        let parameters = serde_json::json!({
            "type":"object",
            "properties":{
                "command":{
                    "type":"string",
                    "description":"
                    命令类型，支持命令有:
                    ls,grep,rm,cp,
                    (rm,cp只能接受文件路径,ls,grep能接受目录)
                    "
                },
                "path":{
                    "type":"string",
                    "description":"操作的指定目标路径，也是cp的原始路径,rm时该项需要在沙盒内"
                },
                "pattern":{
                    "type":"string",
                    "description":"grep时的匹配内容，在grep是此项是必须的"
                },
                "depth":{
                    "type":"number",
                    "description":"ls,grep时的检索递归深度，默认是0"
                },
                "target_path":{
                    "type":"string",
                    "description":"cp时的目标路径，需要在沙盒内"
                }
            },
            "required":["command","path"]
        });

        let function = FunctionDefinition{
            name,description,parameters
        };

        ToolDefinition{
            tool_type:"function".to_string(),
            function
        }
    }

    async fn execute(self,function:&crate::define_call::tool_call::Function)->crate::tool_response::ToolResponse {
        println!("{:#?}",&function);
        let arguments =match &function.arguments {
            Some(s)=>s,
            None => return ToolResponse::Error("function files_system lack arguments".to_string())
        } ;

        let args:Args = serde_json::from_str(arguments).unwrap_or_default();
        println!("{:#?}",&args);
        let response = match self.command(&args) {
            Ok(s)=>s,
            Err(e)=> return ToolResponse::Error(format!("function files_system failed:{}",e)),
        };

        ToolResponse::FileSystem(response)


    }
}



impl FileSystem {
    
    ///新建
    pub(crate) fn new(sand_path:&PathBuf)->Self {
        // let path = shellexpand::tilde(sand_path.to_str().unwrap_or("./"));
        // let sand_box = PathBuf::from(path.as_ref());
        let sand_box = std::fs::canonicalize(sand_path).unwrap_or_default();

        Self { sand_box }
    }
    ///命令执行
    fn command(&self,args:&Args) ->Result<String,FileSystemErr>{
    
        // let path_cow = shellexpand::tilde(&args.path);
        // println!("path_cow:{:#?}",&path_cow);
        // let root = PathBuf::from(path_cow.as_ref());
        let root = std::fs::canonicalize(&args.path).unwrap_or_default();
        println!("root:{}",&root.display());

        let depth = args.depth.unwrap_or(0); 

        match args.command.as_str() {
            "ls" => Ok(FileSystem::ls(&root, depth)?.to_string()),
            "grep"=> {
                let pattern = match &args.pattern {
                    Some(p)=>p,
                    None=> return Ok("grep lack an argument pattern".to_string())
                };
                let mut res = String::new();
                FileSystem::grep(&root, pattern, depth)?
                    .iter()
                    .for_each(|m|res.push_str(&m.to_string()));
                Ok(res)
            }
            "cp"=> {
                let target_path = match &args.target_path {
                    Some(s)=>s,
                    None => return Ok("参数缺失:target_path".to_string())
                };
                // let target_cow = shellexpand::tilde(&target_path);
                let target =std::fs::canonicalize(target_path).unwrap_or_default() ;
                Ok(self.cp(&root, &target))
            }
            "rm"=> {
                Ok(self.rm(&root))
            }
            _=> Ok(format!("command :{},not found",&args.command))
        }

    }
    
    ///ls实现
    fn ls(root:&PathBuf,depth:usize) ->Result<EntryDetil,FileSystemErr> {
        use walkdir::WalkDir;
        // let mut detil = EntryDetil::default();
        let mut detil =EntryDetil::new(root.clone()) ;
        // detil.root = root.clone();

        let walker =WalkDir::new(root).max_depth(depth).into_iter() ;

        for entry in walker {
            let entry =entry.map_err(FileSystemErr::Walk)? ;
            let path = entry.path();
            if path.is_dir() {
                detil.dirs.push(path.to_path_buf());
            }

            if path.is_file() {
                detil.files.push(path.to_path_buf());
            }
        }
    
    
        Ok(detil)
    }

    fn grep(root:&PathBuf,pattern:&str,depth:usize)->Result<Vec<MathDetil>,FileSystemErr> {
        let mut list = Vec::new();

        let walker = walkdir::WalkDir::new(root).max_depth(depth).into_iter();

        for entry in walker {
            let entry = entry.map_err(FileSystemErr::Walk)?;
            let path = entry.path();
            if path.is_dir() {
                continue;
            }

            let file = std::fs::File::open(path).map_err(FileSystemErr::Fs)?;

            let reader = BufReader::new(file);
            let lines:Vec<LineContent> = reader
                .lines()
                .enumerate()
                .filter_map(|(i,line)|{
                    let line = line.ok()?;
                    if line.contains(pattern) {
                        Some(LineContent { num: i + 1, content: line })
                    }else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            if lines.is_empty() {
                continue;
            }

            let detil = MathDetil{
                file:path.to_path_buf(),
                lines
            };
            list.push(detil);
            
        }
        // Ok(())
        Ok(list)
    }

    ///复制
    fn cp(&self,raw:&PathBuf,target:&PathBuf)->String {
        if raw.is_dir() {
            return format!("{}是文件夹，不支持",raw.display());
        }
        if !target.starts_with(&self.sand_box) {
            return format!("target{} 不在沙盒中({})",target.display(),self.sand_box.display());
        }
        
        match std::fs::copy(raw, target) {
            Ok(bytes) => format!("文件复制成功：{}->{}({}bytes)",raw.display(),target.display(),bytes),
            Err(e) => format!("文件复制失败:{} !=> {}(error:{})",raw.display(),target.display(),e.to_string())
        }
    }

    ///rm
    fn rm(&self,path:&PathBuf)->String {
        
        if path.is_dir() {
            return format!("{}是文件夹，不支持",path.display());
        }
        if !path.starts_with(&self.sand_box){
            return format!("{} 不在沙盒({})中",path.display(),self.sand_box.display());
        }

        match std::fs::remove_file(path) {
            Ok(_) => format!("{} 已经删除",path.display()),
            Err(e) =>format!("{} 删除失败 (error:{})",path.display(),e.to_string())
        }
    }
}

#[derive(Default)]
struct EntryDetil{
    root:PathBuf,
    files:Vec<PathBuf>,
    dirs:Vec<PathBuf>
}

impl EntryDetil {
    fn new(root: PathBuf) -> Self {
        let files =Vec::new() ;
        let dirs = Vec::new();
        Self { root, files, dirs }
    }

    ///处理dir遍历输出格式化
    fn walk_dir(&self,dir:&PathBuf)->(String,Vec<&PathBuf>) {
        
        let mut content = String::new();
        // let mut sub_dirs_buf = Vec::new();
        
        content.push_str(&format!("\t=>{}\n",dir.to_str().unwrap_or("error:Unkown")));
        
        let sub_files:Vec<&PathBuf> =self.files
            .iter()
            .filter(|f|f.parent().unwrap_or(&PathBuf::default()) == dir)
            .collect() ;
        // write!(f,"{}\n",dir.to_str().unwrap_or("error:unkown"))?;
        for sub_file in sub_files {
            // write!(f,"\t->{}\n",sub_file.to_str().unwrap_or("error:unkown"));
            content.push_str(&format!("\t\t->{}\n",sub_file.to_str().unwrap_or("error:Unkown")));
        }

        let sub_dirs = self.dirs
            .iter()
            .filter(|f|f.starts_with(dir))
            .collect();
        // Ok(())
        (content,sub_dirs)
    }
}

impl Display for EntryDetil {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut content = String::new();
        let mut sub_dirs_buf = Vec::new();
        for dir in &self.dirs {
            if sub_dirs_buf.contains(&dir) {
                continue;
            }

            let (res,dirs) = self.walk_dir(dir);
            content.push_str(&res);

            if dirs.is_empty() {
                break;
            }
            
            for sub_dir in &dirs {
                let (res,_) =self.walk_dir(sub_dir) ;
                content.push_str(&res);    
            }
            sub_dirs_buf.extend(dirs);
            
        }
        // Ok(())
        write!(f,"{}:\n{}",&self.root.display(),content)
    }
}

#[derive(Default)]
struct MathDetil{
    file:PathBuf,
    lines:Vec<LineContent>
}

struct LineContent{
    num:usize,
    content:String
}

// impl Display for LineContent {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(f,"line:{}\t{}\n",self.num,&self.content)
//     }
// }

impl Display for MathDetil {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut content ="\n|  line  |--------content--------|\n\n".to_string() ;

        for line in &self.lines {
            content.push_str(&format!("{}\t{}\n",line.num,&line.content));
        }
        
     write!(f,"file:{}\n{}\n",&self.file.display(),content)   
    }
}
