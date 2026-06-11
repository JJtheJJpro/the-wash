//! # nova-js-parser
//!
//! A full-featured, `no_std`-compatible ECMAScript 2022 parser written for the
//! **Nova browser engine**.
//!
//! ## Architecture
//!
//! ```text
//!  Source text (&str)
//!       │
//!       ▼
//!  ┌──────────┐        produces       ┌──────────────────┐
//!  │  Lexer   │──────────────────────▶│  Token stream    │
//!  └──────────┘                       └──────────────────┘
//!                                              │
//!                                              ▼
//!                                      ┌──────────────┐
//!                                      │   Parser     │
//!                                      │ (Pratt exprs │
//!                                      │  + rec-desc  │
//!                                      │    stmts)    │
//!                                      └──────────────┘
//!                                              │  produces
//!                                              ▼
//!                                      ┌──────────────┐
//!                                      │  AST Program │
//!                                      └──────────────┘
//! ```
//!
//! ## Usage
//!
//! ```rust
//! use nova_js_parser::{parse_script, parse_module};
//!
//! let program = parse_script("const x = 1 + 2;").unwrap();
//! ```
//!
//! ## `no_std` notes
//!
//! The crate is `#![no_std]` but requires the `alloc` feature (enabled by
//! default) to use `Vec`, `String`, and `Box`. Link against your custom
//! allocator before calling any parse function.

#![warn(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    missing_docs,
    missing_debug_implementations
)]
#![allow(
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::missing_errors_doc,
    clippy::wildcard_imports
)]

pub mod ast;
pub mod error;
pub mod lexer;
pub mod parser;
mod span;
mod token;

pub use ast::Program;
pub use error::{ParseError, ParseResult};
pub use parser::Parser;
pub use span::Span;

/// Parse JavaScript source as a **script** (classic, non-module mode).
///
/// # Errors
/// Returns a [`ParseError`] describing the first syntax error encountered.
pub fn parse_script(source: &str) -> ParseResult<Program> {
    Parser::new(source, ast::SourceType::Script).parse_program()
}

/// Parse JavaScript source as an **ES module** (`import`/`export` allowed).
///
/// # Errors
/// Returns a [`ParseError`] describing the first syntax error encountered.
pub fn parse_module(source: &str) -> ParseResult<Program> {
    Parser::new(source, ast::SourceType::Module).parse_program()
}

/// Parse a single JavaScript **expression** (useful for template evaluation,
/// `eval`, and REPL environments).
///
/// # Errors
/// Returns a [`ParseError`] if the input is not a valid expression.
pub fn parse_expression(source: &str) -> ParseResult<ast::Expr> {
    Parser::new(source, ast::SourceType::Script).parse_expr_only()
}