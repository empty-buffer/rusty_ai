use super::Style;
use std::collections::{HashMap, HashSet};

pub struct SyntaxCache {
    // Track which lines have been highlighted and their results
    pub line_styles: HashMap<usize, Vec<Style>>,
    // Track which lines are dirty and need rehighlighting
    pub dirty_lines: HashSet<usize>,
    // Store the last length of content to detect full-document changes
    pub last_content_length: usize,
}

impl SyntaxCache {
    pub fn new() -> Self {
        Self {
            line_styles: HashMap::new(),
            dirty_lines: HashSet::new(),
            last_content_length: 0,
        }
    }

    pub fn mark_line_dirty(&mut self, line_number: usize) {
        self.dirty_lines.insert(line_number);
    }

    pub fn mark_range_dirty(&mut self, start_line: usize, end_line: usize) {
        for line in start_line..=end_line {
            self.dirty_lines.insert(line);
        }
    }

    pub fn mark_all_dirty(&mut self) {
        self.line_styles.clear();
        self.dirty_lines.clear();
    }

    // pub fn get_cached_style(&self, line_number: usize, char_index: usize) -> Option<Style> {
    //     self.line_styles
    //         .get(&line_number)
    //         .and_then(|styles| styles.get(char_index).cloned())
    // }

    pub fn get_cached_style(&self, line_number: usize, col: usize) -> Option<Style> {
        self.line_styles.get(&line_number).and_then(|styles| {
            if col < styles.len() {
                Some(styles[col])
            } else {
                None
            }
        })
    }

    pub fn cache_line_styles(&mut self, line_number: usize, styles: Vec<Style>) {
        self.line_styles.insert(line_number, styles);
        self.dirty_lines.remove(&line_number);
    }

    pub fn is_line_cached(&self, line_number: usize) -> bool {
        self.line_styles.contains_key(&line_number) && !self.dirty_lines.contains(&line_number)
    }
}
