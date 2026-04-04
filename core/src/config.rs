use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{CoreErr, CoreResult};

use std::fs;

///记忆设置
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[serde(default)]
pub struct MemConfig {
    ///丢弃的低分
    pub min_score: f32,
    ///注入的高分
    pub max_score: f32,
    ///增长率
    pub boost: f32,
    ///衰减率
    pub penalty: f32,
    ///高分注入数
    pub high_limit: usize,
    ///检索数
    pub top_k: usize,
}

impl Default for MemConfig {
    fn default() -> Self {
        Self {
            min_score: 0.05,
            max_score: 9.0,
            boost: 0.02,
            penalty: 0.01,
            high_limit: 2,
            top_k: 3,
        }
    }
}

///agent设置
#[derive(Debug, Deserialize, Serialize, Default, Clone)]
#[serde(default)]
pub struct AgentConfig {
    ///领导
    pub leader: RoleConfig,
    ///下属
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagents: Option<Vec<RoleConfig>>,
    ///嵌入
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embed: Option<RoleConfig>,
}

impl AgentConfig {
    pub fn set_leader(&mut self, character: &str, agent: &str, provider: &str) {
        let role = RoleConfig {
            character: character.to_string(),
            agent: agent.to_string(),
            provider: provider.to_string(),
        };

        self.leader = role;
    }
    pub fn add_subagent(&mut self, character: &str, agent: &str, provider: &str) {
        let role = RoleConfig {
            character: character.to_string(),
            agent: agent.to_string(),
            provider: provider.to_string(),
        };

        let mut list = self.subagents.clone().unwrap_or_default();

        list.push(role);

        self.subagents = Some(list);
    }
}

///角色设置
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct RoleConfig {
    ///名字(要和提示词文件名配合)
    pub character: String,
    ///模型名（要对应api里的设置）
    pub agent: String,
    pub provider: String,
}
impl Default for RoleConfig {
    fn default() -> Self {
        Self {
            character: "default".to_string(),
            agent: "deepseek".to_string(),
            provider: "siliconflow".to_string(),
        }
    }
}

///一般设置
#[derive(Deserialize, Debug, Serialize, Clone)]
#[serde(default)]
pub struct NormalConfig {
    ///配置根目录
    pub sc_root: PathBuf,
    ///api文件路径
    pub api_path: PathBuf,
    ///自动存储启动数量
    pub store_num: usize,
    ///记忆提示词文件路径
    pub mem_prompt: PathBuf,
    ///缓存数量
    pub cache_num: usize,
}

impl Default for NormalConfig {
    fn default() -> Self {
        let sc_root = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("synapcore");

        let api_path = sc_root.clone().join("api.json");
        let mem_prompt = sc_root.clone().join("prompts").join("memory.md");
        Self {
            sc_root,
            api_path,
            store_num: 50,
            mem_prompt,
            cache_num: 50,
        }
    }
}

///核心配置定义
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
#[serde(default)]
pub struct CoreConfig {
    pub normal: NormalConfig,
    pub agent: AgentConfig,
    pub memory: MemConfig,
}

impl CoreConfig {
    pub fn init() -> CoreResult<Self> {
        let mut core = CoreConfig::default();
        // println!("core:{:#?}",&core);

        let config = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("synapcore")
            .join("synapcore.toml");

        if !config.exists() {
            core.save()?;
        }

        match CoreConfig::load(&config) {
            Some(c) => {
                core = c;
            }
            None => return Err(CoreErr::InitError("载入配置失败".to_string())),
        }

        Ok(core)
    }

    pub fn save(&self) -> CoreResult<()> {
        let path = self.normal.sc_root.join("synapcore.toml");
        println!("path:{:#?}", &path);

        if !path.exists() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| CoreErr::InitError(format!("目录创建失败：{}", e)))?;
            } else {
                return Err(CoreErr::InitError("目录创建失败".to_string()));
            }
            fs::File::create_new(&path)
                .map_err(|e| CoreErr::InitError(format!("文件创建失败：{}", e)))?;
        }
        let content = toml::to_string_pretty(&self)
            .map_err(|e| CoreErr::InitError(format!("配置转化失败：{}", e)))?;

        fs::write(path, content).map_err(|e| CoreErr::InitError(format!("文件写入失败：{}", e)))?;
        Ok(())
    }

    fn load(path: &PathBuf) -> Option<CoreConfig> {
        let content = fs::read_to_string(path).ok()?;

        toml::from_str(&content).ok()
    }
}
