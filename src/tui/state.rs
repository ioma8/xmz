use crate::parser::{stream_xml, Token, Break, Continue};
use ratatui::widgets::ListState;
use ratatui::widgets::ScrollbarState;
use std::collections::HashMap;

#[derive(Clone)]
pub struct Level {
    pub tag: Option<String>,
    pub children: Vec<(String, Option<String>)>, // (tag, text_if_leaf)
}

pub struct TuiState {
    pub stack: Vec<Level>,
    pub selected: usize,
    pub list_state: ListState,
    children_cache: HashMap<Option<String>, Vec<(String, Option<String>)>>,
    xml: String,
    pub scrollbar_state: ScrollbarState,
    pub items_len: usize,
}

impl TuiState {
    pub fn new(xml: &str) -> Self {
        let root_tag = get_root_tag(xml);
        let children = match &root_tag {
            Some(tag) => vec![(tag.clone(), None)],
            None => vec![],
        };
        let items_len = children.len();

        Self {
            stack: vec![Level {
                tag: None,
                children,
            }],
            selected: 0,
            list_state: ListState::default(),
            children_cache: HashMap::new(),
            xml: xml.to_string(),
            scrollbar_state: ScrollbarState::default(),
            items_len,
        }
    }

    pub fn get_current_level(&self) -> &Level {
        self.stack.last().unwrap()
    }

    pub fn go_down(&mut self) {
        let current = self.get_current_level();
        if self.selected + 1 < current.children.len() {
            self.selected += 1;
        }
        self.list_state.select(Some(self.selected));
        self.scrollbar_state = self.scrollbar_state.position(self.selected);
    }

    pub fn go_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
        self.list_state.select(Some(self.selected));
        self.scrollbar_state = self.scrollbar_state.position(self.selected);
    }

    pub fn enter(&mut self) {
        let current = self.get_current_level().clone();
        if let Some((tag, _)) = current.children.get(self.selected) {
            let children = get_children_cached(&self.xml, Some(tag), &mut self.children_cache);
            self.items_len = children.len();
            self.stack.push(Level {
                tag: Some(tag.clone()),
                children,
            });
            self.selected = 0;
            self.list_state.select(Some(self.selected));
        }
    }

    pub fn back(&mut self) {
        if self.stack.len() > 1 {
            self.stack.pop();
            self.selected = 0;
            self.list_state.select(Some(self.selected));
            self.items_len = self.get_current_level().children.len();
        }
    }
}

fn get_root_tag(xml: &str) -> Option<String> {
    let mut root_tag = None;
    stream_xml(xml, |token| {
        if let Token::StartTag(name) = token {
            root_tag = Some(name.to_string());
            return Break(());
        }
        Continue(())
    });
    root_tag
}

fn get_children_cached(
    xml: &str,
    parent_tag: Option<&str>,
    cache: &mut HashMap<Option<String>, Vec<(String, Option<String>)>>,
) -> Vec<(String, Option<String>)> {
    let key = parent_tag.map(|s| s.to_string());
    if let Some(cached) = cache.get(&key) {
        return cached.clone();
    }
    let mut children = Vec::new();
    let mut depth = 0;
    let mut inside = parent_tag.is_none();
    let mut parent_matched = false;
    let mut last_tag: Option<String> = None;
    let mut last_text: Option<String> = None;
    let mut collecting_text = false;
    stream_xml(xml, |token| {
        match token {
            Token::StartTag(name) => {
                if let Some(parent) = parent_tag {
                    if !inside && name == parent {
                        inside = true;
                        parent_matched = true;
                        return Continue(());
                    }
                    if inside {
                        if depth == 0 {
                            last_tag = Some(name.to_string());
                            last_text = None;
                            collecting_text = true;
                        }
                        depth += 1;
                    }
                } else {
                    if depth == 0 {
                        last_tag = Some(name.to_string());
                        last_text = None;
                        collecting_text = true;
                    }
                    depth += 1;
                }
            }
            Token::EndTag(name) => {
                if let Some(parent) = parent_tag {
                    if inside {
                        if depth > 0 {
                            depth -= 1;
                        }
                        if depth == 0 && name == parent && parent_matched {
                            return Break(());
                        }
                        if depth == 0 && collecting_text {
                            if let Some(tag) = last_tag.take() {
                                children.push((tag, last_text.take()));
                            }
                            collecting_text = false;
                        }
                    }
                } else {
                    if depth > 0 {
                        depth -= 1;
                    }
                    if depth == 0 && collecting_text {
                        if let Some(tag) = last_tag.take() {
                            children.push((tag, last_text.take()));
                        }
                        collecting_text = false;
                    }
                }
            }
            Token::Text(txt) => {
                if collecting_text && depth == 1 {
                    let t = txt.trim();
                    if !t.is_empty() {
                        if let Some(existing) = &mut last_text {
                            existing.push_str(t);
                        } else {
                            last_text = Some(t.to_string());
                        }
                    }
                }
            }
        }
        Continue(())
    });
    cache.insert(key, children.clone());
    children
}
