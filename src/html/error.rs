//! Parse error types.

use alloc::string::String;
use core::fmt;

/// Errors that can occur during HTML parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// An unexpected character was encountered at a given byte offset.
    UnexpectedChar { found: char, offset: usize },
    /// An unexpected end-of-input was reached.
    UnexpectedEof,
    /// A numeric character reference (e.g. `&#999999;`) was out of range.
    InvalidCharRef(u32),
    /// A named entity reference was unrecognised.
    UnknownEntity(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::UnexpectedChar { found, offset } => {
                write!(f, "unexpected character {:?} at offset {}", found, offset)
            }
            ParseError::UnexpectedEof => write!(f, "unexpected end of input"),
            ParseError::InvalidCharRef(n) => write!(f, "invalid character reference U+{:04X}", n),
            ParseError::UnknownEntity(name) => write!(f, "unknown entity &{};", name),
        }
    }
}
