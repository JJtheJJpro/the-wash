//! A fully-featured CSS3 parser for `no_std` environments.
//! Uses `extern crate alloc` for heap allocation (Vec, String, Box).
//!
//! ## Features
//! - `no_std` compatible via the `alloc` crate
//! - CSS Syntax Level 3 compliant tokenizer
//! - Full stylesheet, qualified rule, and at-rule parsing
//! - Complete selector parsing (type, class, id, attribute, pseudo, combinators)
//! - Component value / block / function parsing
//! - Declaration parsing with `!important`
//! - All major at-rules: `@media`, `@keyframes`, `@font-face`, `@import`,
//!   `@supports`, `@charset`, `@layer`, `@page`, `@namespace`, `@counter-style`
//! - Escape sequences (`\hex`, `\literal`) in identifiers and strings
//! - Error recovery: malformed input produces partial ASTs, never panics
//!
//! ## Quick start
//! ```rust
//! use crate::the_wash::css::parse;
//!
//! let css = r#"
//!   body { color: red; font-size: 16px; }
//!   @media (max-width: 768px) { .hero { display: none; } }
//! "#;
//! let sheet = parse(css).unwrap();
//! ```

extern crate alloc;

pub mod ast;
pub mod error;
pub mod parser;
pub mod selector;
pub mod tokenizer;

pub use ast::{
    AtRule, AtRuleBlock, Block, BlockKind, ComponentValue, Declaration, FunctionBlock,
    KeyframeBlock, KeyframeSelector, QualifiedRule, Rule, Stylesheet,
};
pub use error::ParseError;
pub use parser::Parser;
pub use selector::{
    AttributeMatcher, AttributeOperator, AttributeSelector, CaseSensitivity, Combinator,
    ComplexSelector, CompoundSelector, PseudoSelector, SelectorList, SimpleSelector,
};
pub use tokenizer::{Token, Tokenizer};

/// Parse a CSS string into a [`Stylesheet`] AST.
///
/// This is the primary high-level entry point.
/// The parser is lenient: malformed input produces best-effort output.
///
/// # Errors
/// Currently always returns `Ok` (errors are recovered from internally).
pub fn parse(input: &str) -> Result<Stylesheet, ParseError> {
    Parser::new(input).parse()
}
