use std::fmt::Display;

use reqwest::Client;

use crate::read_config::LLMConfig;

#[derive(Debug)]
pub enum EmbeddingErr {
    ApiError(String),
}

pub type EmbeddingResult<T> = Result<T, EmbeddingErr>;

impl Display for EmbeddingErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ApiError(s) => write!(f, "{}", s),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EmbeddingClient {
    base_url: String,
    api_key: String,
    model: String,
    client: Client,
}

impl EmbeddingClient {
    pub fn new(config: LLMConfig) -> Self {
        let client = Client::new();
        Self {
            base_url: config.provider.base_url,
            api_key: config.api_key,
            model: config.model_id,
            client,
        }
    }

    pub async fn embed(&self, text: &str) -> EmbeddingResult<Vec<f32>> {
        // println!("client:\n{:#?}",&self);

        // let url = format!("{}/embeddings",&self.base_url);
        // let key = self.api_key.clone();

        // println!("url:{}\nkey:{}",&url,&key);

        let response = self
            .client
            .post(format!("{}/embeddings", &self.base_url))
            .header("Authorization", format!("Bearer {}", &self.api_key))
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "model":self.model,
                "input": text,
                "encoding_format":"float"
            }))
            .send()
            .await
            .map_err(|e| EmbeddingErr::ApiError(e.to_string()))?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or("falied".to_string());

            return Err(EmbeddingErr::ApiError(error));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| EmbeddingErr::ApiError(e.to_string()))?;

        let embedding = json["data"][0]["embedding"]
            .as_array()
            .ok_or_else(|| EmbeddingErr::ApiError("Invalid response format".into()))?
            .iter()
            .map(|v| v.as_f64().unwrap_or(0.0) as f32)
            .collect();

        Ok(embedding)
    }
}
