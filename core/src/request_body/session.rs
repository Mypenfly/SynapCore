use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;

use crate::{
    request_body::{agent::Agent, messenge::Messenge},
};

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Session {
    pub id: String,
    pub agent: Agent,
    pub provider: String,
    pub messenge: VecDeque<Messenge>,
}

impl Session {
    pub fn new(model: String, provider: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            agent: Agent::new(model),
            provider,
            messenge: VecDeque::new(),
        }
    }
    ///添加消息
    pub fn add_messenge(&mut self, messenge: Messenge) {
        self.messenge.push_back(messenge);
    }
    ///提取消息
    pub fn get_messenges(&self) -> Vec<&Messenge> {
        self.messenge.iter().collect()
    }
    ///压缩对话，从第一项开始是避免将系统提示词给覆盖了，应该能减少tokens
    pub fn compression(&mut self, n: usize) -> Vec<Messenge> {
        self.messenge.drain(1..n).collect()
    }
    ///转化成api格式
    pub fn format_api(&self) -> Vec<serde_json::Value> {
        self.messenge.iter().map(|m| m.format_api()).collect()
    }
    ///估计token消耗
    // pub fn estimate_tokens(&self) ->usize {
    //     self.messenge.iter().map(|m|{
    //         let content_len = m.content.chars().count();
    //         content_len / 2 + 10
    //     }).sum()
    // }
    ///写入文件
    pub fn save_to_file(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }
    ///加载文件
    pub fn load_from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let json = std::fs::read_to_string(path)?;
        let session = serde_json::from_str(&json)?;
        Ok(session)
    }
}
