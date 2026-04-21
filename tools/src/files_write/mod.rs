use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    define_call::tool_define::{FunctionDefinition, Tool, ToolDefinition},
    tool_response::ToolResponse,
};

#[derive(Default, Serialize, Deserialize)]
struct Args {
    content: String,
    path: String,
}

pub(crate) struct FileWriter {
    pub(crate) sand_box: PathBuf,
}

impl Tool for FileWriter {
    fn definition(&self) -> crate::define_call::tool_define::ToolDefinition {
        let name = "files_write".to_string();
        let description = format!(
            "根据路径写入指定文件(只能写入目标沙盒路径),当前沙盒路径:{}",
            &self.sand_box.display()
        );

        let parameters = serde_json::json!({
            "type":"object",
            "properties":{
                "path":{
                    "type":"string",
                    "description":"写入目标文件路径"
                },
                "content":{
                    "type":"string",
                    "description":"写入的内容"
                }
            },
            "required":["path","content"]
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

    async fn execute(&self, function: &crate::define_call::tool_call::Function) -> ToolResponse {
        let arguments = match &function.arguments {
            Some(s) => s,
            None => return ToolResponse::Error("lack arguments".to_string()),
        };

        let args: Args = serde_json::from_str(arguments).unwrap_or_default();

        let path_cow = shellexpand::tilde(&args.path);
        let path = PathBuf::from(path_cow.as_ref());
        if self.write(&path, &args.content).await.is_none() {
            return ToolResponse::Error(format!(
                "Function files_write failed in path :{}\n\n",
                &args.path
            ));
        }

        let response = format!("Function files_write success:\n{}\n\n", &args.content);
        ToolResponse::Write {
            path: args.path.clone(),
            content: response,
        }
    }
}

impl FileWriter {
    pub(crate) fn new(sand_path: &Path) -> Self {
        let path_cow = shellexpand::tilde(sand_path.to_str().unwrap_or("./"));
        let path = PathBuf::from(path_cow.as_ref());
        let sand_box = std::fs::canonicalize(path).unwrap_or_default();

        Self { sand_box }
    }

    async fn write(&self, path: &PathBuf, content: &str) -> Option<()> {
        if self.is_within(path) {
            FileWriter::into(path, content).await?;
        } else {
            let pending_path = self
                .sand_box
                .join("pending")
                .join(path.strip_prefix("/").unwrap_or(path));

            FileWriter::into(&pending_path, content).await?;
        }

        Some(())
    }

    fn is_within(&self, path: &Path) -> bool {
        let path_str = match path.to_str() {
            Some(s) => s,
            None => return false,
        };
        let path_cow = shellexpand::tilde(path_str);

        let path = PathBuf::from(path_cow.as_ref());
        if !path.exists() {
            let parent = path.parent().unwrap();
            let _ = std::fs::create_dir_all(parent);
            let _ = std::fs::File::create_new(&path);
        }

        let canonical = match fs::canonicalize(path) {
            Ok(p) => p,
            Err(_) => return false,
        };

        if canonical.starts_with("./") {
            return true;
        }

        canonical.starts_with(&self.sand_box)
    }

    async fn into(path: &PathBuf, content: &str) -> Option<()> {
        use tokio::fs;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.ok()?
        }

        fs::write(path, content).await.ok()?;

        Some(())
    }
}
