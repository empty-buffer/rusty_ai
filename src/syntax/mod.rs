use crate::error::Result;
use ropey::Rope;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;
use tree_sitter::{Language, Parser, Query, QueryCursor, StreamingIterator, Tree};
use tree_sitter_language::LanguageFn;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Normal,
    Keyword,
    Function,
    Type,
    String,
    Number,
    Comment,
    Variable,
    Constant,
    Operator,
    Error,
}

pub struct SyntaxHighlighter {
    parser: RefCell<Parser>,
    languages: HashMap<String, LanguageFn>,
    queries: HashMap<Language, Query>,
}

impl SyntaxHighlighter {
    pub fn new() -> Result<Self> {
        let mut parser = Parser::new();

        // Initialize languages map
        let mut languages = HashMap::new();
        let mut queries = HashMap::new();

        // let mut parser = Parser::new();
        // let language = tree_sitter_rust::LANGUAGE;
        // parser
        //     .set_language(&language.into())
        //     .expect("Error loading Rust parser");

        // tree_sitter_rust::HIGHLIGHTS_QUERY;

        // let tree = parser.parse(code, None).unwrap();

        // Register Rust language
        let rust_language = tree_sitter_rust::LANGUAGE;
        languages.insert("rs".to_string(), rust_language);

        // Rust highlight query - simplified for demonstration
        let rust_query = Query::new(&rust_language.into(), tree_sitter_rust::HIGHLIGHTS_QUERY)?;
        queries.insert(rust_language.into(), rust_query);

        // Add other languages as needed
        // ... (Python, JavaScript, etc.)

        Ok(Self {
            parser: RefCell::new(parser),
            languages,
            queries,
        })
    }

    pub fn detect_language(&self, filename: &str) -> Option<&LanguageFn> {
        let extension = Path::new(filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        self.languages.get(extension)
    }

    pub fn highlight_buffer(
        &self,
        buffer: &Rope,
        language: Option<&LanguageFn>,
    ) -> Vec<(Range<usize>, Style)> {
        // Default to no highlighting if language not specified
        let language = match language {
            Some(lang) => lang,
            None => return Vec::new(),
        };

        let mut parser = self.parser.borrow_mut();
        parser
            .set_language(&(*language).into())
            .unwrap_or_else(|_| {
                // Return empty result if language fails to set
                return;
            });

        // Convert rope to string (this can be optimized for large files)
        let text = buffer.to_string();

        // Parse the buffer
        let tree = match parser.parse(&text, None) {
            Some(tree) => tree,
            None => return Vec::new(),
        };

        // Get query for this language
        let query = match self.queries.get(&(*language).into()) {
            Some(query) => query,
            None => return Vec::new(),
        };

        let mut cursor = QueryCursor::new();
        let mut highlights = Vec::new();

        let mut matches = cursor.matches(query, tree.root_node(), text.as_bytes());
        // }

        // let matches = cursor.matches(query, tree.root_node(), text.as_bytes());
        while let Some(match_) = matches.next() {
            // for match_ in matches {
            for capture in match_.captures {
                let node = capture.node;

                // Skip zero-width nodes
                if node.start_byte() == node.end_byte() {
                    continue;
                }

                let range = node.start_byte()..node.end_byte();

                // Map capture names to styles
                let style = match query.capture_names()[capture.index as usize] {
                    "keyword" => Style::Keyword,
                    "function" | "function.macro" => Style::Function,
                    "type" => Style::Type,
                    "string" => Style::String,
                    "number" => Style::Number,
                    "comment" => Style::Comment,
                    "variable" | "variable.field" | "variable.builtin" => Style::Variable,
                    "constant" => Style::Constant,
                    "operator" => Style::Operator,
                    _ => Style::Normal,
                };

                highlights.push((range, style));
            }
        }

        highlights
    }

    pub fn convert_highlights_to_char_ranges(
        &self,
        buffer: &Rope,
        highlights: Vec<(Range<usize>, Style)>,
    ) -> Vec<(Range<usize>, Style)> {
        highlights
            .into_iter()
            .map(|(range, style)| {
                let start_char = buffer.byte_to_char(range.start);
                let end_char = buffer.byte_to_char(range.end);
                (start_char..end_char, style)
            })
            .collect()
    }
}
