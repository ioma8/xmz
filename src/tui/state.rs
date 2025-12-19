use crate::xml::{Node, XmlExplorer};
use ratatui::widgets::ListState;
use ratatui::widgets::ScrollbarState;

/// Info data: (attributes, children_count)
pub type InfoData<'a> = (Vec<(&'a str, &'a str)>, usize);

/// A level in the XML tree navigation.
pub struct Level<'a> {
    pub tag: Option<&'a str>,
    pub children: Vec<Node<'a>>,
    pub last_selected: usize,
}

pub struct TuiState<'a> {
    pub stack: Vec<Level<'a>>,
    pub selected: usize,
    pub list_state: ListState,
    pub explorer: XmlExplorer<'a>,
    pub scrollbar_state: ScrollbarState,
    pub items_len: usize,
    pub show_info_popup: bool,
    pub info_popup_data: Option<InfoData<'a>>,
}

impl<'a> TuiState<'a> {
    pub fn new(xml: &'a str) -> Self {
        let explorer = XmlExplorer::new(xml);

        let children = match explorer.root() {
            Some(node) => vec![node],
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
            explorer,
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
        // Get the selected node without holding a borrow on self
        // Note: we need to clone the node structure (it's just references and usize)
        // to pass it to the explorer which needs a fresh borrow of self.xml via self.explorer
        let selected_node = self
            .stack
            .last()
            .and_then(|level| level.children.get(self.selected))
            .cloned();

        if let Some(node) = selected_node {
            // Save current selection to the current level before pushing new one
            if let Some(current) = self.stack.last_mut() {
                current.last_selected = self.selected;
            }

            let children = self.explorer.children(&node);
            self.items_len = children.len();
            self.stack.push(Level {
                tag: Some(node.tag),
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

        let selected_node = self
            .stack
            .last()
            .and_then(|level| level.children.get(self.selected))
            .cloned();

        if let Some(node) = selected_node {
            let attributes = self.explorer.attributes(&node);

            // Get children count.
            let children = self.explorer.children(&node);
            let child_count = children.len();

            self.info_popup_data = Some((attributes, child_count));
            self.show_info_popup = true;
        }
    }
}
