use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::Local;
use regex::Regex;
use serde::{Deserialize, Serialize};
use synapcore_core::{BotResponse, Core, CoreErr, SendMode, UserMessage};
use tokio::sync::{mpsc, watch};

#[derive(Debug, thiserror::Error)]
pub enum TimerErr {
    #[error("IO错误: {0}")]
    Io(#[from] io::Error),
    #[error("序列化错误: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("时间格式错误，需要 YYYY-MM-DD-HH:mm: {0}")]
    TimeFormat(String),
    #[error("路径不存在: {0}")]
    PathNotFound(String),
    #[error("Core初始化失败: {0}")]
    CoreInit(#[from] CoreErr),
    #[error("Core对话失败: {0}")]
    CoreChat(String),
    #[error("正则错误: {0}")]
    Regex(#[from] regex::Error),
}

type TimerResult<T> = Result<T, TimerErr>;

const TIMER_TAG: &str = "timer";
const TIMER_BODY_MAX_LEN: usize = 50;
const POLL_INTERVAL_SECS: u64 = 30;

#[derive(Debug, Clone)]
pub struct TimerNotification {
    pub character: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timer {
    pub id: String,
    pub time: String,
    pub character: String,
    pub prompt: String,
    pub done: bool,
}

impl Timer {
    pub fn new(time: String, character: String, prompt: String) -> TimerResult<Self> {
        Self::validate_time(&time)?;
        let id = uuid::Uuid::new_v4().to_string();
        Ok(Self {
            id,
            time,
            character,
            prompt,
            done: false,
        })
    }

    pub fn validate_time(time: &str) -> TimerResult<()> {
        if chrono::NaiveDateTime::parse_from_str(time, "%Y-%m-%d-%H:%M").is_ok() {
            return Ok(());
        }
        Err(TimerErr::TimeFormat(time.to_string()))
    }

    fn is_due(&self) -> bool {
        let target = match chrono::NaiveDateTime::parse_from_str(&self.time, "%Y-%m-%d-%H:%M") {
            Ok(t) => t,
            Err(_) => return false,
        };
        let now = Local::now().naive_local();
        now >= target
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TimerStore {
    path: PathBuf,
    timers: Vec<Timer>,
}

impl TimerStore {
    pub fn load(path: &Path) -> TimerResult<Self> {
        let path = path.to_path_buf();
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let timers: Vec<Timer> = serde_json::from_str(&content)?;
            Ok(Self { path, timers })
        } else {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let store = Self {
                path,
                timers: Vec::new(),
            };
            store.save()?;
            Ok(store)
        }
    }

    pub fn reload(&mut self) -> TimerResult<()> {
        if self.path.exists() {
            let content = fs::read_to_string(&self.path)?;
            self.timers = serde_json::from_str(&content)?;
        }
        Ok(())
    }

    pub fn add(&mut self, timer: Timer) -> TimerResult<()> {
        self.timers.push(timer);
        self.save()
    }

    pub fn mark_done(&mut self, id: &str) -> TimerResult<()> {
        if let Some(timer) = self.timers.iter_mut().find(|t| t.id == id) {
            timer.done = true;
            self.save()?;
        }
        Ok(())
    }

    pub fn pending(&self) -> Vec<&Timer> {
        self.timers.iter().filter(|t| !t.done).collect()
    }

    pub fn remove(&mut self, id: &str) -> TimerResult<()> {
        let before = self.timers.len();
        self.timers.retain(|t| t.id != id);
        if self.timers.len() < before {
            self.save()?;
        }
        Ok(())
    }

    fn save(&self) -> TimerResult<()> {
        let content = serde_json::to_string_pretty(&self.timers)?;
        fs::write(&self.path, content)?;
        Ok(())
    }
}

pub fn default_timer_path() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("synapcore_cache")
        .join("timer.json")
}

fn extract_tag_content(content: &str, tag: &str) -> Vec<String> {
    let pattern = format!(r"(?s)<{tag}>(.*?)</{tag}>");
    let re = match Regex::new(&pattern) {
        Ok(re) => re,
        Err(_) => return Vec::new(),
    };
    re.captures_iter(content)
        .filter_map(|caps| caps.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let taken: String = s.chars().take(max_len).collect();
        format!("{taken}...")
    }
}

pub struct TimerLoop {
    store: TimerStore,
    core: Core,
    shutdown_rx: watch::Receiver<bool>,
    notify_tx: mpsc::Sender<TimerNotification>,
}

impl TimerLoop {
    pub fn new(
        shutdown_rx: watch::Receiver<bool>,
        notify_tx: mpsc::Sender<TimerNotification>,
    ) -> TimerResult<Self> {
        let path = default_timer_path();
        let store = TimerStore::load(&path)?;
        let core = Core::init()?;
        Ok(Self {
            store,
            core,
            shutdown_rx,
            notify_tx,
        })
    }

    pub async fn run(&mut self) -> TimerResult<()> {
        let mut interval = tokio::time::interval(Duration::from_secs(POLL_INTERVAL_SECS));

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = self.check_and_fire().await {
                        eprintln!("[TimerLoop] check error: {e}");
                    }
                }
                _ = self.shutdown_rx.changed() => {
                    break;
                }
            }
        }

        Ok(())
    }

    async fn check_and_fire(&mut self) -> TimerResult<()> {
        self.store.reload()?;

        let due: Vec<Timer> = self
            .store
            .pending()
            .into_iter()
            .filter(|t| t.is_due())
            .cloned()
            .collect();

        for timer in due {
            match self.fire(&timer).await {
                Ok(body) => {
                    self.store.mark_done(&timer.id)?;
                    let notification = TimerNotification {
                        character: timer.character.clone(),
                        body,
                    };
                    if let Err(e) = self.notify_tx.send(notification).await {
                        eprintln!("[TimerLoop] notify send error: {e}");
                    }
                }
                Err(e) => {
                    eprintln!("[TimerLoop] fire failed: timer={}, error={}", timer.id, e);
                }
            }
        }

        Ok(())
    }

    async fn fire(&mut self, timer: &Timer) -> TimerResult<String> {
        let decorated_prompt = format!(
            "[系统指令] 这是一个定时提醒任务。\
             请用 <{TIMER_TAG}>内容</{TIMER_TAG}> 格式输出你的回应，\
             内容不超过{TIMER_BODY_MAX_LEN}个字，直接摘要告知用户。\n\n\
             用户原定提醒内容：{}",
            timer.prompt
        );

        let message = UserMessage {
            text: decorated_prompt,
            files: Vec::new(),
            enable_tools: false,
            is_save: false,
            mode: SendMode::Chat,
            character: timer.character.clone(),
        };

        let mut rx = self
            .core
            .chat(&timer.character, &message)
            .await
            .map_err(TimerErr::CoreInit)?;

        let mut full = String::new();
        while let Some(resp) = rx.recv().await {
            match resp {
                BotResponse::Content { chunk } => full.push_str(&chunk),
                BotResponse::Error { error, .. } => {
                    return Err(TimerErr::CoreChat(error));
                }
                _ => {}
            }
        }

        let extracted = extract_tag_content(&full, TIMER_TAG);
        let body = extracted
            .first()
            .map(|s| truncate_str(s, TIMER_BODY_MAX_LEN))
            .unwrap_or_else(|| truncate_str(&full, TIMER_BODY_MAX_LEN));

        Ok(body)
    }
}
