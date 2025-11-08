use crate::error::Result;
use regex::Regex;
use ropey::Rope;
use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;
use std::{cell::RefCell, fs::OpenOptions};
use tree_sitter::{Language, Parser, Query, QueryCursor, StreamingIterator, Tree};
use tree_sitter_language::LanguageFn;

use std::io::Write;
pub mod cache;

struct CodeBlock {
    language: String,
    start: usize,
    end: usize,
    code: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Normal,
    Keyword,
    Function,
    Constructor,
    Property,
    Type,
    Builtin,
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

        let go_language = tree_sitter_go::LANGUAGE;
        languages.insert("go".to_string(), go_language);

        // Rust highlight query - simplified for demonstration
        let rust_query = Query::new(&rust_language.into(), tree_sitter_rust::HIGHLIGHTS_QUERY)?;
        queries.insert(rust_language.into(), rust_query);

        let go_query = Query::new(&go_language.into(), tree_sitter_go::HIGHLIGHTS_QUERY)?;
        queries.insert(go_language.into(), go_query);

        let md_code_block_regex = Regex::new(r"(?m)^```([\w\+\-]+)").unwrap();

        Ok(Self {
            parser: RefCell::new(parser),
            languages,
            queries,
            md_code_block_regex,
        })
    }

    pub fn detect_language(&self, filename: &str) -> Option<&LanguageFn> {
        let extension = Path::new(filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("rust");

        self.languages.get(extension)
    }

    fn extract_code_blocks(&self, text: &str) -> Vec<CodeBlock> {
        let mut blocks = Vec::new();
        let lines: Vec<&str> = text.lines().collect();
        let mut in_code_block = false;
        let mut current_lang = "";
        let mut code_start_byte = 0;
        let mut code_buffer = Vec::new();

        let mut offset = 0; // byte offset to track where lines start

        for line in &lines {
            if line.starts_with("```") {
                if !in_code_block {
                    // Entering a code block
                    if let Some(caps) = self.md_code_block_regex.captures(line) {
                        if let Some(lang_match) = caps.get(1) {
                            current_lang = lang_match.as_str();
                        } else {
                            current_lang = "";
                        }
                    } else {
                        current_lang = "";
                    }
                    in_code_block = true;
                    code_start_byte = offset + line.len() + 1; // start after this line + newline
                    code_buffer.clear();
                } else {
                    // Exiting code block
                    let code_len = code_buffer
                        .iter()
                        .map(|l: &&str| l.len() + 1)
                        .sum::<usize>(); // Add newlines too
                    let code_end_byte = code_start_byte + code_len;

                    blocks.push(CodeBlock {
                        language: current_lang.to_lowercase(),
                        start: code_start_byte,
                        end: code_end_byte,
                        code: code_buffer.join("\n"),
                    });

                    in_code_block = false;
                    current_lang = "";
                }
            } else if in_code_block {
                code_buffer.push(*line);
            }

            offset += line.len() + 1; // +1 for newline
        }

        blocks
    }

    pub fn highlight_buffer(
        &self,
        buffer: &Rope,
        language: Option<&LanguageFn>,
    ) -> Vec<(Range<usize>, Style)> {
        let text = buffer.to_string();

        let mut log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open("log.txt")
            .expect("Failed to open log file");

        // For Markdown, extract all code blocks with language and positions
        let code_blocks = self.extract_code_blocks(&text);

        let mut highlights = Vec::new();

        // Highlight inside each code block
        for block in code_blocks {
            // Check if we have this language registered
            if let Some(lang_fn) = self.languages.get(&block.language) {
                // Setup parser
                let mut parser = self.parser.borrow_mut();
                if parser.set_language(&(*lang_fn).into()).is_err() {
                    continue; // skip unknown languages
                }

                // Parse the code block
                if let Some(tree) = parser.parse(&block.code, None) {
                    if let Some(query) = self.queries.get(&(*lang_fn).into()) {
                        let mut cursor = QueryCursor::new();

                        let mut matches =
                            cursor.matches(query, tree.root_node(), block.code.as_bytes());

                        while let Some(match_) = matches.next() {
                            for capture in match_.captures {
                                let node = capture.node;
                                if node.start_byte() == node.end_byte() {
                                    continue;
                                }
                                let range = node.start_byte()..node.end_byte();

                                let style = match query.capture_names()[capture.index as usize] {
                                    "keyword" => Style::Keyword,
                                    "function" | "function.macro" | "function.method" => {
                                        Style::Function
                                    }
                                    "punctuation.delimiter" | "punctuation.bracket" => {
                                        Style::Normal
                                    }
                                    "type" => Style::Type,
                                    "type.builtin" | "constant.builtin" => Style::Builtin,
                                    "string" => Style::String,
                                    "number" => Style::Number,
                                    "comment" => Style::Comment,
                                    "variable" | "variable.field" | "variable.builtin" => {
                                        Style::Variable
                                    }
                                    "constant" => Style::Constant,
                                    "operator" => Style::Operator,

                                    "constructor" => Style::Constructor,
                                    "property" => Style::Property,
                                    "variable.parameter" => Style::Normal,
                                    other => {
                                        writeln!(log_file, "Normal ({})", other)
                                            .expect("Failed to write to log file");
                                        Style::Normal
                                    }
                                };
                                // Shift ranges by code block start offset
                                let adjusted_range =
                                    (range.start + block.start)..(range.end + block.start);

                                highlights.push((adjusted_range, style));
                            }
                        }
                    }
                }
            }
        }

        // Add highlighting for Markdown syntax outside code blocks

        highlights
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
