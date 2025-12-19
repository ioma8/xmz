use crate::parser::{stream_xml, Token, Break, Continue};
use ratatui::widgets::ListState;
use ratatui::widgets::ScrollbarState;

/// A child entry: (tag_name, optional_text_content, offset)
type ChildEntry<'a> = (&'a str, Option<&'a str>, usize);

/// Cache entry: (parent_offset, children_list)
type CacheEntry<'a> = (usize, Vec<ChildEntry<'a>>);

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
        let (root_tag, root_offset) = match get_root_tag(xml) {
            Some(res) => (Some(res.0), res.1),
            None => (None, 0),
        };

        let children = match root_tag {
            Some(tag) => vec![(tag, None, root_offset)],
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
        // Get the selected tag and offset without holding a borrow on self
        let selected_child = self.stack
            .last()
            .and_then(|level| level.children.get(self.selected))
            .map(|(tag, _, offset)| (*tag, *offset));

        if let Some((tag, offset)) = selected_child {
            let children = get_children_cached(self.xml, offset, Some(tag), &mut self.children_cache);
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

fn get_root_tag(xml: &str) -> Option<(&str, usize)> {
    let mut root = None;
    stream_xml(xml, |token| {
        if let Token::StartTag(name) = token {
            // Subtract 1 to include the '<'
            root = Some((name, bytes_offset(xml, name).saturating_sub(1)));
            return Break(());
        }
        Continue(())
    });
    root
}

/// Gets children of a parent tag, using cache to avoid re-parsing.
/// Uses offset to jump directly to parent location.
fn get_children_cached<'a>(
    xml: &'a str,
    offset: usize,
    parent_tag: Option<&'a str>,
    cache: &mut Vec<CacheEntry<'a>>,
) -> Vec<ChildEntry<'a>> {
    // Linear search in cache by offset
    for (key_offset, children) in cache.iter() {
        if *key_offset == offset {
            return children.clone();
        }
    }
    
    let children = get_children(xml, offset, parent_tag);
    cache.push((offset, children.clone()));
    children
}

/// Parses the XML to extract direct children of the tag at the given offset.
fn get_children<'a>(
    xml: &'a str,
    offset: usize,
    parent_tag: Option<&str>
) -> Vec<ChildEntry<'a>> {
    let mut children = Vec::new();
    let mut depth = 0;
    
    // Slice from the offset. We expect this to start with '<'
    let slice = if offset < xml.len() {
        &xml[offset..]
    } else {
        ""
    };

    let mut inside = false;
    let mut parent_matched = false;
    let mut last_tag: Option<&'a str> = None;
    let mut last_tag_offset: usize = 0;
    let mut last_text: Option<&'a str> = None;
    let mut collecting_text = false;
    
    stream_xml(slice, |token| {
        match token {
            Token::StartTag(name) => {
                if !inside {
                    if let Some(parent) = parent_tag {
                        if name == parent {
                            inside = true;
                            parent_matched = true;
                            return Continue(());
                        }
                    } else {
                        inside = true; 
                    }
                } else {
                    // Inside parent
                    if depth == 0 {
                        last_tag = Some(name);
                        // Subtract 1 to point to '<'
                        last_tag_offset = bytes_offset(xml, name).saturating_sub(1);
                        last_text = None;
                        collecting_text = true;
                    }
                    depth += 1;
                }
            }
            Token::EndTag(name) => {
                if inside {
                    if depth > 0 {
                        depth -= 1;
                    }
                    if depth == 0 && Some(name) == parent_tag && parent_matched {
                        return Break(());
                    }
                    if depth == 0 && collecting_text {
                        if let Some(tag) = last_tag.take() {
                            children.push((tag, last_text.take(), last_tag_offset));
                        }
                        collecting_text = false;
                    }
                }
            }
            Token::Text(txt) => {
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

fn bytes_offset(base: &str, slice: &str) -> usize {
    let base_start = base.as_ptr() as usize;
    let slice_start = slice.as_ptr() as usize;
    if slice_start < base_start || slice_start > base_start + base.len() {
        // This should not happen if slice is part of base
        0
    } else {
        slice_start - base_start
    }
}
