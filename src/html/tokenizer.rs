//! Low-level HTML tokenizer.
//!
//! Converts a raw HTML `&str` into a flat stream of [`Token`]s.
//! The tokenizer is deliberately lenient — it never returns a hard error
//! for malformed markup; instead it emits the best-effort token and moves on.

use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use super::entities::decode_entity;
use super::error::ParseError;
use super::node::Attribute;

// ── Void elements (HTML5 §13.1.2) ──────────────────────────────────────────

/// HTML5 void element names — elements that must not have a closing tag.
pub const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// Returns `true` if `tag` (lowercase) is a void element.
pub fn is_void(tag: &str) -> bool {
    VOID_ELEMENTS.contains(&tag)
}

// ── Token ───────────────────────────────────────────────────────────────────

/// A single lexical token produced by the [`Tokenizer`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    /// `<!DOCTYPE name [public "…"] [system "…"]>`
    Doctype {
        name: String,
        public_id: Option<String>,
        system_id: Option<String>,
    },
    /// `<tag attr="val" …>` — `self_closing` is true for `<tag />`
    StartTag {
        name: String,
        attrs: Vec<Attribute>,
        self_closing: bool,
    },
    /// `</tag>`
    EndTag { name: String },
    /// Raw (un-decoded) text between tags.
    Text(String),
    /// `<!-- … -->`
    Comment(String),
    /// `<![CDATA[ … ]]>`
    CData(String),
    /// `<?target content?>`
    ProcessingInstruction { target: String, content: String },
}

// ── Tokenizer ───────────────────────────────────────────────────────────────

/// Streaming HTML tokenizer.
///
/// Call [`Tokenizer::next_token`] repeatedly until it returns `None`.
pub struct Tokenizer<'a> {
    src: &'a str,
    pos: usize,
}

impl<'a> Tokenizer<'a> {
    /// Create a tokenizer over `src`.
    pub fn new(src: &'a str) -> Self {
        Self { src, pos: 0 }
    }

    /// Current byte offset into the source.
    pub fn position(&self) -> usize {
        self.pos
    }

    // ── low-level helpers ────────────────────────────────────────────────

    #[inline]
    fn remaining(&self) -> &'a str {
        &self.src[self.pos..]
    }

    #[inline]
    fn peek(&self) -> Option<char> {
        self.remaining().chars().next()
    }

    #[inline]
    fn advance(&mut self) -> Option<char> {
        let ch = self.remaining().chars().next()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    #[inline]
    fn eat_if(&mut self, ch: char) -> bool {
        if self.peek() == Some(ch) {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Consume chars while `pred` is true; returns the consumed slice.
    fn take_while<P: Fn(char) -> bool>(&mut self, pred: P) -> &'a str {
        let start = self.pos;
        while self.peek().map_or(false, |c| pred(c)) {
            self.advance();
        }
        &self.src[start..self.pos]
    }

    fn skip_whitespace(&mut self) {
        self.take_while(|c| c.is_ascii_whitespace());
    }

    /// Case-insensitive prefix match + consume.
    fn eat_ascii_prefix(&mut self, prefix: &str) -> bool {
        let rem = self.remaining();
        if rem.len() < prefix.len() {
            return false;
        }
        let slice = &rem[..prefix.len()];
        if slice.eq_ignore_ascii_case(prefix) {
            self.pos += prefix.len();
            true
        } else {
            false
        }
    }

    fn starts_with(&self, s: &str) -> bool {
        self.remaining().starts_with(s)
    }

    // ── public API ───────────────────────────────────────────────────────

    /// Produce the next [`Token`], or `None` at end-of-input.
    ///
    /// This method is lenient: parse failures result in text tokens rather
    /// than hard errors.
    pub fn next_token(&mut self) -> Option<Token> {
        if self.pos >= self.src.len() {
            return None;
        }

        if self.peek() == Some('<') {
            self.advance(); // consume '<'
            self.dispatch_tag()
        } else {
            Some(self.read_text())
        }
    }

    // ── text ─────────────────────────────────────────────────────────────

    fn read_text(&mut self) -> Token {
        let mut text = String::new();
        loop {
            match self.peek() {
                None | Some('<') => break,
                Some('&') => {
                    self.advance();
                    match self.read_entity() {
                        Ok(s) => text.push_str(&s),
                        Err(_) => text.push('&'),
                    }
                }
                Some(c) => {
                    text.push(c);
                    self.advance();
                }
            }
        }
        Token::Text(text)
    }

    // ── entity references ────────────────────────────────────────────────

    fn read_entity(&mut self) -> Result<String, ParseError> {
        if self.eat_if('#') {
            // numeric reference
            let (hex, radix) = if self.eat_if('x') || self.eat_if('X') {
                (true, 16u32)
            } else {
                (false, 10u32)
            };
            let digits = self.take_while(|c| {
                if hex {
                    c.is_ascii_hexdigit()
                } else {
                    c.is_ascii_digit()
                }
            });
            self.eat_if(';');
            let n = u32::from_str_radix(digits, radix).map_err(|_| ParseError::UnexpectedEof)?;
            let ch = char::from_u32(n).ok_or(ParseError::InvalidCharRef(n))?;
            Ok(ch.to_string())
        } else {
            // named reference
            let name = self.take_while(|c| c.is_alphanumeric());
            self.eat_if(';');
            decode_entity(name)
                .map(|s| s.to_string())
                .ok_or_else(|| ParseError::UnknownEntity(name.into()))
        }
    }

    // ── tag dispatch ─────────────────────────────────────────────────────

    fn dispatch_tag(&mut self) -> Option<Token> {
        // `<` has already been consumed.
        match self.peek() {
            Some('!') => {
                self.advance();
                self.read_bang()
            }
            Some('?') => {
                self.advance();
                Some(self.read_pi())
            }
            Some('/') => {
                self.advance();
                Some(self.read_end_tag())
            }
            Some(c) if c.is_alphanumeric() || c == '_' || c == ':' => Some(self.read_start_tag()),
            _ => {
                // Not a real tag — emit `<` as text and let the caller handle it
                Some(Token::Text("<".into()))
            }
        }
    }

    // ── `<!…` ────────────────────────────────────────────────────────────

    fn read_bang(&mut self) -> Option<Token> {
        if self.starts_with("--") {
            self.pos += 2;
            Some(self.read_comment())
        } else if self.eat_ascii_prefix("[CDATA[") {
            Some(self.read_cdata())
        } else if self.eat_ascii_prefix("DOCTYPE") || self.eat_ascii_prefix("doctype") {
            Some(self.read_doctype())
        } else {
            // Unknown `<!…` — consume to `>`
            let text = self.take_while(|c| c != '>');
            self.eat_if('>');
            Some(Token::Comment(text.into()))
        }
    }

    // ── comment: `<!-- … -->` ────────────────────────────────────────────

    fn read_comment(&mut self) -> Token {
        let mut body = String::new();
        loop {
            if self.starts_with("-->") {
                self.pos += 3;
                break;
            }
            match self.advance() {
                None => break,
                Some(c) => body.push(c),
            }
        }
        Token::Comment(body)
    }

    // ── CDATA: `<![CDATA[ … ]]>` ────────────────────────────────────────

    fn read_cdata(&mut self) -> Token {
        let mut body = String::new();
        loop {
            if self.starts_with("]]>") {
                self.pos += 3;
                break;
            }
            match self.advance() {
                None => break,
                Some(c) => body.push(c),
            }
        }
        Token::CData(body)
    }

    // ── DOCTYPE ──────────────────────────────────────────────────────────

    fn read_doctype(&mut self) -> Token {
        self.skip_whitespace();
        let name = self.take_while(|c| !c.is_ascii_whitespace() && c != '>' && c != '[');
        let name = name.to_ascii_lowercase();
        self.skip_whitespace();

        let mut public_id: Option<String> = None;
        let mut system_id: Option<String> = None;

        if self.eat_ascii_prefix("PUBLIC") {
            self.skip_whitespace();
            public_id = self.read_quoted_or_word();
            self.skip_whitespace();
            system_id = self.read_quoted_or_word();
        } else if self.eat_ascii_prefix("SYSTEM") {
            self.skip_whitespace();
            system_id = self.read_quoted_or_word();
        }

        // consume internal subset and closing `>`
        let mut depth = 0usize;
        loop {
            match self.advance() {
                None => break,
                Some('[') => depth += 1,
                Some(']') if depth > 0 => depth -= 1,
                Some('>') if depth == 0 => break,
                _ => {}
            }
        }

        Token::Doctype {
            name,
            public_id,
            system_id,
        }
    }

    fn read_quoted_or_word(&mut self) -> Option<String> {
        match self.peek() {
            Some('"') | Some('\'') => {
                let q = self.advance().unwrap();
                let s = self.take_while(|c| c != q).to_string();
                self.eat_if(q);
                Some(s)
            }
            Some(c) if c != '>' => Some(
                self.take_while(|c| !c.is_ascii_whitespace() && c != '>')
                    .to_string(),
            ),
            _ => None,
        }
    }

    // ── processing instruction: `<?target content?>` ─────────────────────

    fn read_pi(&mut self) -> Token {
        let target = self
            .take_while(|c| !c.is_ascii_whitespace() && c != '>' && c != '?')
            .to_string();
        self.skip_whitespace();
        let mut content = String::new();
        loop {
            if self.starts_with("?>") {
                self.pos += 2;
                break;
            }
            match self.advance() {
                None => break,
                Some(c) => content.push(c),
            }
        }
        Token::ProcessingInstruction { target, content }
    }

    // ── end tag: `</tag>` ────────────────────────────────────────────────

    fn read_end_tag(&mut self) -> Token {
        self.skip_whitespace();
        let name = self
            .take_while(|c| !c.is_ascii_whitespace() && c != '>')
            .to_ascii_lowercase();
        self.skip_whitespace();
        self.eat_if('>');
        Token::EndTag { name }
    }

    // ── start tag: `<tag attr="val" …>` ─────────────────────────────────

    fn read_start_tag(&mut self) -> Token {
        let name = self
            .take_while(|c| !c.is_ascii_whitespace() && c != '>' && c != '/')
            .to_ascii_lowercase();

        let mut attrs: Vec<Attribute> = Vec::new();
        let mut self_closing = false;

        loop {
            self.skip_whitespace();
            match self.peek() {
                None | Some('>') => {
                    self.eat_if('>');
                    break;
                }
                Some('/') => {
                    self.advance();
                    self.eat_if('>');
                    self_closing = true;
                    break;
                }
                _ => {
                    if let Some(attr) = self.read_attribute() {
                        // last writer wins for duplicate attrs
                        attrs.retain(|a: &Attribute| a.name != attr.name);
                        attrs.push(attr);
                    } else {
                        // safety valve: skip one char to avoid infinite loop
                        self.advance();
                    }
                }
            }
        }

        // Void elements are always self-closing regardless of markup
        if is_void(&name) {
            self_closing = true;
        }

        Token::StartTag {
            name,
            attrs,
            self_closing,
        }
    }

    // ── attribute ────────────────────────────────────────────────────────

    fn read_attribute(&mut self) -> Option<Attribute> {
        // attribute name
        let name =
            self.take_while(|c| !c.is_ascii_whitespace() && c != '=' && c != '>' && c != '/');
        if name.is_empty() {
            return None;
        }
        let name = name.to_ascii_lowercase();

        self.skip_whitespace();

        // optional `= value`
        let value = if self.eat_if('=') {
            self.skip_whitespace();
            self.read_attr_value()
        } else {
            String::new() // boolean attribute
        };

        Some(Attribute::new(name, value))
    }

    fn read_attr_value(&mut self) -> String {
        match self.peek() {
            Some('"') | Some('\'') => {
                let q = self.advance().unwrap();
                let mut val = String::new();
                loop {
                    match self.peek() {
                        None | Some('\0') => break,
                        Some(c) if c == q => {
                            self.advance();
                            break;
                        }
                        Some('&') => {
                            self.advance();
                            match self.read_entity() {
                                Ok(s) => val.push_str(&s),
                                Err(_) => val.push('&'),
                            }
                        }
                        Some(c) => {
                            val.push(c);
                            self.advance();
                        }
                    }
                }
                val
            }
            _ => {
                // unquoted attribute value
                let mut val = String::new();
                loop {
                    match self.peek() {
                        None | Some('>') | Some('/') => break,
                        Some(c) if c.is_ascii_whitespace() => break,
                        Some('&') => {
                            self.advance();
                            match self.read_entity() {
                                Ok(s) => val.push_str(&s),
                                Err(_) => val.push('&'),
                            }
                        }
                        Some(c) => {
                            val.push(c);
                            self.advance();
                        }
                    }
                }
                val
            }
        }
    }
}

// ── IntoIterator ─────────────────────────────────────────────────────────────

impl<'a> Iterator for Tokenizer<'a> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_token()
    }
}
