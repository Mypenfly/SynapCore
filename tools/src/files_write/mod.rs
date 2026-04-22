use std::{
    collections::VecDeque,
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
    #[serde(default)]
    action: String,
    #[serde(default)]
    line: usize,
    #[serde(default)]
    end_line: usize,
}

pub(crate) struct FileWriter {
    pub(crate) sand_box: PathBuf,
}

impl Tool for FileWriter {
    fn definition(&self) -> crate::define_call::tool_define::ToolDefinition {
        let name = "files_write".to_string();
        let description = format!(
            "根据路径写入或修改文件内容(只能操作目标沙盒路径)。支持三种操作模式：1) write: 覆盖写入整个文件 2) revise: 修改指定行 3) remove: 删除指定行范围。当前沙盒路径:{}",
            &self.sand_box.display()
        );

        let parameters = serde_json::json!({
            "type":"object",
            "properties":{
                "path":{
                    "type":"string",
                    "description":"目标文件路径"
                },
                "content":{
                    "type":"string",
                    "description":"要写入或修改的内容（write/revise模式需要），remove模式不需要此参数"
                },
                "action":{
                    "type":"string",
                    "description":"操作类型：write（覆盖写入）revise（修改指定行，支持将一行内容替换成多行）remove（删除行范围），默认为write",
                    "enum":["write","revise","remove"]
                },
                "line":{
                    "type":"integer",
                    "description":"要修改的行号（从1开始），在revise模式有效，在remove模式表示删除范围的起始行"
                },
                "end_line":{
                    "type":"integer",
                    "description":"删除范围的结束行号（从1开始），只在action=remove且需要删除多行时有效。如果未指定或为0，则只删除line指定的单行"
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

        let args: Args = match serde_json::from_str(arguments) {
            Ok(s) => s,
            Err(e) => return ToolResponse::Error(format!("Function files_write failed: {}", e)),
        };

        let path_cow = shellexpand::tilde(&args.path);
        let path = PathBuf::from(path_cow.as_ref());

        let action = if args.action.is_empty() {
            "write"
        } else {
            &args.action
        };

        match action {
            "write" => {
                if self.write(&path, &args.content).await.is_none() {
                    return ToolResponse::Error(format!(
                        "Function files_write failed in path: {}\n\n",
                        &args.path
                    ));
                }

                let response = format!("Function files_write success:\n{}\n\n", &args.content);
                ToolResponse::Write {
                    path: args.path.clone(),
                    content: response,
                }
            }
            "revise" => {
                if args.line == 0 {
                    return ToolResponse::Error(
                        "Line number must be >= 1 for revise action\n\n".to_string(),
                    );
                }
                let result = self.revise(&path, args.line, &args.content).await;
                match result {
                    Some((raw_content, new_content)) => {
                        //格式化输出
                        let new_content: Vec<String> =
                            new_content.lines().map(|l| format!("+ {}", l)).collect();

                        let response =
                            format!("- {}\n======>\n+ {}", raw_content, new_content.join("\n"));
                        ToolResponse::Write {
                            path: args.path.clone(),
                            content: response,
                        }
                    }
                    None => ToolResponse::Error(format!(
                        "Function files_write revise failed in path: {}, line: {}\n\n",
                        &args.path, args.line
                    )),
                }
            }
            "remove" => {
                if args.line == 0 {
                    return ToolResponse::Error(
                        "Line number must be >= 1 for remove action\n\n".to_string(),
                    );
                }
                let result = self.remove(&path, args.line, args.end_line).await;
                match result {
                    Some((start_line, end_line, removed_content)) => {
                        //格式化输出
                        let removed_content: Vec<String> = removed_content
                            .lines()
                            .map(|l| format!("- {}", l))
                            .collect();

                        let response = format!(
                            "(removed lines {} ~ {})\n{}",
                            start_line,
                            end_line,
                            removed_content.join("\n")
                        );
                        ToolResponse::Write {
                            path: args.path.clone(),
                            content: response,
                        }
                    }
                    None => ToolResponse::Error(format!(
                        "Function files_write remove failed in path: {}, line: {}-{}\n\n",
                        &args.path,
                        args.line,
                        if args.end_line > 0 {
                            args.end_line
                        } else {
                            args.line
                        }
                    )),
                }
            }
            _ => ToolResponse::Error(format!(
                "Unknown action: {}. Use 'write', 'revise' or 'remove'\n\n",
                action
            )),
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

    ///修改文件
    async fn revise(
        &self,
        path: &PathBuf,
        line_num: usize,
        content: &str,
    ) -> Option<(String, String)> {
        use tokio::fs;

        if line_num == 0 {
            return None;
        }

        // 读取文件内容
        let file_content = match fs::read_to_string(path).await {
            Ok(content) => content,
            Err(_) => {
                // 如果文件不存在，创建一个空文件
                FileWriter::into(path, "").await?;
                String::new()
            }
        };

        let mut lines: Vec<String> = file_content.lines().map(String::from).collect();
        let original_line_count = lines.len();

        // 获取原始行内容
        let raw_content = if line_num <= original_line_count {
            lines[line_num - 1].clone()
        } else {
            String::new()
        };

        // 处理行号范围
        if line_num > original_line_count {
            // 填充空行直到目标行
            while lines.len() < line_num - 1 {
                lines.push(String::new());
            }
            lines.push(content.to_string());
        } else {
            lines[line_num - 1] = content.to_string();
        }

        // 写入文件
        let output = lines.join("\n");
        FileWriter::into(path, &output).await?;

        // 返回原始内容和修改后内容
        Some((raw_content, content.to_string()))
    }

    ///删除文件中的行
    async fn remove(
        &self,
        path: &PathBuf,
        start_line: usize,
        end_line: usize,
    ) -> Option<(usize, usize, String)> {
        use tokio::fs;

        if start_line == 0 {
            return None;
        }

        // 读取文件内容
        let file_content = match fs::read_to_string(path).await {
            Ok(content) => content,
            Err(_) => {
                // 如果文件不存在，删除失败
                return None;
            }
        };

        let mut lines: Vec<String> = file_content.lines().map(String::from).collect();
        let total_lines = lines.len();

        // 验证行号范围
        if start_line > total_lines {
            return None;
        }

        // 计算实际删除的结束行号
        let actual_end_line = if end_line == 0 || end_line < start_line {
            start_line
        } else if end_line > total_lines {
            total_lines
        } else {
            end_line
        };

        // 收集被删除的内容
        let mut removed_lines = Vec::new();
        for i in start_line..=actual_end_line {
            if i <= total_lines {
                removed_lines.push(lines[i - 1].clone());
            }
        }

        // 从文件中删除行
        lines.drain((start_line - 1)..actual_end_line);

        // 写入文件
        let output = lines.join("\n");
        FileWriter::into(path, &output).await?;

        // 返回删除的行范围和内容
        let removed_content = removed_lines.join("\n");
        Some((start_line, actual_end_line, removed_content))
    }

    fn is_within(&self, path: &Path) -> bool {
        let path_str = match path.to_str() {
            Some(s) => s,
            None => return false,
        };
        if path_str.is_empty() {
            return false;
        }

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
            fs::create_dir_all(parent).await.ok()?;
        }

        fs::write(path, content).await.ok()?;

        Some(())
    }
}
