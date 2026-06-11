pub mod entities;
pub mod error;
pub mod node;
pub mod parser;
pub mod tokenizer;

use crate::html::{error::ParseError, node::Document, parser::Parser};

pub fn parse(input: &str) -> Result<Document, ParseError> {
    Parser::new(input).parse()
}
