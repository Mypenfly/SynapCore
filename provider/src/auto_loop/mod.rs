use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};


use serde::{Deserialize, Serialize};
use synapcore_core::Core;

mod auto;
use auto::{AutoLoop, AutoLoopErr, AutoLoopResult};

const CACHE_FILE: &str = "cache.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AutoLoopCache {
    pub time_count: usize,
    pub last_run: Option<u64>,
}

pub(crate) struct AutoLoopManager {
    time_count: usize,
    gap: usize,
    ///设置一个锁避免出现重复loop
    loop_locked:Arc<AtomicBool>,
}

impl AutoLoopManager {
    pub fn new(gap: usize) -> AutoLoopResult<Self> {
        let cache = Self::load_cache()?;
        Ok(Self {
            time_count: cache.time_count,
            gap,
            loop_locked:Arc::new(AtomicBool::new(false))
        })
    }
    ///启动
    pub async fn run_once(&mut self, core: Core) -> AutoLoopResult<()> {
        let lock = self.loop_locked.load(Ordering::SeqCst);
        if lock {
            println!("[AutoLoop] 已有loop在循环，建议提高 auto_loop_gap 间隔时间，建议为300");
            return Ok(());
        }
        println!("[AutoLoop] 开始自动学习...");
        self.loop_locked.store(true, Ordering::SeqCst);
        
        let lock_for_loop = Arc::clone(&self.loop_locked);
        tokio::spawn(async move {
            let mut auto = match AutoLoop::new(core) {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("[Provider] auto_loop run failed: {}", e);
                    return;
                }
            };
            if let Err(e) = auto.auto_study().await {
                eprintln!("[AutoLoop] AutoStudy失败: {}", e);
            }

            println!("[AutoLoop] 开始自我反思...");
            if let Err(e) = auto.auto_reflect().await {
                eprintln!("[AutoLoop] AutoReflect失败: {}", e);
            }

            println!("[AutoLoop] 开始清理工作...");
            if let Err(e) = auto.auto_clear().await {
                eprintln!("[AutoLoop] AutoClear失败: {}", e);
            }

            println!("[AutoLoop] 完成一次循环");
            lock_for_loop.store(false, Ordering::SeqCst);
        });

        self.save_cache()?;

        println!("[AutoLoop] 一轮已经后台启动");
        Ok(())
    }
    ///从配置文件中载入累计时长
    fn load_cache() -> AutoLoopResult<AutoLoopCache> {
        let cache_path = AutoLoopManager::cache_path()?;
        if cache_path.exists() {
            let content = fs::read_to_string(&cache_path)?;
            let cache: AutoLoopCache = serde_json::from_str(&content)?;
            Ok(cache)
        } else {
            Ok(AutoLoopCache::default())
        }
    }

    fn save_cache(&self) -> AutoLoopResult<()> {
        let cache = AutoLoopCache {
            time_count: self.time_count,
            last_run: Some(current_timestamp()),
        };

        let cache_path = AutoLoopManager::cache_path()?;
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(&cache)?;
        fs::write(cache_path, content)?;
        Ok(())
    }

    fn cache_path() -> AutoLoopResult<PathBuf> {
        dirs::cache_dir()
            .ok_or_else(|| AutoLoopErr::Path("无法获取缓存目录".to_string()))
            .map(|p| p.join("synapcore_cache").join(CACHE_FILE))
    }
    ///判断gap时间是否达成
    pub async fn tick(&mut self, elapsed_minutes: usize, gap: usize) -> bool {
        self.time_count += elapsed_minutes;
        self.gap = gap;

        if self.time_count.is_multiple_of(self.gap) {
            println!("[AutoLoop] 达到间隔时间，准备执行");
            return true;
        }

        false
    }

    pub fn exit(&self) -> AutoLoopResult<()> {
        self.save_cache()?;
        Ok(())
    }
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("时间错误")
        .as_secs()
}
