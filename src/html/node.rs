//! DOM node types.

use alloc::{string::String, vec::Vec};
use core::fmt;

/// A single HTML attribute: `name="value"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    /// Attribute name (lowercased).
    pub name: String,
    /// Attribute value (decoded, empty string for boolean attributes).
    pub value: String,
}

impl Attribute {
    pub(crate) fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

/// The concrete variant of a [`Node`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind {
    /// `<!DOCTYPE ...>`
    Doctype {
        /// The DOCTYPE name (usually `html`).
        name: String,
        /// Optional public identifier.
        public_id: Option<String>,
        /// Optional system identifier.
        system_id: Option<String>,
    },
    /// An element: `<tag attr="val">…</tag>`
    Element {
        /// Tag name (lowercased).
        tag: String,
        /// List of attributes.
        attrs: Vec<Attribute>,
        /// Whether this was written as a self-closing tag (`<br />`).
        self_closing: bool,
        /// Child nodes.
        children: Vec<Node>,
    },
    /// A text node (entity-decoded).
    Text(String),
    /// `<!-- comment -->`
    Comment(String),
    /// `<![CDATA[ ... ]]>`
    CData(String),
    /// Processing instruction: `<?target content?>`
    ProcessingInstruction {
        /// PI target name.
        target: String,
        /// PI content.
        content: String,
    },
}

/// A node in the HTML document tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    /// The concrete kind/data for this node.
    pub kind: NodeKind,
}

impl Node {
    /// Create a new element node.
    pub fn element(
        tag: impl Into<String>,
        attrs: Vec<Attribute>,
        self_closing: bool,
        children: Vec<Node>,
    ) -> Self {
        Self {
            kind: NodeKind::Element {
                tag: tag.into(),
                attrs,
                self_closing,
                children,
            },
        }
    }

    /// Create a text node.
    pub fn text(t: impl Into<String>) -> Self {
        Self {
            kind: NodeKind::Text(t.into()),
        }
    }

    /// Create a comment node.
    pub fn comment(c: impl Into<String>) -> Self {
        Self {
            kind: NodeKind::Comment(c.into()),
        }
    }

    /// Create a CDATA node.
    pub fn cdata(c: impl Into<String>) -> Self {
        Self {
            kind: NodeKind::CData(c.into()),
        }
    }

    /// Returns `true` if this node is an element with the given tag name.
    pub fn is_element(&self, tag: &str) -> bool {
        matches!(&self.kind, NodeKind::Element { tag: t, .. } if t == tag)
    }

    /// Returns the tag name if this is an element node.
    pub fn tag(&self) -> Option<&str> {
        match &self.kind {
            NodeKind::Element { tag, .. } => Some(tag),
            _ => None,
        }
    }

    /// Returns the children slice if this is an element node.
    pub fn children(&self) -> &[Node] {
        match &self.kind {
            NodeKind::Element { children, .. } => children,
            _ => &[],
        }
    }

    /// Returns the mutable children vec if this is an element node.

    /// Returns the attribute list if this is an element node.
    pub fn attrs(&self) -> &[Attribute] {
        match &self.kind {
            NodeKind::Element { attrs, .. } => attrs,
            _ => &[],
        }
    }

    /// Look up an attribute by name.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs()
            .iter()
            .find(|a| a.name == name)
            .map(|a| a.value.as_str())
    }

    /// Recursively find all descendant elements with the given tag name.
    pub fn find_all<'a>(&'a self, tag: &'a str) -> FindAll<'a> {
        FindAll {
            stack: alloc::vec![self],
            tag,
        }
    }

    /// Return all text content within this node, recursively.
    pub fn text_content(&self) -> String {
        let mut out = String::new();
        self.collect_text(&mut out);
        out
    }

    fn collect_text(&self, out: &mut String) {
        match &self.kind {
            NodeKind::Text(t) => out.push_str(t),
            NodeKind::Element { children, .. } => {
                for child in children {
                    child.collect_text(out);
                }
            }
            _ => {}
        }
    }
}

/// Iterator returned by [`Node::find_all`].
pub struct FindAll<'a> {
    stack: Vec<&'a Node>,
    tag: &'a str,
}

impl<'a> Iterator for FindAll<'a> {
    type Item = &'a Node;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(node) = self.stack.pop() {
            if let NodeKind::Element { children, .. } = &node.kind {
                // push children in reverse so we visit in document order
                for child in children.iter().rev() {
                    self.stack.push(child);
                }
            }
            if node.is_element(self.tag) {
                return Some(node);
            }
        }
        None
    }
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_indent(f, 0)
    }
}

impl Node {
    fn fmt_indent(&self, f: &mut fmt::Formatter<'_>, depth: usize) -> fmt::Result {
        let indent = core::iter::repeat(' ').take(depth * 2).collect::<String>();
        match &self.kind {
            NodeKind::Doctype { name, .. } => writeln!(f, "{}<!DOCTYPE {}>", indent, name),
            NodeKind::Text(t) => {
                let trimmed = t.trim();
                if !trimmed.is_empty() {
                    writeln!(f, "{}{}", indent, trimmed)
                } else {
                    Ok(())
                }
            }
            NodeKind::Comment(c) => writeln!(f, "{}<!-- {} -->", indent, c),
            NodeKind::CData(c) => writeln!(f, "{}<![CDATA[{}]]>", indent, c),
            NodeKind::ProcessingInstruction { target, content } => {
                writeln!(f, "{}<?{} {}?>", indent, target, content)
            }
            NodeKind::Element {
                tag,
                attrs,
                children,
                self_closing,
            } => {
                write!(f, "{}<{}", indent, tag)?;
                for attr in attrs {
                    if attr.value.is_empty() {
                        write!(f, " {}", attr.name)?;
                    } else {
                        write!(f, " {}=\"{}\"", attr.name, attr.value)?;
                    }
                }
                if *self_closing && children.is_empty() {
                    writeln!(f, " />")?;
                } else {
                    writeln!(f, ">")?;
                    for child in children {
                        child.fmt_indent(f, depth + 1)?;
                    }
                    writeln!(f, "{}</{}>", indent, tag)?;
                }
                Ok(())
            }
        }
    }
}

/// The top-level parsed HTML document.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Document {
    /// Top-level nodes (DOCTYPE, `<html>`, comments, etc.)
    pub children: Vec<Node>,
}

impl Document {
    /// Find the first `<html>` element, if present.
    pub fn root_element(&self) -> Option<&Node> {
        self.children.iter().find(|n| n.is_element("html"))
    }

    /// Recursively search the whole document for all elements with `tag`.
    pub fn find_all<'a>(&'a self, tag: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.children.iter().flat_map(move |n| n.find_all(tag))
    }

    /// Collect **all** text content in the document.
    pub fn text_content(&self) -> String {
        let mut s = String::new();
        for child in &self.children {
            child.collect_text(&mut s);
        }
        s
    }
}

impl fmt::Display for Document {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for child in &self.children {
            write!(f, "{}", child)?;
        }
        Ok(())
    }
}
