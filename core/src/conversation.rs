use serde::{Deserialize, Serialize};
use std::fs;
use std::{io::Write, path::PathBuf};

/// 储存简易聊天记录
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct Conversation {
    pub user: TempData,
    pub agent: String,
}

impl Conversation {
    pub(crate) fn load(path: &PathBuf) -> Option<Vec<Conversation>> {
        let content = fs::read_to_string(path).ok()?;
        let messages = serde_json::from_str(&content).ok()?;
        Some(messages)
    }

    pub(crate) fn create_file(path: &PathBuf) -> Option<()> {
        if !path.exists() {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).ok()?;
            } else {
                return None;
            }

            let con = Conversation::default();
            let content_json = serde_json::json!(vec![con]);
            let content = serde_json::to_string_pretty(&content_json).ok()?;

            let mut file = std::fs::File::create(path).ok()?;

            file.write_all(content.as_bytes()).ok()?;
        }

        Some(())
    }

    pub(crate) fn append(data: &TempData, content: &str) -> Conversation {
        Conversation {
            user: data.to_owned(),
            agent: content.to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct TempData {
    pub text: String,
    pub files: Vec<PathBuf>,
}
