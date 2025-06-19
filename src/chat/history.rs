use crate::error::Result;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct History {
    // pub root: String,
    pub file_path: String,
}

impl History {
    pub fn new() -> Result<Self> {
        let history_dir = ".rusty";
        let file_path = format!("{}/history.md", history_dir);

        if !Path::new(history_dir).exists() {
            fs::create_dir_all(history_dir)?;
        }

        // asd asd
        let _file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(&file_path)?;
        Ok(Self {
            // root: "./".to_owned(),
            file_path,
        })
    }

    pub fn add(&self, question: &str, response: &str) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)?;

        writeln!(file, "Question: {}\n", question)?;
        writeln!(file, "Response: {}\n", response)?;

        Ok(())
    }

    pub fn load(&self) -> Result<String> {
        let mut file = OpenOptions::new().read(true).open(&self.file_path)?;

        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        Ok(contents)
    }
}
