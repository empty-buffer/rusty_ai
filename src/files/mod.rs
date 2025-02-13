use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

use crate::error::{Error, Result}; // use genai::Result;

// pub mod error;

pub fn list_files() -> Result<Vec<String>> {
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

pub fn list_current_dir(path: &PathBuf) -> Result<(Vec<String>, Vec<String>)> {
    let mut files: Vec<String> = Vec::new();
    let mut dirs: Vec<String> = Vec::new();

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();

        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if path.is_file() {
                files.push(name.to_string());
            } else if path.is_dir() {
                dirs.push(name.to_string());
            }
        }
    }

    Ok((files, dirs))
}

// Step 2: Load file content
pub fn load_file(filename: &str) -> Result<String> {
    let path = Path::new(filename);
    let mut file = fs::File::open(path)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;

    Ok(content)
}

pub fn change_dir(current_dir: &Path, path: &str) -> Result<PathBuf> {
    let new_path = if path == ".." {
        current_dir
            .parent()
            .ok_or_else(|| Error::Custom("Parent directory not found".to_string()))?
            .to_path_buf()
    } else {
        current_dir.join(path)
    };

    if new_path.is_dir() {
        Ok(new_path)
    } else {
        Err(Error::Custom("Path is not a directory".to_string()))
    }
}

// Step 3: Create context with user question
// pub fn create_api_context(&self, question: &str) -> String {
//     let mut context = String::new();

//     // Add loaded file contents to context
//     for (filename, content) in &self.loaded_files {
//         context.push_str(&format!("File '{}' content:\n{}\n\n", filename, content));
//     }

//     // Add conversation history
//     for (prev_question, prev_answer) in &self.conversation_history {
//         context.push_str(&format!("Q: {}\nA: {}\n\n", prev_question, prev_answer));
//     }

//     // Add current question
//     context.push_str(&format!("Current question: {}", question));

//     context
// }
