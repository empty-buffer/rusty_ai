mod error;
mod models;
pub mod history;

use std::{collections::HashMap, path::PathBuf};

use genai::chat::{ChatMessage, ChatRequest};
use genai::Client;

use crate::files::{change_dir, list_current_dir, load_file};
use history::History; 
use crate::Result;

// const MODEL_OPENAI: &str = "gpt-4o-mini";
// const MODEL_OLLAMA: &str = "llama3.1:8b"; 

#[derive(Debug, Clone)]
pub enum Model {
    OLLAMA,
    OPENAI,
    ANTROPIC,
}


impl From <Model> for &str {
    fn from(value: Model) -> Self {
        match value {
            Model::OLLAMA => "llama3.1:8b",
            Model::OPENAI => "gpt-4o-mini",
            Model::ANTROPIC => todo!(),
        }
    }
}



pub struct ChatContext {
    pub loaded_files: HashMap<String, String>,
    pub conversation_history: Vec<(String, String)>,
    pub current_dir: PathBuf,
    pub history_file: History,
    pub model: Model,
}

impl ChatContext {
    pub fn new() -> Result<Self> {
        Ok(ChatContext {
            loaded_files: HashMap::new(),
            conversation_history: Vec::new(),
            current_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            history_file: History::new()?,
            model: Model::OLLAMA,

        })
    }

    pub fn files(&self) -> Result<(Vec<String>, Vec<String>)> {
        list_current_dir(&self.current_dir)
    }

    pub fn set_new_dir(&mut self, path: &str) -> Result<()> {
        self.current_dir = change_dir(&self.current_dir, path)?;
        Ok(())
    }


    pub fn set_new_model(&mut self, model: Model) -> Result<()>{
        self.model = model;
        Ok(())
    }
    
    pub fn load_context_from_file(&mut self, filename: &str) -> Result<()> {
        let full_path = self
            .current_dir
            .join(filename)
            .to_string_lossy()
            .to_string();

        let content = load_file(&full_path)?;

        self.loaded_files.insert(filename.to_string(), content);

        Ok(())
    }

    pub fn add_conv_context(&self, question: &str) -> String {
        let mut context = String::new();

        for (filename, content) in &self.loaded_files {
            context.push_str(&format!("Path '{}' \n Content: {}\n\n", filename, content));
        }

        // Add conversation history
        for (prev_question, prev_answer) in &self.conversation_history {
            context.push_str(&format!(
                "User: {}\nSystem: {}\n\n",
                prev_question, prev_answer
            ));
        }

        // Add current
        context.push_str(&format!("Current question: {}", question));

        context
    }

    pub async fn send_to_api(&mut self, context: &str) -> Result<String> {
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

    // Step 5: Save response
    pub fn save_response(&mut self, question: String, answer: String) {
        self.conversation_history.push((question, answer));
    }

    // Get current directory as string
    pub fn get_current_dir_display(&self) -> String {
        self.current_dir.display().to_string()
    }
}
