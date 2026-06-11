//! Parse error types.

use alloc::string::String;
use core::fmt;

/// Errors that may arise during CSS parsing.
///
/// The parser is highly lenient and rarely emits errors; most malformed CSS
/// results in partial AST output rather than a hard error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// An unexpected token was found.
    UnexpectedToken {
        /// Human-readable description of what was found.
        found: String,
        /// Byte offset in the source where the token starts.
        offset: usize,
    },
    /// Unexpected end of input.
    UnexpectedEof,
    /// A numeric escape `\HHHHHH` was outside the Unicode scalar value range.
    InvalidEscape(u32),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::UnexpectedToken { found, offset } => {
                write!(f, "unexpected token {:?} at offset {}", found, offset)
            }
            ParseError::UnexpectedEof => write!(f, "unexpected end of input"),
            ParseError::InvalidEscape(n) => {
                write!(f, "invalid escape \\{:X}", n)
            }
        }
    }
}
