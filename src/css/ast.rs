//! CSS Abstract Syntax Tree types.

use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use core::fmt;

use super::selector::SelectorList;
use super::tokenizer::Token;

// ── Component values ──────────────────────────────────────────────────────────

/// The CSS "component value" — the fundamental unit of parsed value lists.
#[derive(Debug, Clone, PartialEq)]
pub enum ComponentValue {
    /// A preserved token (not a block or function opener).
    Token(Token),
    /// A `{…}`, `[…]`, or `(…)` simple block.
    Block(Block),
    /// A function call `name(…)`.
    Function(FunctionBlock),
}

impl ComponentValue {
    /// Returns `true` for a whitespace token.
    pub fn is_whitespace(&self) -> bool {
        matches!(self, ComponentValue::Token(Token::Whitespace))
    }

    /// Serialise back to a CSS string.
    pub fn to_css_string(&self) -> String {
        let mut s = String::new();
        self.write_css(&mut s);
        s
    }

    pub(crate) fn write_css(&self, out: &mut String) {
        match self {
            ComponentValue::Token(t) => token_to_css(t, out),
            ComponentValue::Block(b) => {
                let (o, c) = b.kind.delimiters();
                out.push(o);
                for cv in &b.value {
                    cv.write_css(out);
                }
                out.push(c);
            }
            ComponentValue::Function(f) => {
                out.push_str(&f.name);
                out.push('(');
                for cv in &f.value {
                    cv.write_css(out);
                }
                out.push(')');
            }
        }
    }
}

fn token_to_css(t: &Token, out: &mut String) {
    match t {
        Token::Ident(s) => out.push_str(s),
        Token::Function(s) => {
            out.push_str(s);
            out.push('(');
        }
        Token::AtKeyword(s) => {
            out.push('@');
            out.push_str(s);
        }
        Token::Hash { value, .. } => {
            out.push('#');
            out.push_str(value);
        }
        Token::String(s) => {
            out.push('"');
            out.push_str(s);
            out.push('"');
        }
        Token::Url(s) => {
            out.push_str("url(");
            out.push_str(s);
            out.push(')');
        }
        Token::Delim(c) => out.push(*c),
        Token::Number { repr, .. } => out.push_str(repr),
        Token::Percentage { repr, .. } => {
            out.push_str(repr);
            out.push('%');
        }
        Token::Dimension { repr, unit, .. } => {
            out.push_str(repr);
            out.push_str(unit);
        }
        Token::Whitespace => out.push(' '),
        Token::Colon => out.push(':'),
        Token::Semicolon => out.push(';'),
        Token::Comma => out.push(','),
        Token::LeftBracket => out.push('['),
        Token::RightBracket => out.push(']'),
        Token::LeftParen => out.push('('),
        Token::RightParen => out.push(')'),
        Token::LeftBrace => out.push('{'),
        Token::RightBrace => out.push('}'),
        _ => {}
    }
}

/// Which bracket type encloses a simple block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    /// `{…}`
    Curly,
    /// `[…]`
    Square,
    /// `(…)`
    Paren,
}

impl BlockKind {
    /// Returns `(opening, closing)` delimiter chars.
    pub fn delimiters(self) -> (char, char) {
        match self {
            BlockKind::Curly => ('{', '}'),
            BlockKind::Square => ('[', ']'),
            BlockKind::Paren => ('(', ')'),
        }
    }
}

/// A simple block: `{…}`, `[…]`, or `(…)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    /// The bracket type.
    pub kind: BlockKind,
    /// Contents as component values.
    pub value: Vec<ComponentValue>,
}

/// A function call: `name(arg1, …)`.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionBlock {
    /// Function name (lowercased).
    pub name: String,
    /// Arguments as component values.
    pub value: Vec<ComponentValue>,
}

// ── Declarations ──────────────────────────────────────────────────────────────

/// A CSS declaration: `property: value [!important]`.
#[derive(Debug, Clone, PartialEq)]
pub struct Declaration {
    /// Property name (lowercased, or `--custom` verbatim).
    pub property: String,
    /// Parsed value list.
    pub value: Vec<ComponentValue>,
    /// `true` when `!important` was present.
    pub important: bool,
}

impl Declaration {
    /// Serialise the value list back to a CSS string.
    pub fn value_string(&self) -> String {
        let raw: String = self.value.iter().map(|cv| cv.to_css_string()).collect();
        // trim ASCII whitespace without std
        let trimmed = raw.trim_matches(|c: char| c.is_ascii_whitespace());
        trimmed.to_string()
    }
}

impl fmt::Display for Declaration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.property, self.value_string())?;
        if self.important {
            write!(f, " !important")?;
        }
        Ok(())
    }
}

// ── Rules ─────────────────────────────────────────────────────────────────────

/// A top-level CSS rule.
#[derive(Debug, Clone, PartialEq)]
pub enum Rule {
    /// A standard qualified rule: `selector { declarations }`.
    Qualified(QualifiedRule),
    /// An at-rule: `@keyword prelude { block }` or `@keyword prelude;`.
    At(AtRule),
}

/// A qualified rule: `selector-list { declarations }`.
#[derive(Debug, Clone, PartialEq)]
pub struct QualifiedRule {
    /// Parsed selector list (if selector parsing succeeded).
    pub selectors: Option<SelectorList>,
    /// Raw prelude component values.
    pub prelude: Vec<ComponentValue>,
    /// Declarations inside the block.
    pub declarations: Vec<Declaration>,
}

/// An at-rule: `@name prelude;` or `@name prelude { block }`.
#[derive(Debug, Clone, PartialEq)]
pub struct AtRule {
    /// At-rule name without `@`, lowercased.
    pub name: String,
    /// Prelude component values (between keyword and `{` or `;`).
    pub prelude: Vec<ComponentValue>,
    /// Block content, if the rule has a `{…}` block.
    pub block: Option<AtRuleBlock>,
}

/// The block content of an at-rule.
#[derive(Debug, Clone, PartialEq)]
pub enum AtRuleBlock {
    /// `@font-face { … }`, `@page { … }` — declarations.
    Declarations(Vec<Declaration>),
    /// `@media { … }`, `@supports { … }`, `@layer { … }` — nested rules.
    Rules(Vec<Rule>),
    /// `@keyframes { … }` — keyframe blocks.
    Keyframes(Vec<KeyframeBlock>),
}

/// A single keyframe block: `from { … }` or `50% { … }`.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyframeBlock {
    /// The keyframe selectors (may be multiple, comma-separated).
    pub selectors: Vec<KeyframeSelector>,
    /// Declarations inside this keyframe.
    pub declarations: Vec<Declaration>,
}

/// A single keyframe selector.
#[derive(Debug, Clone, PartialEq)]
pub enum KeyframeSelector {
    /// `from` keyword (= 0 %)
    From,
    /// `to` keyword (= 100 %)
    To,
    /// An explicit percentage: `42%`
    Percentage(f64),
}

impl KeyframeSelector {
    /// Normalised percentage in the range `0.0..=100.0`.
    pub fn as_percentage(&self) -> f64 {
        match self {
            KeyframeSelector::From => 0.0,
            KeyframeSelector::To => 100.0,
            KeyframeSelector::Percentage(p) => *p,
        }
    }
}

// ── Stylesheet ────────────────────────────────────────────────────────────────

/// A complete parsed CSS stylesheet.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Stylesheet {
    /// Top-level rules.
    pub rules: Vec<Rule>,
}

impl Stylesheet {
    /// Iterate over all qualified rules.
    pub fn qualified_rules(&self) -> impl Iterator<Item = &QualifiedRule> {
        self.rules.iter().filter_map(|r| match r {
            Rule::Qualified(q) => Some(q),
            _ => None,
        })
    }

    /// Iterate over all at-rules.
    pub fn at_rules(&self) -> impl Iterator<Item = &AtRule> {
        self.rules.iter().filter_map(|r| match r {
            Rule::At(a) => Some(a),
            _ => None,
        })
    }

    /// Iterate over all at-rules with the given name.
    pub fn at_rules_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a AtRule> {
        self.at_rules().filter(move |r| r.name == name)
    }

    /// Iterate over every `(rule, declaration)` pair across all qualified rules.
    pub fn all_declarations(&self) -> impl Iterator<Item = (&QualifiedRule, &Declaration)> {
        self.qualified_rules()
            .flat_map(|q| q.declarations.iter().map(move |d| (q, d)))
    }
}

// ── Display ───────────────────────────────────────────────────────────────────

impl fmt::Display for Stylesheet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for rule in &self.rules {
            write!(f, "{}\n", rule)?;
        }
        Ok(())
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Rule::Qualified(q) => write!(f, "{}", q),
            Rule::At(a) => write!(f, "{}", a),
        }
    }
}

impl fmt::Display for QualifiedRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(sel) = &self.selectors {
            write!(f, "{}", sel)?;
        } else {
            for cv in &self.prelude {
                write!(f, "{}", cv.to_css_string())?;
            }
        }
        write!(f, " {{")?;
        for d in &self.declarations {
            write!(f, " {};", d)?;
        }
        write!(f, " }}")
    }
}

impl fmt::Display for AtRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "@{}", self.name)?;
        if !self.prelude.is_empty() {
            write!(f, " ")?;
            for cv in &self.prelude {
                write!(f, "{}", cv.to_css_string())?;
            }
        }
        match &self.block {
            None => write!(f, ";"),
            Some(AtRuleBlock::Declarations(ds)) => {
                write!(f, " {{")?;
                for d in ds {
                    write!(f, " {};", d)?;
                }
                write!(f, " }}")
            }
            Some(AtRuleBlock::Rules(rs)) => {
                write!(f, " {{")?;
                for r in rs {
                    write!(f, " {}", r)?;
                }
                write!(f, " }}")
            }
            Some(AtRuleBlock::Keyframes(kfs)) => {
                write!(f, " {{")?;
                for kf in kfs {
                    for (i, s) in kf.selectors.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        match s {
                            KeyframeSelector::From => write!(f, "from")?,
                            KeyframeSelector::To => write!(f, "to")?,
                            KeyframeSelector::Percentage(p) => write!(f, "{}%", p)?,
                        }
                    }
                    write!(f, " {{")?;
                    for d in &kf.declarations {
                        write!(f, " {};", d)?;
                    }
                    write!(f, " }}")?;
                }
                write!(f, " }}")
            }
        }
    }
}
