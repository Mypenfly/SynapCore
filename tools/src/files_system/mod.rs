use std::{
    fmt::Display,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};
mod error;
use error::FileSystemErr;
use serde::{Deserialize, Serialize};

use crate::{
    define_call::tool_define::{FunctionDefinition, Tool, ToolDefinition},
    tool_response::ToolResponse,
};

#[derive(Default, Serialize, Deserialize, Debug)]
struct Args {
    command: String,
    path: String,
    pattern: Option<String>,
    depth: Option<usize>,
    target_path: Option<String>,
}

pub struct FileSystem {
    sand_box: PathBuf,
}

impl Tool for FileSystem {
    fn definition(&self) -> crate::define_call::tool_define::ToolDefinition {
        let name = "files_system".to_string();
        let description = format!(
            "文件系统操作,部分操作如rm的路径和cp的路径只能局限在沙盒中。当前沙盒路径为：{},(注意：这个文件系统是给agent做的简易系统，不是一个个完整的shell命令，请严格按照说明使用)",
            &self.sand_box.display()
        );

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
                    "description":"grep时的匹配内容，在grep是此项是必须的。支持文件后缀检索，如:.md,但不支持正则（\\.md$）"
                },
                "depth":{
                    "type":"number",
                    "description":"ls,grep时的检索递归深度，默认是1"
                },
                "target_path":{
                    "type":"string",
                    "description":"cp时的目标路径，需要在沙盒内,可以是目录（为目录时则与原文件同名，非目录是使用取得名字）"
                }
            },
            "required":["command","path"]
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
        // println!("{:#?}", &function);
        let arguments = match &function.arguments {
            Some(s) => s,
            None => return ToolResponse::Error("function files_system lack arguments".to_string()),
        };

        let result: Result<Args, serde_json::Error> = serde_json::from_str(arguments);
        if let Err(e) = result {
            return ToolResponse::Error(format!("function files_system failed : {}", e));
        }
        let args = result.unwrap();
        // println!("{:#?}", &args);
        let response = match self.command(&args) {
            Ok(s) => s,
            Err(e) => return ToolResponse::Error(format!("function files_system failed:{}", e)),
        };

        ToolResponse::FileSystem(response)
    }
}

impl FileSystem {
    ///新建
    pub(crate) fn new(sand_path: &Path) -> Self {
        let path = shellexpand::tilde(sand_path.to_str().unwrap_or("./"));
        let sand_box = PathBuf::from(path.as_ref());
        let sand_box = std::fs::canonicalize(sand_box).unwrap_or_default();

        Self { sand_box }
    }
    ///命令执行
    fn command(&self, args: &Args) -> Result<String, FileSystemErr> {
        let path_cow = shellexpand::tilde(&args.path);
        // println!("path_cow:{:#?}",&path_cow);
        let root_path = PathBuf::from(path_cow.as_ref());
        let root = std::fs::canonicalize(root_path).unwrap_or_default();
        // println!("root:{}", &root.display());

        let depth = args.depth.unwrap_or(1);

        match args.command.as_str() {
            "ls" => Ok(FileSystem::ls(&root, depth)?.to_string()),
            "grep" => {
                let pattern = match &args.pattern {
                    Some(p) => p,
                    None => return Ok("grep lack an argument pattern".to_string()),
                };
                let mut res = String::new();
                FileSystem::grep(&root, pattern, depth)?
                    .iter()
                    .for_each(|m| res.push_str(&m.to_string()));
                Ok(res)
            }
            "cp" => {
                let target_path = match &args.target_path {
                    Some(s) => s,
                    None => return Ok("参数缺失:target_path".to_string()),
                };
                let target_cow = shellexpand::tilde(&target_path);
                let target_path = PathBuf::from(target_cow.as_ref());
                // println!("target_path:{}",target_path.display());
                //如果target是不存在，需要先创建
                if !target_path.exists() {
                    if target_path.is_dir() {
                        let _ = std::fs::create_dir_all(&target_path);
                    } else {
                        // println!("target:{}",target.display());
                        let parent = target_path.parent().unwrap();
                        let _ = std::fs::create_dir_all(parent);

                        let _ = std::fs::File::create_new(&target_path);
                    }
                }
                let target = std::fs::canonicalize(target_path).unwrap_or_default();

                Ok(self.cp(&root, &target))
            }
            "rm" => Ok(self.rm(&root)),
            _ => Ok(format!("command :{}not found", &args.command)),
        }
    }

    ///ls实现
    fn ls(root: &PathBuf, depth: usize) -> Result<EntryDetil, FileSystemErr> {
        use walkdir::WalkDir;
        // let mut detil = EntryDetil::default();
        let mut detil = EntryDetil {
            root: root.clone(),
            trees: Vec::new(),
        };
        // detil.root = root.clone();

        let walker = WalkDir::new(root).max_depth(depth).into_iter();

        for entry in walker {
            let entry = entry.map_err(FileSystemErr::Walk)?;
            let path = entry.path();
            if path.is_dir() {
                detil.build_tree(path);
                continue;
            }

            if path.is_file() {
                detil.add_in_tree(path.parent().unwrap(), path);
            }
        }

        Ok(detil)
    }

    pub(crate) fn grep(
        root: &PathBuf,
        pattern: &str,
        depth: usize,
    ) -> Result<Vec<MathDetil>, FileSystemErr> {
        let pattern = pattern.to_lowercase();
        let mut list = Vec::new();

        let walker = walkdir::WalkDir::new(root).max_depth(depth).into_iter();

        for entry in walker {
            let mut detil = MathDetil::default();
            let entry = entry.map_err(FileSystemErr::Walk)?;
            let path = entry.path();
            if path.is_dir() {
                continue;
            }
            let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("./");
            if name.to_lowercase().contains(&pattern) {
                detil.file = path.to_path_buf();
            }

            let file = std::fs::File::open(path).map_err(FileSystemErr::Fs)?;

            let reader = BufReader::new(file);
            let lines: Vec<LineContent> = reader
                .lines()
                .enumerate()
                .filter_map(|(i, line)| {
                    let line = line.ok()?;
                    if line.to_lowercase().contains(&pattern) {
                        Some(LineContent {
                            num: i + 1,
                            content: line,
                        })
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            if !lines.is_empty() {
                detil = MathDetil {
                    file: path.to_path_buf(),
                    lines,
                };
            }

            list.push(detil);
        }
        // Ok(())
        Ok(list)
    }

    ///复制
    fn cp(&self, raw: &PathBuf, target: &PathBuf) -> String {
        if raw.is_dir() {
            return format!("{}是文件夹，不支持", raw.display());
        }
        if !target.starts_with(&self.sand_box) {
            return format!(
                "target{} 不在沙盒中({})",
                target.display(),
                self.sand_box.display()
            );
        }
        let target = if target.is_dir() {
            let name = raw
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("cp_err.txt");
            &target.join(name)
        } else {
            target
        };

        match std::fs::copy(raw, target) {
            Ok(bytes) => format!(
                "文件复制成功：{}->{}({}bytes)",
                raw.display(),
                target.display(),
                bytes
            ),
            Err(e) => format!(
                "文件复制失败:{} !=> {}(error:{})",
                raw.display(),
                target.display(),
                e
            ),
        }
    }

    ///rm
    fn rm(&self, path: &PathBuf) -> String {
        if path.is_dir() {
            return format!("{}是文件夹，不支持", path.display());
        }
        if !path.starts_with(&self.sand_box) {
            return format!("{} 不在沙盒({})中", path.display(), self.sand_box.display());
        }

        match std::fs::remove_file(path) {
            Ok(_) => format!("{} 已经删除", path.display()),
            Err(e) => format!("{} 删除失败 (error:{})", path.display(), e),
        }
    }
}

#[derive(Default)]
struct EntryDetil {
    root: PathBuf,
    trees: Vec<EntryTree>,
}

impl Display for EntryDetil {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut content = format!("EntryDetils in root:{}\n", self.root.display());
        for tree in &self.trees {
            content.push_str(&tree.to_string());
        }
        write!(f, "{}", content)
    }
}

impl EntryDetil {
    fn build_tree(&mut self, dir: &Path) {
        let tree = EntryTree {
            dir: dir.to_path_buf(),
            files: Vec::new(),
        };
        self.trees.push(tree);
    }

    fn add_in_tree(&mut self, dir: &Path, file: &Path) {
        let tree = match self.trees.iter_mut().find(|t| t.dir == *dir) {
            Some(t) => t,
            None => return,
        };

        tree.files.push(file.to_path_buf());
    }
}
#[derive(Default, Clone)]
struct EntryTree {
    dir: PathBuf,
    files: Vec<PathBuf>,
}

impl Display for EntryTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut content = format!("\n📂 {}\n", self.dir.display());
        for file in &self.files {
            content.push_str(&format!("\t📄 {}\n", file.display()));
        }
        write!(f, "{}", content)
    }
}

#[derive(Default)]
pub(crate) struct MathDetil {
    file: PathBuf,
    lines: Vec<LineContent>,
}

struct LineContent {
    num: usize,
    content: String,
}

// impl Display for LineContent {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(f,"line:{}\t{}\n",self.num,&self.content)
//     }
// }

impl Display for MathDetil {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut content = format!("path:{}\n", self.file.display());
        if !self.lines.is_empty() {
            content = "\n|  line  |--------content--------|\n\n".to_string();
        }
        for line in &self.lines {
            content.push_str(&format!("{}\t{}\n", line.num, &line.content));
        }

        write!(f, "{}", content)
    }
}
