//! Tree-building parser: converts a token stream into a DOM [`Document`].

use alloc::{string::String, vec, vec::Vec};

use super::{
    error::ParseError,
    node::{Attribute, Document, Node, NodeKind},
    tokenizer::{Token, Tokenizer},
};

// ── Raw-text elements ────────────────────────────────────────────────────────
//
// In HTML5, the content of <script>, <style>, <textarea>, and <title>
// is treated as raw text and not parsed as child elements.
const RAW_TEXT_ELEMENTS: &[&str] = &["script", "style"];
const RCDATA_ELEMENTS: &[&str] = &["textarea", "title"];

fn is_raw_or_rcdata(tag: &str) -> bool {
    RAW_TEXT_ELEMENTS.contains(&tag) || RCDATA_ELEMENTS.contains(&tag)
}

// ── ImpliedEnd: tags that auto-close certain open elements ───────────────────
//
// Simplified subset of HTML5 "adoption agency" rules.
//
// When we see `</p>` and there is no `<p>` on the stack, we manufacture a
// `<p>` node so that `<p>text</p>` inside a `<div>` without an explicit
// opening still works.  Similarly, `<li>` auto-closes the previous `<li>`.

/// Tags whose end-tag may be omitted (simplified list).
const OPTIONAL_CLOSE: &[&str] = &[
    "li", "dt", "dd", "p", "rb", "rp", "rt", "rtc", "optgroup", "option", "colgroup", "caption",
    "thead", "tbody", "tfoot", "tr", "td", "th",
];

fn auto_closes(open: &str, new_open: &str) -> bool {
    // A new <li> auto-closes the previous <li>
    // A new <dt>/<dd> auto-closes the previous <dt>/<dd>
    match (open, new_open) {
        ("li", "li") => true,
        ("dt" | "dd", "dt" | "dd") => true,
        ("option", "option" | "optgroup") => true,
        ("optgroup", "optgroup") => true,
        ("tr", "tr") => true,
        ("td" | "th", "td" | "th" | "tr") => true,
        ("p", t) if is_block(t) => true,
        _ => false,
    }
}

fn is_block(tag: &str) -> bool {
    matches!(
        tag,
        "address"
            | "article"
            | "aside"
            | "blockquote"
            | "details"
            | "dialog"
            | "dd"
            | "div"
            | "dl"
            | "dt"
            | "fieldset"
            | "figcaption"
            | "figure"
            | "footer"
            | "form"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "header"
            | "hgroup"
            | "hr"
            | "li"
            | "main"
            | "nav"
            | "ol"
            | "pre"
            | "section"
            | "summary"
            | "table"
            | "ul"
            | "p"
    )
}

// ── Stack frame ──────────────────────────────────────────────────────────────

struct Frame {
    tag: String,
    attrs: Vec<Attribute>,
    self_closing: bool,
    children: Vec<Node>,
}

impl Frame {
    fn new(tag: impl Into<String>, attrs: Vec<Attribute>, self_closing: bool) -> Self {
        Self {
            tag: tag.into(),
            attrs,
            self_closing,
            children: Vec::new(),
        }
    }

    fn into_node(self) -> Node {
        Node::element(self.tag, self.attrs, self.self_closing, self.children)
    }
}

// ── Parser ───────────────────────────────────────────────────────────────────

/// Tree-building HTML parser.
///
/// Internally maintains an open-element stack and appends nodes to the
/// innermost open element.  Produces a [`Document`] via [`Parser::parse`].
pub struct Parser<'a> {
    tokenizer: Tokenizer<'a>,
}

impl<'a> Parser<'a> {
    /// Create a new parser for `input`.
    pub fn new(input: &'a str) -> Self {
        Self {
            tokenizer: Tokenizer::new(input),
        }
    }

    /// Run the parser and return the completed [`Document`].
    pub fn parse(mut self) -> Result<Document, ParseError> {
        // The "open element stack": each entry is a tag + accumulated children.
        // Index 0 is the virtual document root.
        let mut stack: Vec<Frame> = vec![Frame::new("$root", Vec::new(), false)];

        while let Some(token) = self.tokenizer.next_token() {
            match token {
                // ── Doctype ───────────────────────────────────────────────
                Token::Doctype {
                    name,
                    public_id,
                    system_id,
                } => {
                    let node = Node {
                        kind: NodeKind::Doctype {
                            name,
                            public_id,
                            system_id,
                        },
                    };
                    Self::append_to_top(&mut stack, node);
                }

                // ── Comment ───────────────────────────────────────────────
                Token::Comment(c) => {
                    Self::append_to_top(&mut stack, Node::comment(c));
                }

                // ── CDATA ─────────────────────────────────────────────────
                Token::CData(c) => {
                    Self::append_to_top(&mut stack, Node::cdata(c));
                }

                // ── Processing instruction ────────────────────────────────
                Token::ProcessingInstruction { target, content } => {
                    let node = Node {
                        kind: NodeKind::ProcessingInstruction { target, content },
                    };
                    Self::append_to_top(&mut stack, node);
                }

                // ── Text ──────────────────────────────────────────────────
                Token::Text(t) => {
                    // Merge adjacent text nodes
                    let top = stack.last_mut().unwrap();
                    if let Some(Node {
                        kind: NodeKind::Text(existing),
                    }) = top.children.last_mut()
                    {
                        existing.push_str(&t);
                    } else {
                        top.children.push(Node::text(t));
                    }
                }

                // ── Start tag ─────────────────────────────────────────────
                Token::StartTag {
                    name,
                    attrs,
                    self_closing,
                } => {
                    // Handle optional-end auto-close
                    if let Some(top) = stack.last() {
                        if OPTIONAL_CLOSE.contains(&top.tag.as_str())
                            && auto_closes(&top.tag, &name)
                        {
                            let frame = stack.pop().unwrap();
                            Self::append_to_top(&mut stack, frame.into_node());
                        }
                    }

                    if self_closing {
                        // Self-closing or void: push and immediately pop
                        let node = Node::element(name, attrs, true, Vec::new());
                        Self::append_to_top(&mut stack, node);
                    } else if is_raw_or_rcdata(&name) {
                        // Raw-text: read until the matching end tag ourselves
                        let raw = self.read_raw_text(&name);
                        let mut children = Vec::new();
                        if !raw.is_empty() {
                            children.push(Node::text(raw));
                        }
                        let node = Node::element(name, attrs, false, children);
                        Self::append_to_top(&mut stack, node);
                    } else {
                        stack.push(Frame::new(name, attrs, false));
                    }
                }

                // ── End tag ───────────────────────────────────────────────
                Token::EndTag { name } => {
                    // Walk the stack upward to find a matching open tag.
                    let match_idx = stack
                        .iter()
                        .enumerate()
                        .rev()
                        .find(|(_, f)| f.tag == name)
                        .map(|(i, _)| i);

                    if let Some(idx) = match_idx {
                        // Pop everything above the matching frame (auto-close),
                        // then pop the matching frame itself.
                        while stack.len() > idx + 1 {
                            let frame = stack.pop().unwrap();
                            Self::append_to_top(&mut stack, frame.into_node());
                        }
                        let frame = stack.pop().unwrap();
                        Self::append_to_top(&mut stack, frame.into_node());
                    }
                    // If no match found, the end tag is spurious — ignore it.
                }
            }
        }

        // Drain any remaining open elements (implicit close at EOF)
        while stack.len() > 1 {
            let frame = stack.pop().unwrap();
            Self::append_to_top(&mut stack, frame.into_node());
        }

        let root = stack.pop().unwrap();
        Ok(Document {
            children: root.children,
        })
    }

    // ── helpers ──────────────────────────────────────────────────────────────

    fn append_to_top(stack: &mut Vec<Frame>, node: Node) {
        if let Some(top) = stack.last_mut() {
            top.children.push(node);
        }
    }

    /// Collect raw source text until `</name>` (case-insensitive).
    fn read_raw_text(&mut self, tag: &str) -> String {
        let mut raw = String::new();
        // We need to peek into the tokenizer at the source level.
        // Trick: collect tokens but only accept Text; stop on matching EndTag.
        loop {
            match self.tokenizer.next_token() {
                None => break,
                Some(Token::EndTag { name }) if name == tag => break,
                Some(Token::Text(t)) => raw.push_str(&t),
                Some(Token::Comment(c)) => {
                    // Keep comments verbatim inside raw-text elements
                    raw.push_str("<!--");
                    raw.push_str(&c);
                    raw.push_str("-->");
                }
                Some(_) => {} // discard other tokens inside raw-text
            }
        }
        raw
    }
}
