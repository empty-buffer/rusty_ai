mod error;
pub mod history;
mod models;

use std::{collections::HashMap, path::PathBuf};

use genai::chat::{ChatMessage, ChatRequest};
use genai::Client;

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
            Model::OLLAMA => "llama3.1:8b",
            Model::OPENAI => "gpt-4o-mini",
            Model::ANTROPIC => todo!(),
        }
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

    pub async fn send_to_api(self, context: &str) -> Result<String> {
        let chat_req = ChatRequest::new(vec![
            ChatMessage::system("Questions related to Rust language"),
            ChatMessage::user(context),
        ]);

        let chat_client = Client::default();

        let res = chat_client
            .exec_chat(self.model.clone().into(), chat_req, None)
            .await
            .expect("Big Problem");

        let answer = res.content_text_as_str().unwrap_or("No answer");

        Ok(answer.to_string())
    }
}
