use crate::chat::{ChatContext, Model};
use crate::editor::RequestState;
use crate::error::Result;
use once_cell::sync::Lazy;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use tokio::runtime::Runtime;

// Shared Tokio runtime
static RUNTIME: Lazy<Runtime> =
    Lazy::new(|| Runtime::new().expect("Failed to create Tokio runtime"));

pub struct AsyncCommandHandler {
    editor_state: Arc<Mutex<EditorState>>,
    chat_context: ChatContext,
}

// Define a struct to hold shared editor state that can be accessed from async contexts
pub struct EditorState {
    pub request_state: RequestState,
    pub api_response: Option<ApiResponse>,
}

pub struct ApiResponse {
    pub content: String,
    pub error: Option<String>,
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            request_state: RequestState::Idle,
            api_response: None,
        }
    }

    pub fn set_error(&mut self, error: String) {
        self.request_state = RequestState::Error(error);
    }
}

impl AsyncCommandHandler {
    pub fn new(editor_state: Arc<Mutex<EditorState>>, chat_context: ChatContext) -> Self {
        Self {
            editor_state,
            chat_context,
        }
    }

    // Simulate Ollama API request
    pub fn request_ollama(&self) {
        // Update state to processing
        if let Ok(mut state) = self.editor_state.lock() {
            state.request_state = RequestState::Proccessing;
        }

        // Clone the state reference for the async task
        let state_ref = Arc::clone(&self.editor_state);

        // Spawn the async task
        RUNTIME.spawn(async move {
            // Simulate API call
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

            // Update state when done
            if let Ok(mut state) = state_ref.lock() {
                state.request_state = RequestState::Idle;
            }
        });
    }

    pub fn send_to_api(&self, content: String, ai_model: Model) {
        // Early validation
        if content.is_empty() {
            if let Ok(mut state) = self.editor_state.lock() {
                state.set_error("Cannot send empty buffer. Please write the question".to_owned());
            }
            return;
        }

        // Set state to processing
        if let Ok(mut state) = self.editor_state.lock() {
            state.request_state = RequestState::Proccessing;
        }

        // Setup logging
        let mut log = match OpenOptions::new()
            .create(true)
            .append(true)
            .open("rusty_ai_error.log")
        {
            Ok(file) => file,
            Err(e) => {
                eprintln!("Could not open log file: {}", e);
                if let Ok(mut state) = self.editor_state.lock() {
                    state.set_error(format!("Failed to open log file: {}", e));
                }
                return;
            }
        };

        if let Err(e) = writeln!(log, "Sending to {}", ai_model) {
            eprintln!("Could not write to log: {}", e);
        }

        // Clone the needed references for the thread
        let chat_context = self.chat_context.clone();
        let content_clone = content.clone();
        let api_name_clone = ai_model.to_string();
        let state_ref = Arc::clone(&self.editor_state);

        // Spawn the worker thread
        thread::spawn(move || {
            // Execute the async operation in the runtime
            let result = RUNTIME
                .block_on(async { chat_context.send_to_api(ai_model, &content_clone).await });

            // Log and update state based on the result
            match result {
                Ok(response) => {
                    // Format the response
                    let formatted_response = format!("\n\nAssistant\n {}", response);

                    // Update the editor state with the response
                    if let Ok(mut state) = state_ref.lock() {
                        state.request_state = RequestState::Idle;
                        state.api_response = Some(ApiResponse {
                            content: formatted_response,
                            error: None,
                        });
                    }
                }
                Err(e) => {
                    // Log the error
                    if let Err(log_err) = writeln!(log, "api error: {:?}", e) {
                        eprintln!("Failed to write to log: {}", log_err);
                    }

                    // Update the editor state with the error
                    if let Ok(mut state) = state_ref.lock() {
                        state.request_state = RequestState::Error(e.to_string());
                        state.api_response = Some(ApiResponse {
                            content: String::new(),
                            error: Some(e.to_string()),
                        });
                    }
                }
            }
        });
    }

    // Future method for LSP requests
    pub fn request_lsp_completion(&self, _position: (usize, usize)) {
        // Similar implementation to request_ollama
        // Will be implemented when needed
    }
}
