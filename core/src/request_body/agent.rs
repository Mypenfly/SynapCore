use serde::{Deserialize, Serialize};

use super::messenge::Messenge;

#[derive(Default,Debug,Serialize,Deserialize,PartialEq,Clone)]
pub struct Agent{
    pub model:String,
    pub leader:bool
}

impl Agent {
    pub fn new(model:String) -> Self {
        Self { model, leader: false }
    }
}


