use crate::error::Result;
use chrono::Local;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct History {
    pub root: String,
    pub file_path: String,
}

impl History {
    pub fn new() -> Result<Self> {
        let history_dir = ".rusty";

        let now = Local::now();

        // Format date as dd.mm.yyyy
        let date_str = now.format("%d.%m.%Y").to_string();

        // Create filename
        let filename = format!("rusty_{}.md", date_str);

        let file_path = format!("{}/{}", history_dir, filename);

        if !Path::new(history_dir).exists() {
            fs::create_dir_all(history_dir)?;
        }

        let _file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(&file_path)?;
        Ok(Self {
            root: history_dir.to_owned(),
            file_path,
        })
    }

    pub fn new_file(&mut self, name: String) -> Result<()> {
        let file_path = format!("{}/{}", self.root, name);

        let _file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(&file_path)?;

        self.file_path = name;

        Ok(())
    }

    pub fn save_file(&mut self, content: String) -> Result<()> {
        fs::write(&self.file_path, content)?;

        Ok(())
    }

    pub fn save_to_file(&mut self, file_name: String, content: String) -> Result<()> {
        let file_path = format!("{}/{}", self.root, file_name);

        let _file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(&file_path)?;

        fs::write(file_path, content)?;

        self.file_path = file_name;

        Ok(())
    }

    pub fn current_file_content(&self) -> Result<String> {
        let current_path = format!("{}/{}", self.root, self.file_path);

        let mut file = OpenOptions::new().read(true).open(&current_path)?;

        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        Ok(contents)
    }
    pub fn load_file(&mut self, name: String) -> Result<String> {
        let file_path = format!("{}/{}", self.root, name);

        let mut file = OpenOptions::new().read(true).open(&file_path)?;

        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        self.file_path = file_path;

        Ok(contents)
    }
}

// pub fn add(&self, question: &str, response: &str) -> Result<()> {
//     let mut file = OpenOptions::new()
//         .create(true)
//         .append(true)
//         .open(&self.file_path)?;

//     writeln!(file, "Question: {}\n", question)?;
//     writeln!(file, "Response: {}\n", response)?;

//     Ok(())
// }
