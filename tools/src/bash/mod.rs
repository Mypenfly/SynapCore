use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::{Mutex, mpsc};

use crate::bash::error::BashErr;
use crate::define_call::tool_call::Function;
use crate::define_call::tool_define::{FunctionDefinition, Tool, ToolDefinition};
use crate::tool_response::ToolResponse;

mod error;

#[derive(Default, Debug, Serialize, Deserialize)]
struct Args {
    command: Vec<String>,
}

pub(super) struct Bash {
    exe: Option<BashExe>,
}

impl Tool for Bash {
    fn definition(&self) -> crate::define_call::tool_define::ToolDefinition {
        let name = "bash".to_string();
        let description =
            "支持除了sudo,rm命令以外的所有命令（如果要用rm请考虑function files_system）"
                .to_string();
        let parameters = serde_json::json!({
            "type":"object",
            "properties":{
                "command":{
                    "type":"array",
                    "item":{"type":"string"},
                    "description":"具体命令，例如：[\"ls\",\"-la\"]"
                }
            },
            "required":["command"]
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
        let arguments = match &function.arguments {
            Some(s) => s,
            None => return ToolResponse::Error("Function bash lacks arguments".to_string()),
        };
        let args: Result<Args, serde_json::Error> = serde_json::from_str(arguments);
        if let Err(e) = args {
            return ToolResponse::Error(format!("Function bash failed : {}", e));
        };

        let args = args.unwrap();

        self.exe.clone().unwrap().shell(args).await
        // let exe = self.exe.as_mut().unwrap();
        // exe.shell(args).await
    }
}

impl Bash {
    pub(crate) fn new() -> Self {
        Self { exe: None }
    }

    ///初始化
    pub(crate) fn init(&mut self) -> Result<(), BashErr> {
        // println!("init");

        //避免二次开启
        if self.exe.is_some() {
            return Ok(());
        }

        let mut child = Command::new("bash")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(BashErr::Init)?;

        let stdin = match child.stdin.take() {
            Some(s) => s,
            None => return Err(BashErr::Other("stdin take in none".to_string())),
        };
        let stdout = match child.stdout.take() {
            Some(s) => s,
            None => return Err(BashErr::Other("stdout take in none".to_string())),
        };
        let stderr = match child.stderr.take() {
            Some(s) => s,
            None => return Err(BashErr::Other("stdout take in none".to_string())),
        };

        let (out_tx, out_rx) = mpsc::channel(100);

        let tx_out = out_tx.clone();

        //标准输出
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx_out.send(BashOut::StdOut(line)).await;
            }
        });

        //错误输出
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = out_tx.send(BashOut::StdErr(line)).await;
            }
        });
        self.exe = Some(BashExe {
            rx: Arc::new(Mutex::new(out_rx)),
            stdin: Arc::new(Mutex::new(stdin)),
        });

        Ok(())
    }
}

///执行体
#[derive(Debug, Clone)]
struct BashExe {
    ///接受通道
    rx: Arc<Mutex<mpsc::Receiver<BashOut>>>,
    ///发送通道
    stdin: Arc<Mutex<tokio::process::ChildStdin>>,
}

impl BashExe {
    ///执行
    pub(crate) async fn shell(&mut self, args: Args) -> ToolResponse {
        // println!("shell");
        //判别命令内容
        let cmd = args.command;

        if cmd.contains(&"sudo".to_string()) || cmd.contains(&"rm".to_string()) {
            return ToolResponse::Error(format!(
                "Function bash can not execute dangerouse command : {:?} which contains sudo or rm",
                &cmd
            ));
        }

        //结束判别符
        let marker = format!("__CMD__DONE__{}_", &cmd[0]);
        let mut command = String::new();
        for c in &cmd {
            if c.contains("sudo") || c.contains("rm") {
                return ToolResponse::Error(format!(
                    "Function bash can not execute dangerouse command : {:?} which contains sudo or rm",
                    &cmd
                ));
            }

            command.push_str(&format!("{} ", c));
        }
        //写入
        {
            let mut stdin = self.stdin.lock().await;

            let _ = stdin.write_all(format!("{}\n", command).as_bytes()).await;
            let _ = stdin
                .write_all(format!("echo {}\n", &marker).as_bytes())
                .await;
            let _ = stdin.flush().await;
        }

        let mut output = String::new();
        let rx = Arc::clone(&self.rx);
        let mut rx = rx.lock().await;

        // println!("before recv");

        while let Some(out) = rx.recv().await {
            // println!("{}",&out.to_string());
            let content = out.to_string();

            if content.contains(&marker) {
                break;
            }

            output.push_str(&format!("{}\n", content));
        }

        // println!("content:{}",&output);

        ToolResponse::Bash { command, output }
    }
}

enum BashOut {
    StdOut(String),
    StdErr(String),
}

impl Display for BashOut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StdOut(v) => write!(f, "{}", v),
            Self::StdErr(e) => write!(f, "\nError:\n{}\n", e),
        }
    }
}
