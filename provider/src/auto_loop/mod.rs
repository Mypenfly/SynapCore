use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::Local;
use regex::Regex;
use serde::{Deserialize, Serialize};
use synapcore_core::{BotResponse, Core, CoreErr, SendMode, UserMessage};

const REFLECTION_TAG: &str = "reflection";
const CACHE_FILE: &str = "cache.json";

#[derive(Debug, thiserror::Error)]
pub enum AutoLoopErr {
    #[error("IO错误: {0}")]
    Io(#[from] std::io::Error),
    #[error("序列化错误: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("Core错误: {0}")]
    Core(#[from] CoreErr),
    #[error("路径错误: {0}")]
    Path(String),
    #[error("正则错误: {0}")]
    Regex(#[from] regex::Error),
    // #[error("时间转换错误: {0}")]
    // Time(String),
}

type AutoLoopResult<T> = Result<T, AutoLoopErr>;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AutoLoopCache {
    pub time_count: usize,
    pub last_run: Option<u64>,
}

pub struct AutoLoop {
    core: Core,
    time_count: usize,
    gap: usize,
}

impl AutoLoop {
    pub fn new(core: Core) -> AutoLoopResult<Self> {
        let gap = core.config.normal.auto_loop_gap;
        let cache = Self::load_cache()?;

        Ok(Self {
            core,
            time_count: cache.time_count,
            gap,
        })
    }

    ///从配置文件中载入累计时长
    fn load_cache() -> AutoLoopResult<AutoLoopCache> {
        let cache_path = AutoLoop::cache_path()?;
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

        let cache_path = AutoLoop::cache_path()?;
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

    fn reflection_path(character: &str) -> AutoLoopResult<PathBuf> {
        dirs::config_dir()
            .ok_or_else(|| AutoLoopErr::Path("无法获取配置目录".to_string()))
            .map(|p| {
                p.join("synapcore")
                    .join("data")
                    .join(format!("{}_reflection.md", character))
            })
    }

    fn extract_tag_content(content: &str, tag: &str) -> Vec<String> {
        let pattern = format!(r"(?s)<{tag}>(.*?)</{tag}>");
        match Regex::new(&pattern) {
            Ok(re) => re
                .captures_iter(content)
                .filter_map(|caps| caps.get(1).map(|m| m.as_str().to_string()))
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    fn read_reflection(character: &str) -> AutoLoopResult<String> {
        let path = Self::reflection_path(character)?;
        if path.exists() {
            fs::read_to_string(&path).map_err(|e| e.into())
        } else {
            Ok(String::new())
        }
    }

    fn write_reflection(character: &str, content: &str) -> AutoLoopResult<()> {
        let path = Self::reflection_path(character)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, content)?;
        Ok(())
    }

    ///提示词格式
    fn format_reflection_prompt(existing_reflection: &str) -> String {
        format!(
            "现在是AutoReflect模式。请你进行自我反思，总结关于用户画像、经验总结等内容。\n\n\
            请严格按照以下格式输出：\n\
            <reflection>\n\
            ## 用户画像 (User Profile)\n\
            - **基本信息**: [性别, 年龄, 职业等]\n\
            - **兴趣领域**: [技术, 学习, 工作, 生活等]\n\
            - **沟通风格**: [直接, 委婉, 技术型, 实用型等]\n\
            - **知识水平**: [初级, 中级, 高级, 专家等]\n\
            \n\
            ## 对话模式观察 (Conversation Patterns)\n\
            - **常见问题类型**: [技术问题, 学习求助, 工作咨询, 生活建议等]\n\
            - **回应偏好**: [详细解释, 简短回答, 代码示例, 理论说明等]\n\
            \n\
            ## 经验总结 (Experience Summary)\n\
            1. **有效策略**: [哪些方法对该用户特别有效]\n\
            2. **无效策略**: [哪些方法效果不佳或应避免]\n\
            3. **成功案例**: [特别成功的交互案例]\n\
            4. **改进建议**: [未来交互中可以改进的地方]\n\
            \n\
            ## 知识积累 (Knowledge Accumulation)\n\
            - **已掌握技能**: [用户已经学会的技能或知识]\n\
            - **正在学习**: [用户当前正在学习的内容]\n\
            - **知识缺口**: [用户可能需要的但尚未掌握的知识]\n\
            \n\
            ## 关系质量评估 (Relationship Quality)\n\
            - **信任程度**: [低, 中, 高]\n\
            - **合作顺畅度**: [顺畅, 一般, 需要改进]\n\
            - **沟通效率**: [高效, 正常, 有待提高]\n\
            \n\
            ## 注意事项 (Notes)\n\
            1. 保持客观, 基于实际交互数据\n\
            2. 避免主观臆断\n\
            3. 定期更新, 反映最新状态\n\
            4. 格式保持简洁明了\n\
            \n\
            ## 时间戳\n\
            - **上次更新**: [{}]\n\
            </reflection>\n\
            \n\
            现有反思内容（供参考，请更新和完善）：\n\
            {}\n\
            \n\
            请生成完整的新反思内容，覆盖现有内容并加入最新观察。",
            Local::now().format("%Y-%m-d %H:%M:%S"),
            existing_reflection
        )
    }

    ///学习
    async fn auto_study(&mut self) -> AutoLoopResult<()> {
        let character = self.core.config.agent.leader.character.clone();
        let prompt = "[System cammand]现在是AutoStudy模式，
        请你详细使用各式工具进行学习，内容包括但不限于最近和用户进行的交流，
        最近在做的项目，学习内容要使用skills_book工具规范记录，
        学习过程建议使用files_extract(学习现有项目)，web_search(查找有关资料)。
        特别注意此次任务对话记录和工具调用记录不会保存，你写在skills_book,和note_book中的内容就是你以后参照的标准";

        let message = UserMessage {
            text: prompt.to_string(),
            files: Vec::new(),
            enable_tools: true,
            is_save: false,
            mode: SendMode::Chat,
            character: character.clone(),
        };

        let mut rx = self.core.chat(&character, &message).await?;

        while let Some(resp) = rx.recv().await {
            if let BotResponse::Error { character, error } = resp {
                return Err(AutoLoopErr::Core(CoreErr::AssistantError {
                    model: character,
                    error,
                }));
            }
        }

        Ok(())
    }

    ///反思
    async fn auto_reflect(&mut self) -> AutoLoopResult<()> {
        let character = self.core.config.agent.leader.character.clone();
        let existing_reflection = Self::read_reflection(&character)?;
        let prompt = Self::format_reflection_prompt(&existing_reflection);

        let message = UserMessage {
            text: prompt,
            files: Vec::new(),
            enable_tools: false,
            is_save: false,
            mode: SendMode::Chat,
            character: character.clone(),
        };

        let mut rx = self.core.chat(&character, &message).await?;
        let mut full_response = String::new();

        while let Some(resp) = rx.recv().await {
            match resp {
                synapcore_core::BotResponse::Content { chunk } => {
                    full_response.push_str(&chunk);
                }
                synapcore_core::BotResponse::Error { error, .. } => {
                    return Err(AutoLoopErr::Core(CoreErr::AssistantError {
                        model: character.clone(),
                        error,
                    }));
                }
                _ => {}
            }
        }

        let reflections = Self::extract_tag_content(&full_response, REFLECTION_TAG);
        if let Some(reflection_content) = reflections.first() {
            Self::write_reflection(&character, reflection_content)?;
        }

        Ok(())
    }

    ///自动清理
    async fn auto_clear(&mut self) -> AutoLoopResult<()> {
        let character = self.core.config.agent.leader.character.clone();
        let prompt = "[System Command]现在是AutoClear模式，请对note_book和skills_book的内容进行清理，建议对已经失去效力或者长期不用的note和skill进行清理（建议清理启动数量 >= 20）。请开始清理工作。";

        let message = UserMessage {
            text: prompt.to_string(),
            files: Vec::new(),
            enable_tools: true,
            is_save: false,
            mode: SendMode::Chat,
            character: character.clone(),
        };

        let mut rx = self.core.chat(&character, &message).await?;

        while let Some(resp) = rx.recv().await {
            if let BotResponse::Error { character, error } = resp {
                return Err(AutoLoopErr::Core(CoreErr::AssistantError {
                    model: character,
                    error,
                }));
            }
        }

        Ok(())
    }

    ///执行一次loop
    pub async fn run_once(&mut self) -> AutoLoopResult<()> {
        println!("[AutoLoop] 开始自动学习...");
        if let Err(e) = self.auto_study().await {
            eprintln!("[AutoLoop] AutoStudy失败: {}", e);
        }

        println!("[AutoLoop] 开始自我反思...");
        if let Err(e) = self.auto_reflect().await {
            eprintln!("[AutoLoop] AutoReflect失败: {}", e);
        }

        println!("[AutoLoop] 开始清理工作...");
        if let Err(e) = self.auto_clear().await {
            eprintln!("[AutoLoop] AutoClear失败: {}", e);
        }

        self.time_count = 0;
        self.save_cache()?;

        println!("[AutoLoop] 完成一轮自动循环");
        Ok(())
    }

    ///判断gap时间是否达成
    pub async fn tick(&mut self, elapsed_minutes: usize) -> bool {
        self.time_count += elapsed_minutes;

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
