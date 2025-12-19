use crate::parser::{stream_xml, Token, Break, Continue};
use ratatui::widgets::ListState;
use ratatui::widgets::ScrollbarState;

/// A child entry: (tag_name, optional_text_content)
type ChildEntry<'a> = (&'a str, Option<&'a str>);

/// Cache entry: (parent_tag, children_list)
type CacheEntry<'a> = (Option<&'a str>, Vec<ChildEntry<'a>>);

/// A level in the XML tree navigation.
/// Uses references into the original XML to avoid allocations.
pub struct Level<'a> {
    pub tag: Option<&'a str>,
    pub children: Vec<ChildEntry<'a>>,
}

pub struct TuiState<'a> {
    pub stack: Vec<Level<'a>>,
    pub selected: usize,
    pub list_state: ListState,
    children_cache: Vec<CacheEntry<'a>>,
    xml: &'a str,
    pub scrollbar_state: ScrollbarState,
    pub items_len: usize,
}

impl<'a> TuiState<'a> {
    pub fn new(xml: &'a str) -> Self {
        let root_tag = get_root_tag(xml);
        let children = match root_tag {
            Some(tag) => vec![(tag, None)],
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
            children_cache: Vec::new(),
            xml,
            scrollbar_state: ScrollbarState::default(),
            items_len,
        }
    }

    pub fn get_current_level(&self) -> &Level<'a> {
        self.stack.last().unwrap()
    }

    /// Returns the number of children at the current level
    fn current_children_len(&self) -> usize {
        self.stack.last().map_or(0, |l| l.children.len())
    }

    pub fn go_down(&mut self) {
        let len = self.current_children_len();
        if self.selected + 1 < len {
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
        // Get the selected tag without holding a borrow on self
        let selected_tag = self.stack
            .last()
            .and_then(|level| level.children.get(self.selected))
            .map(|(tag, _)| *tag);

        if let Some(tag) = selected_tag {
            let children = get_children_cached(self.xml, Some(tag), &mut self.children_cache);
            self.items_len = children.len();
            self.stack.push(Level {
                tag: Some(tag),
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
            self.items_len = self.current_children_len();
        }
    }
}

fn get_root_tag(xml: &str) -> Option<&str> {
    let mut root_tag = None;
    stream_xml(xml, |token| {
        if let Token::StartTag(name) = token {
            root_tag = Some(name);
            return Break(());
        }
        Continue(())
    });
    root_tag
}

/// Gets children of a parent tag, using cache to avoid re-parsing.
/// Returns references into the original XML string.
fn get_children_cached<'a>(
    xml: &'a str,
    parent_tag: Option<&'a str>,
    cache: &mut Vec<CacheEntry<'a>>,
) -> Vec<ChildEntry<'a>> {
    // Linear search in cache (typically small number of entries)
    for (key, children) in cache.iter() {
        if *key == parent_tag {
            return children.clone();
        }
    }
    
    let children = get_children(xml, parent_tag);
    cache.push((parent_tag, children.clone()));
    children
}

/// Parses the XML to extract direct children of the given parent tag.
/// Returns references into the original XML string (zero-allocation for tag names).
fn get_children<'a>(xml: &'a str, parent_tag: Option<&str>) -> Vec<ChildEntry<'a>> {
    let mut children = Vec::new();
    let mut depth = 0;
    let mut inside = parent_tag.is_none();
    let mut parent_matched = false;
    let mut last_tag: Option<&'a str> = None;
    let mut last_text: Option<&'a str> = None;
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
                            last_tag = Some(name);
                            last_text = None;
                            collecting_text = true;
                        }
                        depth += 1;
                    }
                } else {
                    if depth == 0 {
                        last_tag = Some(name);
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
                // Note: For text content, we keep the reference to avoid allocation.
                // If multiple text nodes exist, we only keep the first one (simplified).
                if collecting_text && depth == 1 && last_text.is_none() {
                    let t = txt.trim();
                    if !t.is_empty() {
                        last_text = Some(t);
                    }
                }
            }
        }
        Continue(())
    });
    
    children
}
