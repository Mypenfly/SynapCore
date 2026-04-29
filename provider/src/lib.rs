mod auto_loop;
mod notify;
mod provider_cmd;
mod timer;

use synapcore_core::{BotResponse, Core, CoreResult};

pub use notify::SystemNotify;
pub use provider_cmd::{ProviderCommand, ProviderResponse};
pub use synapcore_core::SendMode;
pub use timer::{Timer, TimerErr, TimerNotification, TimerStore};

use timer::TimerLoop;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tokio::time::Duration;

use crate::auto_loop::AutoLoopManager;

pub struct Provider {
    core: Core,
    shutdown_tx: watch::Sender<bool>,
    timer_rx: mpsc::Receiver<TimerNotification>,
    auto_loop: Option<AutoLoopManager>,
}

impl Provider {
    ///启动
    pub fn new() -> CoreResult<Self> {
        let core = Core::init()?;
        let (shutdown_tx, _) = watch::channel(false);
        let (_, timer_rx) = mpsc::channel::<TimerNotification>(64);
        Ok(Self {
            core,
            shutdown_tx,
            timer_rx,
            auto_loop: None,
        })
    }

    ///发送请求
    async fn send(
        &mut self,
        message: &synapcore_core::UserMessage,
    ) -> CoreResult<tokio::sync::mpsc::Receiver<synapcore_core::BotResponse>> {
        match message.mode {
            SendMode::Task => self.core.task(message).await,
            SendMode::Chat => self.core.chat(&message.character, message).await,
        }
    }

    ///timer 启动
    async fn timer_run(&mut self) -> CoreResult<JoinHandle<()>> {
        let (timer_notify_tx, timer_notify_rx) = mpsc::channel::<TimerNotification>(64);
        let shutdown_rx = self.shutdown_tx.subscribe();

        let timer_loop = match TimerLoop::new(shutdown_rx, timer_notify_tx) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("[Provider] TimerLoop init failed: {e}, running without timer");
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
                }
            }
        };

        self.timer_rx = timer_notify_rx;

        let mut timer_loop = timer_loop;

        let timer_handle = tokio::spawn(async move {
            if let Err(e) = timer_loop.run().await {
                eprintln!("[Provider] TimerLoop exited with error: {e}");
            }
        });
        Ok(timer_handle)
    }
    ///auto_loop启动
    fn auto_loop_run(&mut self) -> CoreResult<()> {
        let gap = self.core.config.normal.auto_loop_gap;
        let auto_manager = match AutoLoopManager::new(gap) {
            Ok(am) => Some(am),
            Err(e) => {
                eprint!("[Provider] Auto loop init failed : {}", e);
                None
            }
        };
        self.auto_loop = auto_manager;
        Ok(())
    }

    ///退出
    fn exit(&mut self) -> CoreResult<()> {
        if let Some(al) = &self.auto_loop {
            let al_result = al.exit();
            if let Err(e) = al_result {
                eprintln!("[Provider] auto loop error : {}", e);
            }
        }
        self.core.exit()
    }

    /// 启动Provider主循环
    pub async fn run(
        mut self,
        mut cmd_rx: tokio::sync::mpsc::Receiver<ProviderCommand>,
        resp_tx: tokio::sync::mpsc::Sender<ProviderResponse>,
    ) -> CoreResult<()> {
        // 创建shutdown接收器用于主循环
        let mut shutdown_rx_for_main = self.shutdown_tx.subscribe();
        //启动auto_loop
        self.auto_loop_run()?;
        // let auto_loop_manager = AutoLoopManager::new(self.core.config.normal.auto_loop_gap);

        // 启动Timer
        let timer_handle = self.timer_run().await?;
        // AutoLoop计时器
        let mut auto_loop_interval = tokio::time::interval(Duration::from_secs(60)); // 每分钟检查一次
        let mut auto_loop_elapsed_minutes = 0;

        // 主循环
        let (_, mut bot_response) = mpsc::channel(1024);
        // eprintln!("[Provider] main loop start");
        loop {
            tokio::select! {
                // 处理命令
                Some(cmd) = cmd_rx.recv() => {
                    //指令处理
                    match self.handle_command(cmd).await {
                        Ok(LoopContinue::Continue(false)) => break,
                        Ok(LoopContinue::Continue(true)) => continue,
                        Ok(LoopContinue::Response(rev)) =>bot_response = rev ,
                        Err(e) => {
                            let _ = resp_tx.send(ProviderResponse::Error(format!("命令处理错误: {}", e))).await;
                        }
                    }
                }

                // 处理Timer通知
                Some(notification) = self.timer_rx.recv() => {
                    if let Err(e) = SystemNotify::send(
                        "SynapCore 定时提醒",
                        &notification.body,
                    ) {
                        eprintln!("[Provider] notify error: {e}");
                    }
                }

                //检查接受情况
                Some(content) = bot_response.recv() => {
                    // println!("{}",&content);
                    let response = ProviderResponse::Response(content);
                    let _ =resp_tx.send(response).await ;
                }


                // AutoLoop计时
                _ = auto_loop_interval.tick() => {
                    auto_loop_elapsed_minutes += 1;
                    // println!("计时 + 1");

                    // println!("auto loop : {:#?}",&self.auto_loop);
                    if let Some(al) = &mut self.auto_loop && al.tick(auto_loop_elapsed_minutes,self.core.config.normal.auto_loop_gap).await{
                        let result = Core::init();
                        if let Ok(core) = result {
                        let al_result = al.run_once(core).await;
                            if let Err(e) = al_result{
                                eprintln!("[Provider] AutoLoop执行失败: {}", e);
                        }}else {
                            eprintln!("[provider] AutoLoop failed in core init : {:#?}",result);
                        }
                    }
                }

                // 检查shutdown信号
                _ = shutdown_rx_for_main.changed() => {
                    if *shutdown_rx_for_main.borrow() {
                        break;
                    }
                }
            }
        }

        // 清理资源
        let _ = timer_handle.await;
        Ok(())
    }

    async fn handle_command(&mut self, cmd: ProviderCommand) -> CoreResult<LoopContinue> {
        match cmd {
            ProviderCommand::SwitchThink(enable) => {
                self.switch_think(enable)?;
                Ok(LoopContinue::Continue(true))
            }
            ProviderCommand::ChangeModel {
                character,
                agent,
                provider,
            } => {
                self.change_model(&character, &agent, &provider)?;
                Ok(LoopContinue::Continue(true))
            }
            ProviderCommand::Send { message } => {
                let result = self.send(&message).await;
                let rev = match result {
                    Ok(re) => re,
                    Err(e) => return Err(e),
                };

                Ok(LoopContinue::Response(rev))
            }
            ProviderCommand::Exit => {
                // 执行AutoLoop和Core的exit方法
                self.exit()?;

                Ok(LoopContinue::Continue(false))
            }
        }
    }

    fn switch_think(&mut self, enable_think: bool) -> CoreResult<()> {
        self.core.api_json.params.enable_thinking = enable_think;
        Ok(())
    }

    fn change_model(&mut self, character: &str, agent: &str, provider: &str) -> CoreResult<()> {
        self.core
            .config
            .agent
            .set_leader(character, agent, provider);
        Ok(())
    }
}

enum LoopContinue {
    Continue(bool),
    Response(tokio::sync::mpsc::Receiver<BotResponse>),
}

mod test {
    use std::io::Write;

    use synapcore_core::UserMessage;

    use crate::Provider;

    #[tokio::test]
    async fn test() {
        let mut query = UserMessage::chat("Yore");
        query.text = "你好".to_string();
        query.enable_tools = false;
        query.is_save = false;

        let provider = Provider::new().unwrap();

        let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(1024);
        let (resp_tx, mut resp_rx) = tokio::sync::mpsc::channel(1024);

        tokio::spawn(async move {
            // println!("Core {:#?}",&provider.core);
            let _ = provider.run(cmd_rx, resp_tx).await;
        });

        let _ = cmd_tx
            .send(crate::ProviderCommand::Send { message: query })
            .await;

        while let Some(content) = resp_rx.recv().await {
            match content {
                crate::ProviderResponse::Response(res) => {
                    print!("{}", res);
                    std::io::stdout().flush().unwrap();
                }
                crate::ProviderResponse::Error(e) => {
                    eprintln!("{}", e);
                }
            }
            // print!("{}",content);
            // std::io::stdout().flush().unwrap();
        }
    }
}
