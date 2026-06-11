//! Token definitions for the JavaScript lexer.
//!
//! Every valid JavaScript lexeme maps to one variant of [`TokenKind`].
//! String content (identifiers, string literals, etc.) is heap-allocated via
//! the `alloc` crate so the lexer can be used in `no_std` environments.

use alloc::string::String;

use super::span::Span;

// ─────────────────────────────────────────────────────────────────────────────
// Token kind
// ─────────────────────────────────────────────────────────────────────────────

/// Every possible JavaScript lexeme, classified into a variant.
#[derive(Debug, Clone, PartialEq)]
#[allow(missing_docs)]
pub enum TokenKind {
    // ── Identifiers & literals ────────────────────────────────────────────────
    /// Any identifier that is not a reserved keyword, e.g. `foo`, `$bar`, `_x`.
    Ident(String),
    /// Numeric literal already parsed to `f64`, e.g. `42`, `3.14`, `0xff`, `0b1010`.
    Number(f64),
    /// BigInt literal (the digits, without the trailing `n`), e.g. `"9007199254740993"`.
    BigInt(String),
    /// Fully-decoded string literal contents, e.g. `"hello\nworld"` → `"hello\nworld"`.
    Str(String),
    /// Regular expression pattern + flags, e.g. `/ab+c/gi`.
    Regex { pattern: String, flags: String },

    // ── Template literal parts ────────────────────────────────────────────────
    /// A template with no substitutions: `` `hello` `` → `"hello"`.
    TemplateNoSub(String),
    /// The head of a template with substitutions: `` `hello ${`` → `"hello "`.
    TemplateHead(String),
    /// A middle section: `} world ${` → `" world "`.
    TemplateMiddle(String),
    /// The tail: `} end` `` ` `` → `" end"`.
    TemplateTail(String),

    // ── Value-producing keywords ───────────────────────────────────────────────
    True, False, Null,
    This, Super, New,

    // ── Control-flow keywords ──────────────────────────────────────────────────
    Break, Continue, Return, Throw, Debugger,
    If, Else,
    For, While, Do,
    Switch, Case, Default,
    In, Of,
    With,

    // ── Declaration keywords ───────────────────────────────────────────────────
    Var, Let, Const,
    Function, Class, Extends,

    // ── Operator keywords ──────────────────────────────────────────────────────
    Delete, Typeof, Void, Instanceof,

    // ── Module keywords ────────────────────────────────────────────────────────
    Import, Export, From, As,

    // ── Async / generator keywords ─────────────────────────────────────────────
    Async, Await, Yield,

    // ── Exception keywords ─────────────────────────────────────────────────────
    Try, Catch, Finally,

    // ── Class-body contextual keywords ────────────────────────────────────────
    Static, Get, Set,

    // ── Meta property identifiers ──────────────────────────────────────────────
    Target, // new.target
    Meta,   // import.meta

    // ── Punctuation ───────────────────────────────────────────────────────────
    LParen,    // (
    RParen,    // )
    LBrace,    // {
    RBrace,    // }
    LBracket,  // [
    RBracket,  // ]
    Semi,      // ;
    Colon,     // :
    Comma,     // ,
    Dot,       // .
    Spread,    // ...
    Arrow,     // =>
    Hash,      // # (private class fields)
    Question,  // ?
    QuestionDot, // ?.

    // ── Arithmetic operators ───────────────────────────────────────────────────
    Plus,       // +
    Minus,      // -
    Star,       // *
    Slash,      // /
    Percent,    // %
    StarStar,   // **
    PlusPlus,   // ++
    MinusMinus, // --

    // ── Comparison operators ───────────────────────────────────────────────────
    EqEq,      // ==
    BangEq,    // !=
    EqEqEq,    // ===
    BangEqEq,  // !==
    Lt,        // <
    LtEq,      // <=
    Gt,        // >
    GtEq,      // >=

    // ── Logical operators ──────────────────────────────────────────────────────
    Bang,              // !
    AmpAmp,            // &&
    PipePipe,          // ||
    QuestionQuestion,  // ??

    // ── Bitwise operators ──────────────────────────────────────────────────────
    Amp,    // &
    Pipe,   // |
    Caret,  // ^
    Tilde,  // ~
    LtLt,   // <<
    GtGt,   // >>
    GtGtGt, // >>>

    // ── Assignment operators ───────────────────────────────────────────────────
    Eq,              // =
    PlusEq,          // +=
    MinusEq,         // -=
    StarEq,          // *=
    SlashEq,         // /=
    PercentEq,       // %=
    StarStarEq,      // **=
    AmpEq,           // &=
    PipeEq,          // |=
    CaretEq,         // ^=
    LtLtEq,          // <<=
    GtGtEq,          // >>=
    GtGtGtEq,        // >>>=
    AmpAmpEq,        // &&=
    PipePipeEq,      // ||=
    QuestionQuestionEq, // ??=

    // ── End of input ──────────────────────────────────────────────────────────
    Eof,
}

impl TokenKind {
    /// Is this token an assignment operator?
    pub fn is_assignment_op(&self) -> bool {
        matches!(
            self,
            Self::Eq | Self::PlusEq | Self::MinusEq | Self::StarEq
            | Self::SlashEq | Self::PercentEq | Self::StarStarEq
            | Self::AmpEq | Self::PipeEq | Self::CaretEq
            | Self::LtLtEq | Self::GtGtEq | Self::GtGtGtEq
            | Self::AmpAmpEq | Self::PipePipeEq | Self::QuestionQuestionEq
        )
    }

    /// Is this a prefix-unary operator keyword or symbol?
    pub fn is_unary_op(&self) -> bool {
        matches!(
            self,
            Self::Bang | Self::Tilde | Self::Plus | Self::Minus
            | Self::Typeof | Self::Void | Self::Delete
        )
    }

    /// Is this `++` or `--`?
    pub fn is_update_op(&self) -> bool {
        matches!(self, Self::PlusPlus | Self::MinusMinus)
    }

    /// Could this be a keyword that also reads as an identifier in sloppy mode?
    /// These are "contextual keywords": `let`, `static`, `async`, `get`, `set`,
    /// `from`, `as`, `of`, `target`, `meta`.
    pub fn is_contextual_keyword(&self) -> bool {
        matches!(
            self,
            Self::Let | Self::Static | Self::Async | Self::Get
            | Self::Set | Self::From | Self::As | Self::Of
            | Self::Target | Self::Meta
        )
    }

    /// Return the string representation of a keyword/punctuation token, used
    /// for error messages.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::True       => "true",
            Self::False      => "false",
            Self::Null       => "null",
            Self::This       => "this",
            Self::Super      => "super",
            Self::New        => "new",
            Self::Break      => "break",
            Self::Continue   => "continue",
            Self::Return     => "return",
            Self::Throw      => "throw",
            Self::Debugger   => "debugger",
            Self::If         => "if",
            Self::Else       => "else",
            Self::For        => "for",
            Self::While      => "while",
            Self::Do         => "do",
            Self::Switch     => "switch",
            Self::Case       => "case",
            Self::Default    => "default",
            Self::In         => "in",
            Self::Of         => "of",
            Self::With       => "with",
            Self::Var        => "var",
            Self::Let        => "let",
            Self::Const      => "const",
            Self::Function   => "function",
            Self::Class      => "class",
            Self::Extends    => "extends",
            Self::Delete     => "delete",
            Self::Typeof     => "typeof",
            Self::Void       => "void",
            Self::Instanceof => "instanceof",
            Self::Import     => "import",
            Self::Export     => "export",
            Self::From       => "from",
            Self::As         => "as",
            Self::Async      => "async",
            Self::Await      => "await",
            Self::Yield      => "yield",
            Self::Try        => "try",
            Self::Catch      => "catch",
            Self::Finally    => "finally",
            Self::Static     => "static",
            Self::Get        => "get",
            Self::Set        => "set",
            Self::Target     => "target",
            Self::Meta       => "meta",
            Self::LParen     => "(",
            Self::RParen     => ")",
            Self::LBrace     => "{",
            Self::RBrace     => "}",
            Self::LBracket   => "[",
            Self::RBracket   => "]",
            Self::Semi       => ";",
            Self::Colon      => ":",
            Self::Comma      => ",",
            Self::Dot        => ".",
            Self::Spread     => "...",
            Self::Arrow      => "=>",
            Self::Hash       => "#",
            Self::Question   => "?",
            Self::QuestionDot => "?.",
            Self::Plus       => "+",
            Self::Minus      => "-",
            Self::Star       => "*",
            Self::Slash      => "/",
            Self::Percent    => "%",
            Self::StarStar   => "**",
            Self::PlusPlus   => "++",
            Self::MinusMinus => "--",
            Self::EqEq       => "==",
            Self::BangEq     => "!=",
            Self::EqEqEq     => "===",
            Self::BangEqEq   => "!==",
            Self::Lt         => "<",
            Self::LtEq       => "<=",
            Self::Gt         => ">",
            Self::GtEq       => ">=",
            Self::Bang       => "!",
            Self::AmpAmp     => "&&",
            Self::PipePipe   => "||",
            Self::QuestionQuestion => "??",
            Self::Amp        => "&",
            Self::Pipe       => "|",
            Self::Caret      => "^",
            Self::Tilde      => "~",
            Self::LtLt       => "<<",
            Self::GtGt       => ">>",
            Self::GtGtGt     => ">>>",
            Self::Eq         => "=",
            Self::PlusEq     => "+=",
            Self::MinusEq    => "-=",
            Self::StarEq     => "*=",
            Self::SlashEq    => "/=",
            Self::PercentEq  => "%=",
            Self::StarStarEq => "**=",
            Self::AmpEq      => "&=",
            Self::PipeEq     => "|=",
            Self::CaretEq    => "^=",
            Self::LtLtEq     => "<<=",
            Self::GtGtEq     => ">>=",
            Self::GtGtGtEq   => ">>>=",
            Self::AmpAmpEq   => "&&=",
            Self::PipePipeEq => "||=",
            Self::QuestionQuestionEq => "??=",
            Self::Eof        => "<eof>",
            Self::Ident(_)   => "<identifier>",
            Self::Number(_)  => "<number>",
            Self::BigInt(_)  => "<bigint>",
            Self::Str(_)     => "<string>",
            Self::Regex{..}  => "<regex>",
            Self::TemplateNoSub(_) | Self::TemplateHead(_)
            | Self::TemplateMiddle(_) | Self::TemplateTail(_) => "<template>",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Token
// ─────────────────────────────────────────────────────────────────────────────

/// A single lexeme with its source location and whitespace context.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    /// The classified lexeme kind.
    pub kind: TokenKind,
    /// Byte range within the source string.
    pub span: Span,
    /// `true` if at least one **line terminator** appeared in the whitespace
    /// between the previous token and this one. This is used by the parser to
    /// implement **Automatic Semicolon Insertion** (ASI).
    pub preceded_by_newline: bool,
}

impl Token {
    /// Construct a new token.
    #[inline]
    pub fn new(kind: TokenKind, span: Span, preceded_by_newline: bool) -> Self {
        Self { kind, span, preceded_by_newline }
    }

    /// Convenience: is this the end-of-file sentinel?
    #[inline]
    pub fn is_eof(&self) -> bool {
        self.kind == TokenKind::Eof
    }
}