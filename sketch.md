



I working on cli app in rust and want to be able load context from file before send it to api. 
step 1 be able to show all file in current directory
step 2 load them in app
steo 3 enrich with user question
step 4 send to api
step 5 save response in the app for possibe next quesion

I'll help you create a Rust CLI app with those steps. Here's an example implementation:

```rust
use std::fs;
use std::io::{self, Read};
use std::path::Path;
use std::collections::HashMap;

struct ChatContext {
    loaded_files: HashMap<String, String>,
    conversation_history: Vec<(String, String)>, // (question, answer) pairs
}

impl ChatContext {
    fn new() -> Self {
        ChatContext {
            loaded_files: HashMap::new(),
            conversation_history: Vec::new(),
        }
    }

    // Step 1: List files in current directory
    fn list_files(&self) -> io::Result<Vec<String>> {
        let mut files = Vec::new();
        for entry in fs::read_dir(".")? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Some(filename) = path.file_name() {
                    if let Some(filename_str) = filename.to_str() {
                        files.push(filename_str.to_string());
                    }
                }
            }
        }
        Ok(files)
    }

    // Step 2: Load file content
    fn load_file(&mut self, filename: &str) -> io::Result<()> {
        let path = Path::new(filename);
        let mut file = fs::File::open(path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        self.loaded_files.insert(filename.to_string(), content);
        Ok(())
    }

    // Step 3: Create context with user question
    fn create_api_context(&self, question: &str) -> String {
        let mut context = String::new();
        
        // Add loaded file contents to context
        for (filename, content) in &self.loaded_files {
            context.push_str(&format!("File '{}' content:\n{}\n\n", filename, content));
        }

        // Add conversation history
        for (prev_question, prev_answer) in &self.conversation_history {
            context.push_str(&format!("Q: {}\nA: {}\n\n", prev_question, prev_answer));
        }

        // Add current question
        context.push_str(&format!("Current question: {}", question));
        
        context
    }

    // Step 4: Send to API (mock implementation)
    fn send_to_api(&mut self, context: &str) -> String {
        // This is where you'd implement your actual API call
        // For this example, we'll just return a mock response
        let response = format!("Mock API response for context: {}", context);
        response
    }

    // Step 5: Save response
    fn save_response(&mut self, question: String, answer: String) {
        self.conversation_history.push((question, answer));
    }
}

fn main() -> io::Result<()> {
    let mut chat_context = ChatContext::new();
    
    // Step 1: List files
    println!("Available files in current directory:");
    let files = chat_context.list_files()?;
    for file in &files {
        println!("- {}", file);
    }

    // Step 2: Load files (example with first file)
    if let Some(first_file) = files.first() {
        println!("\nLoading file: {}", first_file);
        chat_context.load_file(first_file)?;
    }

    // Step 3 & 4: Get user question and send to API
    println!("\nEnter your question:");
    let mut question = String::new();
    io::stdin().read_line(&mut question)?;
    question = question.trim().to_string();

    let context = chat_context.create_api_context(&question);
    let response = chat_context.send_to_api(&context);

    // Step 5: Save response
    chat_context.save_response(question, response.clone());

    println!("\nResponse: {}", response);

    Ok(())
}
```

To make this more interactive and useful, you might want to add a loop for multiple questions and more robust file handling. Here's an enhanced version:

```rust
use std::io::{self, Write};

fn main() -> io::Result<()> {
    let mut chat_context = ChatContext::new();
    
    loop {
        println!("\nAvailable commands:");
        println!("1. List files");
        println!("2. Load file");
        println!("3. Ask question");
        println!("4. Show conversation history");
        println!("5. Exit");
        print!("\nEnter command (1-5): ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        match input.trim() {
            "1" => {
                println!("\nAvailable files:");
                let files = chat_context.list_files()?;
                for file in &files {
                    println!("- {}", file);
                }
            },
            "2" => {
                print!("Enter filename to load: ");
                io::stdout().flush()?;
                let mut filename = String::new();
                io::stdin().read_line(&mut filename)?;
                match chat_context.load_file(filename.trim()) {
                    Ok(_) => println!("File loaded successfully!"),
                    Err(e) => println!("Error loading file: {}", e),
                }
            },
            "3" => {
                print!("Enter your question: ");
                io::stdout().flush()?;
                let mut question = String::new();
                io::stdin().read_line(&mut question)?;
                let question = question.trim().to_string();

                let context = chat_context.create_api_context(&question);
                let response = chat_context.send_to_api(&context);
                chat_context.save_response(question, response.clone());
                println!("\nResponse: {}", response);
            },
            "4" => {
                println!("\nConversation history:");
                for (i, (question, answer)) in chat_context.conversation_history.iter().enumerate() {
                    println!("\nQ{}: {}", i + 1, question);
                    println!("A{}: {}", i + 1, answer);
                }
            },
            "5" => break,
            _ => println!("Invalid command!"),
        }
    }

    Ok(())
}
```

To use this with a real API (like OpenAI's GPT), you'd need to:

1. Add the appropriate API client dependencies to your `Cargo.toml`
2. Implement the actual API call in the `send_to_api` method
3. Handle API responses and errors appropriately

For example, to use with OpenAI's API, you'd add:

```toml
[dependencies]
reqwest = { version = "0.11", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.0", features = ["full"] }
```

Then implement the actual API call:

```rust
async fn send_to_api(&mut self, context: &str) -> Result<String, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let response = client
        .post("https://api.openai.com/v1/completions")
        .header("Authorization", "Bearer YOUR_API_KEY")
        .json(&serde_json::json!({
            "model": "gpt-3.5-turbo",
            "messages": [
                {
                    "role": "system",
                    "content": context
                }
            ]
        }))
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;

    // Extract and return the response text
    Ok(response["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("No response")
        .to_string())
}
```

Remember to handle errors appropriately and never hardcode API keys in your code. Use environment variables or configuration files instead.
