use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::request_body::{
    agent::Agent,
    messenge::{Messenge, Role},
};

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Session {
    pub id: String,
    pub agent: Agent,
    pub provider: String,
    pub messenge: Vec<Messenge>,
}

impl Session {
    pub fn new(model: String, provider: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            agent: Agent::new(model),
            provider,
            messenge: Vec::new(),
        }
    }
    ///添加消息
    pub fn add_messenge(&mut self, messenge: Messenge) {
        self.messenge.push(messenge);
    }
    ///指定位置添加
    pub fn add_into(&mut self, messenge: Messenge, position: usize) {
        let len = self.messenge.len();

        if len < position + 1 {
            self.messenge.push(messenge);
        } else {
            self.messenge[position] = messenge;
        }
    }
    ///压缩对话，从第一项开始是避免将系统提示词给覆盖了，应该能减少tokens
    pub fn compression(&mut self, from: usize, to: usize) -> Vec<Messenge> {
        //先清洗掉tool_call
        self.clear_tools();
        if self.messenge.len() <= to - 10 {
            return Vec::new();
        }

        let last = self.check_last(to).unwrap_or(from);

        let mut list = Vec::new();
        //如果last大于20,直接清除
        let sub_list = if last >= 20 {
            self.messenge.drain(from..self.messenge.len())
        } else {
            self.messenge.drain(from..last)
        };
        list.extend(sub_list);
        list
    }
    ///保证压缩算法，确保压缩之后的system后第一个是user
    fn check_last(&self, to: usize) -> Option<usize> {
        if self.messenge.len() < to {
            return None;
        }
        if self.messenge[to].role == Role::User {
            return Some(to);
        }
        for num in to..0 {
            let role = &self.messenge[num].role;
            if role == &Role::User {
                return Some(num - 1);
            };
        }

        None
    }
    ///清洗tool_call
    fn clear_tools(&mut self) {
        self.messenge.pop_if(|m| m.role == Role::Tool);
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
