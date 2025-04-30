use crate::error::Result;
use regex::Regex;
use ropey::Rope;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;
use tree_sitter::{Language, Parser, Query, QueryCursor, StreamingIterator, Tree};
use tree_sitter_language::LanguageFn;

pub mod cache;

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
    Selection,
}

pub struct SyntaxHighlighter {
    parser: RefCell<Parser>,
    languages: HashMap<String, LanguageFn>,
    queries: HashMap<Language, Query>,
    md_code_block_regex: Regex,
}

impl SyntaxHighlighter {
    pub fn new() -> Result<Self> {
        let mut parser = Parser::new();

        // Initialize languages map
        let mut languages = HashMap::new();
        let mut queries = HashMap::new();

        // Register Rust language
        let rust_language = tree_sitter_rust::LANGUAGE;
        languages.insert("rust".to_string(), rust_language);

        // Rust highlight query - simplified for demonstration
        let rust_query = Query::new(&rust_language.into(), tree_sitter_rust::HIGHLIGHTS_QUERY)?;
        queries.insert(rust_language.into(), rust_query);

        // Add other languages as needed
        // ... (Python, JavaScript, etc.)

        let md_code_block_regex = Regex::new(r"```(\w+)").unwrap();

        Ok(Self {
            parser: RefCell::new(parser),
            languages,
            queries,
            md_code_block_regex,
        })
    }

    pub fn detect_language_from_content(&self, buffer: &Rope) -> Option<&LanguageFn> {
        let content = buffer.to_string();

        // Check for markdown code blocks
        if let Some(captures) = self.md_code_block_regex.captures(&content) {
            if let Some(lang_match) = captures.get(1) {
                let lang_name = lang_match.as_str().to_lowercase();
                return self.languages.get(&lang_name);
            }
        }

        None
    }

    pub fn detect_language(&self, filename: &str) -> Option<&LanguageFn> {
        let extension = Path::new(filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        self.languages.get(extension)
    }

    pub fn highlight_bufferv1(
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

    // pub fn convert_highlights_to_char_ranges(
    //     &self,
    //     buffer: &Rope,
    //     highlights: Vec<(Range<usize>, Style)>,
    // ) -> Vec<(Range<usize>, Style)> {
    //     highlights
    //         .into_iter()
    //         .map(|(range, style)| {
    //             let start_char = buffer.byte_to_char(range.start);
    //             let end_char = buffer.byte_to_char(range.end);
    //             (start_char..end_char, style)
    //         })
    //         .collect()
    // }

    pub fn highlight_buffer(
        &self,
        buffer: &Rope,
        language: Option<&LanguageFn>,
    ) -> Vec<(Range<usize>, Style)> {
        // Try to detect language from content if none specified
        let content_language = self.detect_language_from_content(buffer);
        let language = language.or(content_language);

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

        // If this is a Markdown file with code blocks, extract the code block content
        let code_text = if self.md_code_block_regex.is_match(&text) {
            self.extract_code_block(&text)
        } else {
            text.clone()
        };

        // Parse the buffer
        let tree = match parser.parse(&code_text, None) {
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

        let mut matches = cursor.matches(query, tree.root_node(), code_text.as_bytes());

        // Process matches
        while let Some(match_) = matches.next() {
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

                // If this is a Markdown file, adjust the range to account for code block position
                let adjusted_range = if self.md_code_block_regex.is_match(&text) {
                    self.adjust_range_for_code_block(&text, range)
                } else {
                    range
                };

                highlights.push((adjusted_range, style));
            }
        }

        highlights
    }

    fn extract_code_block(&self, text: &str) -> String {
        let lines: Vec<&str> = text.lines().collect();
        let mut in_code_block = false;
        let mut code_lines = Vec::new();
        let mut language = "";

        for line in lines {
            if line.starts_with("```") && !in_code_block {
                // Capture language identifier
                if let Some(captures) = self.md_code_block_regex.captures(line) {
                    if let Some(lang_match) = captures.get(1) {
                        language = lang_match.as_str();
                    }
                }
                in_code_block = true;
                continue;
            }

            if line.starts_with("```") && in_code_block {
                in_code_block = false;
                continue;
            }

            if in_code_block {
                code_lines.push(line);
            }
        }

        code_lines.join("\n")
    }

    // Adjust highlight ranges to account for code block position in Markdown
    fn adjust_range_for_code_block(&self, text: &str, range: Range<usize>) -> Range<usize> {
        let lines: Vec<&str> = text.lines().collect();
        let mut in_code_block = false;
        let mut offset = 0;
        let mut code_start_offset = 0;

        for line in lines {
            if line.starts_with("```") && !in_code_block {
                in_code_block = true;
                code_start_offset = offset + line.len() + 1; // +1 for newline
                continue;
            }

            if in_code_block {
                break;
            }

            offset += line.len() + 1; // +1 for newline
        }

        (range.start + code_start_offset)..(range.end + code_start_offset)
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
