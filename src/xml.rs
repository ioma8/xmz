use crate::parser::{Break, Continue, Token, extract_attributes, stream_xml};

/// A node in the XML tree.
/// Represents an element with its tag name, text content, and attributes.
/// Uses references ('a) to the original XML string to avoid allocations.
#[derive(Debug, Clone)]
pub struct Node<'a> {
    pub tag: &'a str,
    pub text: Option<&'a str>,
    pub offset: usize,
    pub attributes_raw: &'a str,
}

/// Cache entry: (parent_offset, children_nodes)
type CacheEntry<'a> = (usize, Vec<Node<'a>>);

/// Handles navigation and data access for the XML document.
/// Wraps the raw XML string and provides caching for children lookups.
pub struct XmlExplorer<'a> {
    xml: &'a str,
    cache: Vec<CacheEntry<'a>>,
}

impl<'a> XmlExplorer<'a> {
    pub fn new(xml: &'a str) -> Self {
        Self {
            xml,
            cache: Vec::new(),
        }
    }

    /// Returns the root node of the document.
    pub fn root(&self) -> Option<Node<'a>> {
        let mut root = None;
        stream_xml(self.xml, |token| {
            if let Token::StartTag(name, attrs) = token {
                // Subtract 1 to include the '<' in the offset logic if needed for consistency,
                // matching previous logic: bytes_offset(xml, name).saturating_sub(1)
                let offset = bytes_offset(self.xml, name).saturating_sub(1);
                root = Some(Node {
                    tag: name,
                    text: None,
                    offset,
                    attributes_raw: attrs,
                });
                return Break(());
            }
            Continue(())
        });
        root
    }

    /// Returns children of the given parent node.
    /// Uses internal cache to avoid re-parsing.
    pub fn children(&mut self, parent: &Node<'a>) -> Vec<Node<'a>> {
        // Check cache first
        for (key_offset, cached_children) in self.cache.iter() {
            if *key_offset == parent.offset {
                return cached_children.clone();
            }
        }

        // Not in cache, parse
        let children = self.parse_children(parent.offset, Some(parent.tag));
        self.cache.push((parent.offset, children.clone()));
        children
    }

    /// Extracts parsed attributes (key-value pairs) for the node.
    pub fn attributes(&self, node: &Node<'a>) -> Vec<(&'a str, &'a str)> {
        extract_attributes(self.xml, node.offset)
    }

    /// Internal parsing logic to find direct children
    fn parse_children(&self, offset: usize, parent_tag: Option<&str>) -> Vec<Node<'a>> {
        let mut children = Vec::new();
        let mut depth = 0;

        // Slice from the offset. We expect this to start with '<'
        let slice = if offset < self.xml.len() {
            &self.xml[offset..]
        } else {
            ""
        };

        let mut inside = false;
        let mut parent_matched = false;

        // Current child being built
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
                            // If no parent_tag provided (e.g. root search context?), treat as inside
                            inside = true;
                        }
                    } else {
                        // Inside parent
                        if depth == 0 {
                            last_tag = Some(name);
                            // Subtract 1 to point to '<'
                            last_tag_offset = bytes_offset(self.xml, name).saturating_sub(1);
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
                                children.push(Node {
                                    tag,
                                    text: last_text.take(),
                                    offset: last_tag_offset,
                                    attributes_raw: last_attrs,
                                });
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
}

fn bytes_offset(base: &str, slice: &str) -> usize {
    let base_start = base.as_ptr() as usize;
    let slice_start = slice.as_ptr() as usize;
    if slice_start < base_start || slice_start > base_start + base.len() {
        0
    } else {
        slice_start - base_start
    }
}
