pub mod history;

mod error;
mod models;

use std::env;
// use std::ascii::AsciiExt;
use std::{collections::HashMap, path::PathBuf};

use genai::chat::{ChatMessage, ChatRequest};
use genai::Client;
use rusty_ollama::Ollama;

use crate::files::{change_dir, list_current_dir, load_file};
use crate::Result;
use history::History;

#[derive(Debug, Clone)]
pub enum Model {
    OLLAMA,
    OPENAI,
    ANTROPIC,
}

impl From<Model> for &str {
    fn from(value: Model) -> Self {
        match value {
            Model::OLLAMA => "qwen3:32b-q4_K_M",
            Model::OPENAI => "gpt-4.1-mini",
            Model::ANTROPIC => todo!(),
        }
    }
}

impl From<Model> for String {
    fn from(value: Model) -> Self {
        match value {
            Model::OLLAMA => "qwen3:32b-q4_K_M".to_owned(),
            Model::OPENAI => "gpt-4o-mini".to_owned(),
            Model::ANTROPIC => todo!(),
        }
    }
}

impl core::fmt::Display for Model {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::result::Result<(), core::fmt::Error> {
        write!(fmt, "{self:?}")
    }
}

#[derive(Debug, Clone)]
pub struct ChatContext {
    pub model: Model,
}

impl ChatContext {
    pub fn new() -> Result<Self> {
        Ok(ChatContext {
            model: Model::OPENAI,
        })
    }

    pub async fn send_to_api(self, model: Model, content: &str) -> Result<String> {
        match model {
            Model::OLLAMA => return self.request_ollama(model, content).await,
            Model::OPENAI => return self.request_gen_ai(model, content).await,
            Model::ANTROPIC => return self.request_gen_ai(model, content).await,
        }
    }

    async fn request_gen_ai(self, model: Model, content: &str) -> Result<String> {
        let chat_req = ChatRequest::new(vec![
            ChatMessage::system("Questions related eather to Rust or Go language"),
            ChatMessage::user(content),
        ]);

        let chat_client = Client::default();

        let res = chat_client
            .exec_chat(model.into(), chat_req, None)
            .await
            .expect("Big Problem");

        let answer = res.content_text_as_str().unwrap_or("No answer");

        Ok(answer.to_string())
    }

    async fn request_ollama(self, model: Model, content: &str) -> Result<String> {
        let endpoint = match env::var("OLLAMA_ENDPOINT") {
            Ok(val) => val,
            Err(e) => return Err(crate::error::Error::Custom(e.to_string())),
        };

        let mut client = Ollama::new(endpoint, model)?;

        // client.stream_generate(prompt)

        // client.context
        let response = client.generate(content).await?;
        Ok(response.response)
    }
}
