use derive_more::From;
use ollama_rs::error::OllamaError;

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, From)]
pub enum Error {
    Exit,
    #[from]
    Custom(String),

    #[from]
    Io(std::io::Error),

    #[from]
    Json(serde_json::Error),

    // #[from]c
    // History(crate::chat::history)
    #[from]
    InquireError(inquire::InquireError),

    #[from]
    Ollama(OllamaError),

    #[from]
    TreeSitter(tree_sitter::QueryError),
}

impl From<&str> for Error {
    fn from(value: &str) -> Self {
        Self::Custom(value.to_string())
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::result::Result<(), core::fmt::Error> {
        write!(fmt, "{self:?}")
    }
}

impl std::error::Error for Error {}
