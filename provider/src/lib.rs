mod notify;
mod timer;

use synapcore_core::{Core, CoreResult};

pub use notify::SystemNotify;
pub use synapcore_core::SendMode;
pub use timer::{Timer, TimerErr, TimerNotification, TimerStore};
mod auto_loop;

use timer::TimerLoop;
use tokio::sync::{mpsc, watch};

pub struct Provider {
    core: Core,
    shutdown_tx: watch::Sender<bool>,
    timer_rx: mpsc::Receiver<TimerNotification>,
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
        })
    }

    pub async fn send(
        &mut self,
        message: &synapcore_core::UserMessage,
    ) -> CoreResult<tokio::sync::mpsc::Receiver<synapcore_core::BotResponse>> {
        match message.mode {
            SendMode::Task => self.core.task(message).await,
            SendMode::Chat => self.core.chat(&message.character, message).await,
        }
    }

    pub async fn run(&mut self) -> CoreResult<()> {
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

        //provider循环
        loop {
            tokio::select! {
                Some(notification) = self.timer_rx.recv() => {
                    if let Err(e) = SystemNotify::send(
                        "SynapCore 定时提醒",
                        &notification.body,
                    ) {
                        eprintln!("[Provider] notify error: {e}");
                    }
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(3600)) => {
                    if *self.shutdown_tx.borrow() {
                        break;
                    }
                }
            }
        }

        let _ = timer_handle.await;
        Ok(())
    }
}
