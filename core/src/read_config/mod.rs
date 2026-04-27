use std::{collections::HashMap, fs, io::Write, path::Path};

use serde::{Deserialize, Serialize};

mod json_err;
use json_err::{JsonErr, JsonResult};

///解析json的反应体
#[derive(Debug, Default, Deserialize, Serialize, Clone)]
pub struct Provider {
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub models: Vec<Model>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub use_params:Option<bool>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub extract_params:Option<HashMap<String,String>>
}

#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct Model {
    pub name: String,
    pub model_id: String,
}

#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct Params {
    pub temperature: f64,
    pub max_tokens: u64,
    pub top_p: f64,
    pub enable_thinking: bool,
}

#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct JsonConfig {
    pub providers: Vec<Provider>,
    pub streaming: bool,
    pub params: Params,
    pub metadata: HashMap<String, String>,
}

impl Default for JsonConfig {
    fn default() -> Self {
        let model = Model {
            name: "gpt4o".to_string(),
            model_id: "gpt-4o".to_string(),
        };

        let provider = Provider {
            name: "openai".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: "YOUR API KEY".to_string(),
            models: vec![model],
            use_params:Some(true),
            extract_params:None
        };

        let streaming = true;
        let params = Params {
            temperature: 0.7,
            max_tokens: 4096,
            top_p: 0.9,
            enable_thinking: true,
        };

        let metadata = HashMap::new();

        Self {
            providers: vec![provider],
            streaming,
            params,
            metadata,
        }
    }
}

impl JsonConfig {
    pub fn from_file(path: &Path) -> JsonResult<Self> {
        if !path.exists() {
            return Err(JsonErr::GetConfig("未找到配置文件".to_string()));
        }

        let content = fs::read_to_string(path).map_err(JsonErr::Read)?;

        // println!("{:#?}",content);

        serde_json::from_str(&content).map_err(JsonErr::Convert)
    }

    pub fn rewrite_config(&self, path: &Path) -> JsonResult<()> {
        // println!("SELF:\n{:#?}\n",&self);
        if let Some(root_path) = path.parent() {
            std::fs::create_dir_all(root_path)
                .map_err(|e| JsonErr::GetConfig(format!("创建目录失败:{}", e)))?;
        }

        let new_config = serde_json::to_string_pretty(self).map_err(JsonErr::Convert)?;

        // File::create 以写模式打开文件，文件不存在则创建，存在则截断
        let mut file = std::fs::File::create(path).map_err(JsonErr::FileOpen)?;
        file.write_all(new_config.as_bytes())
            .map_err(JsonErr::FileOpen)?;
        Ok(())
    }

    ///提取config
    pub fn get_config(&self, provider: &str, model: &str) -> JsonResult<LLMConfig> {
        let mut llm_config = LLMConfig::default();
        let list: Vec<Provider> = self
            .providers
            .iter()
            .filter(|p| p.name == provider)
            .cloned()
            .collect();

        if list.is_empty() {
            return Err(JsonErr::GetConfig(format!(
                "未找到：provider({})",
                provider
            )));
        }

        for prov in list.iter() {
            llm_config.provider = prov.clone();
            llm_config.api_key = prov.api_key.clone();
            for m in &prov.models {
                if m.name == model {
                    llm_config.model_id = m.model_id.clone();
                } else {
                    continue;
                };
            }
        }

        if llm_config.model_id.is_empty() {
            return Err(JsonErr::GetConfig(format!("未找到：model({})", model)));
        }

        Ok(llm_config)
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct LLMConfig {
    pub provider: Provider,
    pub model_id: String,
    pub api_key: String,
}

// #[cfg(test)]
// mod test {

//     use std::{collections::HashMap, fs, io::Write};

//     use super::JsonConfig;

//     #[test]
//     fn test() {
//         let path = "/home/mypenfly/projects/synapcore/.config/config.json";
//         let mut config = JsonConfig::from_file(path).unwrap();
//         config
//             .metadata
//             .insert(String::from("session_id"), String::from("synapcore"));

//         let new_config = serde_json::to_string_pretty(&config).unwrap();

//         let path2 = "/home/mypenfly/projects/synapcore/.config/config_test.json";
//         let mut file = fs::File::create(path2).unwrap();

//         file.write_all(new_config.as_bytes()).unwrap();

//         println!("config: {:#?}", config);
//     }
// }
