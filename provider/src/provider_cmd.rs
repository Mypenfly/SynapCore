use synapcore_core::{BotResponse, UserMessage};

#[derive(Debug)]
pub enum ProviderCommand {
    SwitchThink(bool),
    ChangeModel {
        character: String,
        agent: String,
        provider: String,
    },
    Send{
        message:UserMessage
    },
    Exit,
}

#[derive(Debug)]
pub enum ProviderResponse {
    Response(BotResponse),
    Error(String),
}
