//! Error types for lexing and parsing.

use super::span::Span;
use super::token::TokenKind;

use alloc::string::String;

/// A lexical error produced by the [`Lexer`](crate::lexer::Lexer).
#[derive(Debug, Clone, PartialEq)]
pub enum LexError {
    /// A string or template literal was opened but never closed.
    UnterminatedString(Span),
    /// A block comment (`/* … */`) was opened but never closed.
    UnterminatedComment(Span),
    /// A regular-expression literal was opened but never closed.
    UnterminatedRegex(Span),
    /// An unrecognised escape sequence inside a string or regex.
    InvalidEscape { span: Span, ch: char },
    /// A numeric literal has an invalid form (e.g. `0x` with no hex digits).
    InvalidNumericLiteral(Span),
    /// An unexpected character that the lexer doesn't recognise at all.
    UnexpectedChar { span: Span, ch: char },
    /// A Unicode escape in an identifier is not valid.
    InvalidUnicodeEscape(Span),
}

impl core::fmt::Display for LexError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnterminatedString(s)   => write!(f, "unterminated string literal at {s}"),
            Self::UnterminatedComment(s)  => write!(f, "unterminated block comment at {s}"),
            Self::UnterminatedRegex(s)    => write!(f, "unterminated regular expression at {s}"),
            Self::InvalidEscape { span, ch } =>
                write!(f, "invalid escape '\\{ch}' at {span}"),
            Self::InvalidNumericLiteral(s) =>
                write!(f, "invalid numeric literal at {s}"),
            Self::UnexpectedChar { span, ch } =>
                write!(f, "unexpected character '{ch}' at {span}"),
            Self::InvalidUnicodeEscape(s) =>
                write!(f, "invalid Unicode escape in identifier at {s}"),
        }
    }
}

// ── Parse errors ─────────────────────────────────────────────────────────────

/// A syntax error produced by the [`Parser`](crate::parser::Parser).
#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    /// Underlying lexer error.
    Lex(LexError),

    /// The parser expected a specific token but found something else.
    Expected {
        expected: TokenKind,
        found:    TokenKind,
        span:     Span,
    },

    /// Unexpected token with a human-readable context hint.
    Unexpected {
        found: TokenKind,
        span:  Span,
        #[cfg(feature = "alloc")]
        hint:  String,
        #[cfg(not(feature = "alloc"))]
        hint:  &'static str,
    },

    /// The input ended before the parse was complete.
    UnexpectedEof(Span),

    /// The parser reached a construct that is not valid in the current
    /// context (e.g. `return` outside a function).
    InvalidContext {
        #[cfg(feature = "alloc")]
        msg: String,
        #[cfg(not(feature = "alloc"))]
        msg: &'static str,
        span: Span,
    },

    /// `??` mixed with `||` or `&&` without explicit parentheses.
    NullishMixedWithLogical(Span),

    /// `**` with an unparenthesised unary expression on the left.
    ExponentWithUnary(Span),

    /// A `continue` label targets a non-loop statement.
    InvalidContinueTarget(Span),

    /// A `break` label was used without a matching label in scope.
    UndefinedLabel(Span),

    /// `super()` call outside a constructor, or `super.x` outside a method.
    IllegalSuper(Span),

    /// Destructuring assignment target is not a valid l-value pattern.
    InvalidDestructuring(Span),

    /// Duplicate parameter name in strict mode or in an arrow/function with
    /// a destructuring parameter.
    DuplicateParameter(Span),

    /// `import`/`export` used outside a module.
    ImportExportInScript(Span),

    /// `await` used outside an async function.
    AwaitOutsideAsync(Span),

    /// `yield` used outside a generator function.
    YieldOutsideGenerator(Span),
}

impl From<LexError> for ParseError {
    fn from(e: LexError) -> Self {
        Self::Lex(e)
    }
}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Lex(e)                        => write!(f, "lex error: {e}"),
            Self::Expected { expected, found, span } =>
                write!(f, "expected {expected:?}, found {found:?} at {span}"),
            Self::Unexpected { found, span, hint } =>
                write!(f, "unexpected {found:?} at {span}: {hint}"),
            Self::UnexpectedEof(s)              => write!(f, "unexpected end of file at {s}"),
            Self::InvalidContext { msg, span }  => write!(f, "{msg} at {span}"),
            Self::NullishMixedWithLogical(s)    =>
                write!(f, "?? cannot be mixed with || or && without parentheses at {s}"),
            Self::ExponentWithUnary(s)          =>
                write!(f, "unparenthesised unary expression before ** at {s}"),
            Self::InvalidContinueTarget(s)      =>
                write!(f, "continue target is not a loop at {s}"),
            Self::UndefinedLabel(s)             =>
                write!(f, "undefined label at {s}"),
            Self::IllegalSuper(s)               =>
                write!(f, "illegal 'super' at {s}"),
            Self::InvalidDestructuring(s)       =>
                write!(f, "invalid destructuring target at {s}"),
            Self::DuplicateParameter(s)         =>
                write!(f, "duplicate parameter name at {s}"),
            Self::ImportExportInScript(s)       =>
                write!(f, "import/export declaration in non-module script at {s}"),
            Self::AwaitOutsideAsync(s)          =>
                write!(f, "await used outside of an async function at {s}"),
            Self::YieldOutsideGenerator(s)      =>
                write!(f, "yield used outside of a generator function at {s}"),
        }
    }
}

/// Convenience alias.
pub type ParseResult<T> = Result<T, ParseError>;