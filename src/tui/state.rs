use crate::parser::{stream_xml, extract_attributes, Token, Break, Continue};
use ratatui::widgets::ListState;
use ratatui::widgets::ScrollbarState;

/// A child entry: (tag_name, optional_text_content, offset, attributes_raw)
type ChildEntry<'a> = (&'a str, Option<&'a str>, usize, &'a str);

/// Cache entry: (parent_offset, children_list)
type CacheEntry<'a> = (usize, Vec<ChildEntry<'a>>);

/// Info data: (attributes, children_count)
pub type InfoData<'a> = (Vec<(&'a str, &'a str)>, usize);

/// A level in the XML tree navigation.
/// Uses references into the original XML to avoid allocations.
pub struct Level<'a> {
    pub tag: Option<&'a str>,
    pub children: Vec<ChildEntry<'a>>,
    pub last_selected: usize,
}

pub struct TuiState<'a> {
    pub stack: Vec<Level<'a>>,
    pub selected: usize,
    pub list_state: ListState,
    children_cache: Vec<CacheEntry<'a>>,
    xml: &'a str,
    pub scrollbar_state: ScrollbarState,
    pub items_len: usize,
    pub show_info_popup: bool,
    pub info_popup_data: Option<InfoData<'a>>,
}

impl<'a> TuiState<'a> {
    pub fn new(xml: &'a str) -> Self {
        let (root_tag, root_offset, root_attrs) = match get_root_tag(xml) {
            Some(res) => (Some(res.0), res.1, res.2),
            None => (None, 0, ""),
        };

        let children = match root_tag {
            Some(tag) => vec![(tag, None, root_offset, root_attrs)],
            None => vec![],
        };
        let items_len = children.len();

        Self {
            stack: vec![Level {
                tag: None,
                children,
                last_selected: 0,
            }],
            selected: 0,
            list_state: ListState::default(),
            children_cache: Vec::new(),
            xml,
            scrollbar_state: ScrollbarState::default(),
            items_len,
            show_info_popup: false,
            info_popup_data: None,
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

    pub fn page_down(&mut self) {
        let len = self.current_children_len();
        self.selected = (self.selected + 10).min(len.saturating_sub(1));
        self.list_state.select(Some(self.selected));
        self.scrollbar_state = self.scrollbar_state.position(self.selected);
    }

    pub fn page_up(&mut self) {
        self.selected = self.selected.saturating_sub(10);
        self.list_state.select(Some(self.selected));
        self.scrollbar_state = self.scrollbar_state.position(self.selected);
    }

    pub fn home(&mut self) {
        self.selected = 0;
        self.list_state.select(Some(self.selected));
        self.scrollbar_state = self.scrollbar_state.position(self.selected);
    }

    pub fn end(&mut self) {
        let len = self.current_children_len();
        self.selected = len.saturating_sub(1);
        self.list_state.select(Some(self.selected));
        self.scrollbar_state = self.scrollbar_state.position(self.selected);
    }

    pub fn enter(&mut self) {
        // Get the selected tag and offset without holding a borrow on self
        let selected_child = self.stack
            .last()
            .and_then(|level| level.children.get(self.selected))
            .map(|(tag, _, offset, _)| (*tag, *offset));

        if let Some((tag, offset)) = selected_child {
            // Save current selection to the current level before pushing new one
            if let Some(current) = self.stack.last_mut() {
                current.last_selected = self.selected;
            }

            let children = get_children_cached(self.xml, offset, Some(tag), &mut self.children_cache);
            self.items_len = children.len();
            self.stack.push(Level {
                tag: Some(tag),
                children,
                last_selected: 0,
            });
            self.selected = 0;
            self.list_state.select(Some(self.selected));
        }
    }

    pub fn back(&mut self) {
        if self.stack.len() > 1 {
            self.stack.pop();
            // Restore selection from the now-current level
            self.selected = self.stack.last().map_or(0, |l| l.last_selected);
            self.list_state.select(Some(self.selected));
            self.items_len = self.current_children_len();
        }
    }

    pub fn toggle_info(&mut self) {
        if self.show_info_popup {
            self.show_info_popup = false;
            self.info_popup_data = None;
            return;
        }

        // Get the selected tag and offset without holding a borrow on self
        let selected_child = self.stack
            .last()
            .and_then(|level| level.children.get(self.selected))
            .map(|(tag, _, offset, _)| (*tag, *offset));

        if let Some((tag, offset)) = selected_child {
            let attributes = extract_attributes(self.xml, offset);
            
            // Get children count. This might parse children if not in cache.
            // We use the existing cache helper.
            let children = get_children_cached(self.xml, offset, Some(tag), &mut self.children_cache);
            let child_count = children.len();

            self.info_popup_data = Some((attributes, child_count));
            self.show_info_popup = true;
        }
    }
}

fn get_root_tag(xml: &str) -> Option<(&str, usize, &str)> {
    let mut root = None;
    stream_xml(xml, |token| {
        if let Token::StartTag(name, attrs) = token {
            // Subtract 1 to include the '<'
            root = Some((name, bytes_offset(xml, name).saturating_sub(1), attrs));
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
    let mut last_attrs: &'a str = "";
    let mut last_text: Option<&'a str> = None;
    let mut collecting_text = false;
    
    stream_xml(slice, |token| {
        match token {
            Token::StartTag(name, attrs) => {
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
                        last_attrs = attrs;
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
                            children.push((tag, last_text.take(), last_tag_offset, last_attrs));
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
