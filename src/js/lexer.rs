//! JavaScript lexer / tokeniser.
//!
//! The [`Lexer`] struct converts raw UTF-8 source text into a stream of
//! [`Token`]s. It implements ECMAScript 2022 lexical grammar including:
//!
//! - All operators and punctuation
//! - Numeric literals (decimal int/float, hex, octal, binary, BigInt, `_`
//!   separators)
//! - String literals with full escape sequence decoding
//! - Template literal parts (head/middle/tail/no-substitution)
//! - Regular expression literals (context-sensitive detection)
//! - Identifiers and all reserved keywords
//! - Legacy HTML comment syntax (`<!--`, `-->`)
//! - Unicode identifiers via `\uXXXX` escapes

use alloc::{string::String, string::ToString, vec::Vec};

use super::{
    error::LexError,
    span::Span,
    token::{Token, TokenKind},
};

// ─────────────────────────────────────────────────────────────────────────────
// Lexer state
// ─────────────────────────────────────────────────────────────────────────────

/// Context that tells the lexer whether `/` starts a **regex** or a
/// **division** operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegexCtx {
    /// The `/` is a division operator (or `/=`).
    Div,
    /// The `/` starts a regular expression literal.
    Regex,
}

/// The JavaScript lexer.
///
/// Call [`Lexer::next_token`] repeatedly until you receive a token with kind
/// [`TokenKind::Eof`].
#[derive(Debug)]
pub struct Lexer<'src> {
    /// The full source text.
    source: &'src str,
    /// Current byte position in `source`.
    pos: usize,
    /// Stack of brace depths for nested template expressions.
    ///
    /// Each entry tracks `{ … }` depth for one active template substitution.
    /// When the depth drops to zero the lexer resumes reading template chars.
    template_stack: Vec<u32>,
    /// Whether the most recent non-trivial token could be followed by a
    /// division operator (vs. the start of a regex literal).
    pub regex_ctx: RegexCtx,
}

impl<'src> Lexer<'src> {
    /// Create a new lexer from a source string.
    pub fn new(source: &'src str) -> Self {
        Self {
            source,
            pos: 0,
            template_stack: Vec::new(),
            regex_ctx: RegexCtx::Regex,
        }
    }

    // ── Primitive helpers ─────────────────────────────────────────────────────

    /// Current byte (or `\0` at EOF).
    #[inline]
    fn cur(&self) -> u8 {
        self.source.as_bytes().get(self.pos).copied().unwrap_or(0)
    }

    /// Byte at `pos + offset` (or `\0`).
    #[inline]
    fn peek_at(&self, offset: usize) -> u8 {
        self.source.as_bytes().get(self.pos + offset).copied().unwrap_or(0)
    }

    /// `true` if we are at or past the end of the source.
    #[inline]
    fn at_end(&self) -> bool {
        self.pos >= self.source.len()
    }

    /// Advance by one byte and return it.
    #[inline]
    fn bump(&mut self) -> u8 {
        let b = self.cur();
        self.pos += 1;
        b
    }

    /// Advance past a multi-byte UTF-8 character starting at `self.pos` and
    /// return it.
    #[inline]
    fn bump_char(&mut self) -> char {
        let s = &self.source[self.pos..];
        let ch = s.chars().next().unwrap_or('\0');
        self.pos += ch.len_utf8();
        ch
    }

    /// Peek at the next character without consuming.
    #[inline]
    fn peek_char(&self) -> char {
        self.source[self.pos..].chars().next().unwrap_or('\0')
    }

    /// Mark the start of a token.
    #[inline]
    fn mark(&self) -> u32 {
        self.pos as u32
    }

    /// Build a span from `start` to `self.pos`.
    #[inline]
    fn span(&self, start: u32) -> Span {
        Span::new(start, self.pos as u32)
    }

    // ── Whitespace / comments ─────────────────────────────────────────────────

    /// Skip whitespace and comments.
    ///
    /// Returns `true` if any line terminator was encountered.
    fn skip_whitespace(&mut self) -> bool {
        let mut saw_newline = false;
        loop {
            match self.cur() {
                // ASCII whitespace
                b' ' | b'\t' | b'\x0C' | b'\x0B' => { self.pos += 1; }
                // Line terminators
                b'\n' => { self.pos += 1; saw_newline = true; }
                b'\r' => {
                    self.pos += 1;
                    if self.cur() == b'\n' { self.pos += 1; }
                    saw_newline = true;
                }
                // Line comment
                b'/' if self.peek_at(1) == b'/' => {
                    self.pos += 2;
                    while !self.at_end() && self.cur() != b'\n' && self.cur() != b'\r' {
                        self.pos += 1;
                    }
                }
                // Block comment
                b'/' if self.peek_at(1) == b'*' => {
                    let start = self.pos;
                    self.pos += 2;
                    loop {
                        if self.at_end() {
                            // Unterminated — we'll handle the error in next_token
                            break;
                        }
                        if self.cur() == b'*' && self.peek_at(1) == b'/' {
                            self.pos += 2;
                            break;
                        }
                        if self.cur() == b'\n' || self.cur() == b'\r' {
                            saw_newline = true;
                        }
                        self.pos += 1;
                    }
                    let _ = start; // used in error reporting below
                }
                // HTML open comment (legacy): <!-- is treated as a line comment
                b'<' if self.peek_at(1) == b'!'
                      && self.peek_at(2) == b'-'
                      && self.peek_at(3) == b'-' =>
                {
                    self.pos += 4;
                    while !self.at_end() && self.cur() != b'\n' && self.cur() != b'\r' {
                        self.pos += 1;
                    }
                }
                // HTML close comment (legacy): --> at start of line
                b'-' if saw_newline
                      && self.peek_at(1) == b'-'
                      && self.peek_at(2) == b'>' =>
                {
                    self.pos += 3;
                    while !self.at_end() && self.cur() != b'\n' && self.cur() != b'\r' {
                        self.pos += 1;
                    }
                }
                // Unicode non-ASCII whitespace / line separator (U+2028/U+2029)
                _ if !self.at_end() => {
                    let ch = self.peek_char();
                    match ch {
                        '\u{00A0}' | '\u{FEFF}' | '\u{1680}'
                        | '\u{2000}'..='\u{200A}'
                        | '\u{202F}' | '\u{205F}' | '\u{3000}' => {
                            self.pos += ch.len_utf8();
                        }
                        '\u{2028}' | '\u{2029}' => {
                            self.pos += ch.len_utf8();
                            saw_newline = true;
                        }
                        _ => break,
                    }
                }
                _ => break,
            }
        }
        saw_newline
    }

    // ── Number literals ───────────────────────────────────────────────────────

    fn lex_number(&mut self, start: u32) -> Result<Token, LexError> {
        let is_zero = self.source.as_bytes()[start as usize] == b'0';
        let mut is_bigint = false;

        if is_zero {
            match self.cur() {
                b'x' | b'X' => return self.lex_int_radix(start, 16, is_bigint),
                b'o' | b'O' => return self.lex_int_radix(start, 8, is_bigint),
                b'b' | b'B' => return self.lex_int_radix(start, 2, is_bigint),
                _ => {}
            }
        }

        // Decimal integer or float
        self.skip_decimal_digits();

        // Fractional part
        let is_float = self.cur() == b'.' && {
            let next = self.peek_at(1);
            (next >= b'0' && next <= b'9') || next == b'_'
                || (is_zero && next != b'.') // e.g. 0. is valid
                || next == b'e' || next == b'E'
                || (self.cur() == b'.' && (self.peek_at(1) < b'0' || self.peek_at(1) > b'9')
                    && self.peek_at(1) != b'e' && self.peek_at(1) != b'E'
                    && self.peek_at(1) != b'_')
        };

        // Better: just check cur == '.' and not next == '.'
        let is_float = if self.cur() == b'.' && self.peek_at(1) != b'.' {
            // could be 1.toFixed() — check next char
            if self.peek_at(1) == b'.' {
                false
            } else {
                self.pos += 1;
                self.skip_decimal_digits();
                true
            }
        } else {
            false
        };
        let _ = is_float;

        // Exponent
        if self.cur() == b'e' || self.cur() == b'E' {
            self.pos += 1;
            if self.cur() == b'+' || self.cur() == b'-' { self.pos += 1; }
            if !self.cur().is_ascii_digit() {
                return Err(LexError::InvalidNumericLiteral(self.span(start)));
            }
            self.skip_decimal_digits();
        }

        // BigInt suffix
        if self.cur() == b'n' {
            self.pos += 1;
            is_bigint = true;
        }

        let raw = &self.source[start as usize..self.pos];
        let raw_no_sep = raw.replace('_', "");

        if is_bigint {
            let digits = raw_no_sep.trim_end_matches('n').to_string();
            return Ok(Token::new(TokenKind::BigInt(digits), self.span(start), false));
        }

        let value: f64 = raw_no_sep.parse().unwrap_or(f64::NAN);
        Ok(Token::new(TokenKind::Number(value), self.span(start), false))
    }

    fn lex_int_radix(&mut self, start: u32, radix: u32, _is_bigint: bool) -> Result<Token, LexError> {
        self.pos += 1; // skip 'x'/'o'/'b'
        let digit_start = self.pos;
        loop {
            match self.cur() {
                b'0'..=b'9' | b'a'..=b'f' | b'A'..=b'F' | b'_' => { self.pos += 1; }
                _ => break,
            }
        }
        if self.pos == digit_start {
            return Err(LexError::InvalidNumericLiteral(self.span(start)));
        }
        let is_bigint = self.cur() == b'n' && { self.pos += 1; true };
        let raw = &self.source[digit_start..self.pos - if is_bigint { 1 } else { 0 }];
        let raw_clean = raw.replace('_', "");
        if is_bigint {
            return Ok(Token::new(TokenKind::BigInt(raw_clean), self.span(start), false));
        }
        let value = u64::from_str_radix(&raw_clean, radix)
            .map(|n| n as f64)
            .unwrap_or(f64::NAN);
        Ok(Token::new(TokenKind::Number(value), self.span(start), false))
    }

    fn skip_decimal_digits(&mut self) {
        loop {
            match self.cur() {
                b'0'..=b'9' | b'_' => { self.pos += 1; }
                _ => break,
            }
        }
    }

    // ── String literals ───────────────────────────────────────────────────────

    fn lex_string(&mut self, quote: u8) -> Result<String, LexError> {
        let start = (self.pos - 1) as u32; // opening quote already consumed
        let mut buf = String::new();
        loop {
            if self.at_end() {
                return Err(LexError::UnterminatedString(Span::new(start, self.pos as u32)));
            }
            let ch = self.bump_char();
            match ch {
                c if c as u32 == quote as u32 => break,
                '\n' | '\r' | '\u{2028}' | '\u{2029}' => {
                    return Err(LexError::UnterminatedString(Span::new(start, self.pos as u32)));
                }
                '\\' => {
                    let esc = self.decode_escape(start)?;
                    buf.push(esc);
                }
                c => buf.push(c),
            }
        }
        Ok(buf)
    }

    /// Decode a single escape sequence (the `\` has already been consumed).
    fn decode_escape(&mut self, str_start: u32) -> Result<char, LexError> {
        if self.at_end() {
            return Err(LexError::UnterminatedString(Span::new(str_start, self.pos as u32)));
        }
        let span_start = (self.pos - 1) as u32;
        let ch = self.bump_char();
        Ok(match ch {
            'n'  => '\n',
            'r'  => '\r',
            't'  => '\t',
            'b'  => '\x08',
            'f'  => '\x0C',
            'v'  => '\x0B',
            '0' if !self.cur().is_ascii_digit() => '\0',
            '\n' => '\0', // line continuation → ignored
            '\r' => {
                if self.cur() == b'\n' { self.pos += 1; }
                '\0'
            }
            'x'  => {
                let hi = self.hex_digit(span_start)?;
                let lo = self.hex_digit(span_start)?;
                char::from_u32((hi << 4) | lo).unwrap_or('\0')
            }
            'u'  => self.decode_unicode_escape(span_start)?,
            // Any other char after \ is just that char (or the char itself)
            c    => c,
        })
    }

    fn hex_digit(&mut self, span_start: u32) -> Result<u32, LexError> {
        let ch = self.peek_char();
        let val = ch.to_digit(16).ok_or_else(|| LexError::InvalidEscape {
            span: Span::new(span_start, self.pos as u32 + ch.len_utf8() as u32),
            ch,
        })?;
        self.pos += ch.len_utf8();
        Ok(val)
    }

    fn decode_unicode_escape(&mut self, span_start: u32) -> Result<char, LexError> {
        if self.cur() == b'{' {
            // \u{HHHH}
            self.pos += 1;
            let mut code = 0u32;
            let mut has_digit = false;
            loop {
                let ch = self.peek_char();
                if ch == '}' { self.pos += 1; break; }
                let d = ch.to_digit(16).ok_or(LexError::InvalidUnicodeEscape(
                    Span::new(span_start, self.pos as u32),
                ))?;
                code = (code << 4) | d;
                if code > 0x10FFFF {
                    return Err(LexError::InvalidUnicodeEscape(Span::new(span_start, self.pos as u32)));
                }
                self.pos += ch.len_utf8();
                has_digit = true;
            }
            if !has_digit {
                return Err(LexError::InvalidUnicodeEscape(Span::new(span_start, self.pos as u32)));
            }
            char::from_u32(code).ok_or(LexError::InvalidUnicodeEscape(Span::new(span_start, self.pos as u32)))
        } else {
            // \uHHHH
            let mut code = 0u32;
            for _ in 0..4 {
                code = (code << 4) | self.hex_digit(span_start)?;
            }
            char::from_u32(code).ok_or(LexError::InvalidUnicodeEscape(Span::new(span_start, self.pos as u32)))
        }
    }

    // ── Template literals ─────────────────────────────────────────────────────

    /// Lex from the opening backtick (already consumed). Returns a
    /// `TemplateNoSub` or `TemplateHead` token.
    fn lex_template_start(&mut self, start: u32) -> Result<Token, LexError> {
        let (cooked, is_tail) = self.lex_template_chars(start)?;
        let kind = if is_tail {
            TokenKind::TemplateNoSub(cooked)
        } else {
            self.template_stack.push(0); // enter expression
            TokenKind::TemplateHead(cooked)
        };
        Ok(Token::new(kind, self.span(start), false))
    }

    /// Resume a template after a `}`. Returns `TemplateMiddle` or `TemplateTail`.
    fn lex_template_resume(&mut self, start: u32) -> Result<Token, LexError> {
        let (cooked, is_tail) = self.lex_template_chars(start)?;
        let kind = if is_tail {
            self.template_stack.pop();
            TokenKind::TemplateTail(cooked)
        } else {
            TokenKind::TemplateMiddle(cooked)
        };
        Ok(Token::new(kind, self.span(start), false))
    }

    /// Read template characters until `` ` `` (tail) or `${` (substitution).
    /// Returns `(cooked_string, is_tail)`.
    fn lex_template_chars(&mut self, str_start: u32) -> Result<(String, bool), LexError> {
        let mut buf = String::new();
        loop {
            if self.at_end() {
                return Err(LexError::UnterminatedString(Span::new(str_start, self.pos as u32)));
            }
            let ch = self.bump_char();
            match ch {
                '`' => return Ok((buf, true)),  // end of template
                '$' if self.cur() == b'{' => {
                    self.pos += 1; // consume '{'
                    return Ok((buf, false)); // start of substitution
                }
                '\\' => {
                    // Template cooked values allow \<newline> continuations
                    let esc = self.decode_escape(str_start)?;
                    if esc != '\0' { buf.push(esc); } // \<newline> → empty
                }
                c => buf.push(c),
            }
        }
    }

    // ── Regular expressions ───────────────────────────────────────────────────

    fn lex_regex(&mut self, start: u32) -> Result<Token, LexError> {
        // Opening `/` already consumed. Read until unescaped closing `/`.
        let mut pattern = String::new();
        let mut in_class = false; // inside [...]
        loop {
            if self.at_end() || self.cur() == b'\n' || self.cur() == b'\r' {
                return Err(LexError::UnterminatedRegex(self.span(start)));
            }
            let ch = self.bump_char();
            match ch {
                '/' if !in_class => break,
                '[' => { in_class = true; pattern.push('['); }
                ']' if in_class => { in_class = false; pattern.push(']'); }
                '\\' if !self.at_end() => {
                    pattern.push('\\');
                    pattern.push(self.bump_char());
                }
                c => pattern.push(c),
            }
        }
        // Flags
        let mut flags = String::new();
        while self.cur().is_ascii_alphabetic() {
            flags.push(self.bump_char());
        }
        Ok(Token::new(TokenKind::Regex { pattern, flags }, self.span(start), false))
    }

    // ── Identifiers & keywords ────────────────────────────────────────────────

    fn lex_ident_or_keyword(&mut self, start: u32) -> Token {
        // Consume remaining ident chars
        loop {
            let ch = self.peek_char();
            if is_ident_continue(ch) {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }
        let text = &self.source[start as usize..self.pos];
        let kind = keyword_or_ident(text);
        Token::new(kind, self.span(start), false)
    }

    // ── Main entry point ──────────────────────────────────────────────────────

    /// Produce the next token from the source stream.
    ///
    /// Call this repeatedly; when [`TokenKind::Eof`] is returned the stream is
    /// exhausted.
    pub fn next_token(&mut self) -> Result<Token, LexError> {
        let preceded_by_newline = self.skip_whitespace();

        if self.at_end() {
            let pos = self.pos as u32;
            return Ok(Token::new(TokenKind::Eof, Span::new(pos, pos), preceded_by_newline));
        }

        // Handle template resume: if we are inside a template expression and
        // we see `}`, decrement brace depth; when it hits zero, resume template.
        if self.cur() == b'}' {
            if let Some(depth) = self.template_stack.last_mut() {
                if *depth == 0 {
                    let start = self.mark();
                    self.pos += 1;
                    let mut tok = self.lex_template_resume(start)?;
                    tok.preceded_by_newline = preceded_by_newline;
                    return Ok(tok);
                } else {
                    *depth -= 1;
                }
            }
        }

        // Track brace depth inside template substitutions
        if self.cur() == b'{' {
            if let Some(depth) = self.template_stack.last_mut() {
                *depth += 1;
            }
        }

        let start = self.mark();
        let b = self.bump();

        let kind = match b {
            // Whitespace / newlines already skipped above
            b'(' => TokenKind::LParen,
            b')' => TokenKind::RParen,
            b'{' => TokenKind::LBrace,
            b'}' => TokenKind::RBrace,
            b'[' => TokenKind::LBracket,
            b']' => TokenKind::RBracket,
            b';' => TokenKind::Semi,
            b',' => TokenKind::Comma,
            b':' => TokenKind::Colon,
            b'~' => TokenKind::Tilde,
            b'#' => TokenKind::Hash,

            b'.' => {
                if self.cur() == b'.' && self.peek_at(1) == b'.' {
                    self.pos += 2;
                    TokenKind::Spread
                } else if self.cur().is_ascii_digit() {
                    // .123 — numeric literal starting with dot
                    self.pos -= 1; // back up to include the dot
                    let s = self.mark();
                    self.pos += 1;
                    self.skip_decimal_digits();
                    if self.cur() == b'e' || self.cur() == b'E' {
                        self.pos += 1;
                        if self.cur() == b'+' || self.cur() == b'-' { self.pos += 1; }
                        self.skip_decimal_digits();
                    }
                    let raw = &self.source[s as usize..self.pos];
                    let val: f64 = raw.parse().unwrap_or(f64::NAN);
                    let mut tok = Token::new(TokenKind::Number(val), self.span(s), preceded_by_newline);
                    self.update_regex_ctx(&tok.kind);
                    return Ok(tok);
                } else {
                    TokenKind::Dot
                }
            }

            b'?' => {
                if self.cur() == b'?' {
                    self.pos += 1;
                    if self.cur() == b'=' { self.pos += 1; TokenKind::QuestionQuestionEq }
                    else { TokenKind::QuestionQuestion }
                } else if self.cur() == b'.' && !self.peek_at(1).is_ascii_digit() {
                    self.pos += 1;
                    TokenKind::QuestionDot
                } else {
                    TokenKind::Question
                }
            }

            b'`' => {
                let mut tok = self.lex_template_start(start)?;
                tok.preceded_by_newline = preceded_by_newline;
                self.update_regex_ctx(&tok.kind);
                return Ok(tok);
            }

            b'\'' | b'"' => {
                let s = self.lex_string(b)?;
                let kind = TokenKind::Str(s);
                let mut tok = Token::new(kind, self.span(start), preceded_by_newline);
                self.update_regex_ctx(&tok.kind);
                return Ok(tok);
            }

            b'0'..=b'9' => {
                let mut tok = self.lex_number(start)?;
                tok.preceded_by_newline = preceded_by_newline;
                self.update_regex_ctx(&tok.kind);
                return Ok(tok);
            }

            b'/' => {
                match self.regex_ctx {
                    RegexCtx::Regex => {
                        if self.cur() == b'/' {
                            // Line comment already handled in skip_whitespace;
                            // this branch shouldn't be reached — but just in case:
                            while !self.at_end() && self.cur() != b'\n' {
                                self.pos += 1;
                            }
                            return self.next_token();
                        } else if self.cur() == b'*' {
                            // Block comment likewise
                            return self.next_token();
                        }
                        let mut tok = self.lex_regex(start)?;
                        tok.preceded_by_newline = preceded_by_newline;
                        self.update_regex_ctx(&tok.kind);
                        return Ok(tok);
                    }
                    RegexCtx::Div => {
                        if self.cur() == b'=' {
                            self.pos += 1;
                            TokenKind::SlashEq
                        } else {
                            TokenKind::Slash
                        }
                    }
                }
            }

            b'+' => {
                if self.cur() == b'+' { self.pos += 1; TokenKind::PlusPlus }
                else if self.cur() == b'=' { self.pos += 1; TokenKind::PlusEq }
                else { TokenKind::Plus }
            }
            b'-' => {
                if self.cur() == b'-' { self.pos += 1; TokenKind::MinusMinus }
                else if self.cur() == b'=' { self.pos += 1; TokenKind::MinusEq }
                else { TokenKind::Minus }
            }
            b'*' => {
                if self.cur() == b'*' {
                    self.pos += 1;
                    if self.cur() == b'=' { self.pos += 1; TokenKind::StarStarEq }
                    else { TokenKind::StarStar }
                } else if self.cur() == b'=' {
                    self.pos += 1;
                    TokenKind::StarEq
                } else {
                    TokenKind::Star
                }
            }
            b'%' => {
                if self.cur() == b'=' { self.pos += 1; TokenKind::PercentEq }
                else { TokenKind::Percent }
            }
            b'&' => {
                if self.cur() == b'&' {
                    self.pos += 1;
                    if self.cur() == b'=' { self.pos += 1; TokenKind::AmpAmpEq }
                    else { TokenKind::AmpAmp }
                } else if self.cur() == b'=' {
                    self.pos += 1; TokenKind::AmpEq
                } else {
                    TokenKind::Amp
                }
            }
            b'|' => {
                if self.cur() == b'|' {
                    self.pos += 1;
                    if self.cur() == b'=' { self.pos += 1; TokenKind::PipePipeEq }
                    else { TokenKind::PipePipe }
                } else if self.cur() == b'=' {
                    self.pos += 1; TokenKind::PipeEq
                } else {
                    TokenKind::Pipe
                }
            }
            b'^' => {
                if self.cur() == b'=' { self.pos += 1; TokenKind::CaretEq }
                else { TokenKind::Caret }
            }
            b'!' => {
                if self.cur() == b'=' {
                    self.pos += 1;
                    if self.cur() == b'=' { self.pos += 1; TokenKind::BangEqEq }
                    else { TokenKind::BangEq }
                } else {
                    TokenKind::Bang
                }
            }
            b'=' => {
                if self.cur() == b'=' {
                    self.pos += 1;
                    if self.cur() == b'=' { self.pos += 1; TokenKind::EqEqEq }
                    else { TokenKind::EqEq }
                } else if self.cur() == b'>' {
                    self.pos += 1; TokenKind::Arrow
                } else {
                    TokenKind::Eq
                }
            }
            b'<' => {
                if self.cur() == b'<' {
                    self.pos += 1;
                    if self.cur() == b'=' { self.pos += 1; TokenKind::LtLtEq }
                    else { TokenKind::LtLt }
                } else if self.cur() == b'=' {
                    self.pos += 1; TokenKind::LtEq
                } else {
                    TokenKind::Lt
                }
            }
            b'>' => {
                if self.cur() == b'>' {
                    self.pos += 1;
                    if self.cur() == b'>' {
                        self.pos += 1;
                        if self.cur() == b'=' { self.pos += 1; TokenKind::GtGtGtEq }
                        else { TokenKind::GtGtGt }
                    } else if self.cur() == b'=' {
                        self.pos += 1; TokenKind::GtGtEq
                    } else {
                        TokenKind::GtGt
                    }
                } else if self.cur() == b'=' {
                    self.pos += 1; TokenKind::GtEq
                } else {
                    TokenKind::Gt
                }
            }

            // Identifier / keyword
            b if is_ident_start_byte(b) => {
                // Check for Unicode escape in identifier start: \uXXXX
                let mut tok = if b == b'\\' && self.cur() == b'u' {
                    // \uXXXX as first char of identifier — parse it
                    self.pos += 1;
                    let ch = self.decode_unicode_escape(start)
                        .map_err(|e| e)?;
                    let mut name = String::new();
                    name.push(ch);
                    loop {
                        let next = self.peek_char();
                        if is_ident_continue(next) { name.push(self.bump_char()); }
                        else { break; }
                    }
                    Token::new(keyword_or_ident(&name), self.span(start), preceded_by_newline)
                } else {
                    let mut tok = self.lex_ident_or_keyword(start);
                    tok.preceded_by_newline = preceded_by_newline;
                    tok
                };
                self.update_regex_ctx(&tok.kind);
                return Ok(tok);
            }

            other => {
                let ch = other as char;
                // Possibly a multi-byte UTF-8 identifier start
                if !self.at_end() {
                    let s = &self.source[start as usize..];
                    if let Some(c) = s.chars().next() {
                        if is_ident_start(c) {
                            self.pos = start as usize + c.len_utf8();
                            let mut tok = self.lex_ident_or_keyword(start);
                            tok.preceded_by_newline = preceded_by_newline;
                            self.update_regex_ctx(&tok.kind);
                            return Ok(tok);
                        }
                    }
                }
                return Err(LexError::UnexpectedChar {
                    span: Span::new(start, self.pos as u32),
                    ch,
                });
            }
        };

        self.update_regex_ctx(&kind);
        let mut tok = Token::new(kind, self.span(start), false);
        tok.preceded_by_newline = preceded_by_newline;
        Ok(tok)
    }

    // ── Regex context tracking ────────────────────────────────────────────────

    /// Update whether the *next* `/` should be interpreted as a regex or div.
    ///
    /// `/` is a regex when it follows:
    /// - An operator, punctuation (except `)` `]` `++` `--`), or keyword
    /// - The opening of a block `{`
    /// - Nothing (start of expression)
    fn update_regex_ctx(&mut self, kind: &TokenKind) {
        use TokenKind::*;
        self.regex_ctx = match kind {
            // After these tokens, `/` is division
            RParen | RBracket | PlusPlus | MinusMinus
            | Ident(_) | Number(_) | BigInt(_) | Str(_)
            | Regex { .. } | TemplateNoSub(_) | TemplateTail(_)
            | True | False | Null | This | Super => RegexCtx::Div,
            // After everything else, `/` starts a regex
            _ => RegexCtx::Regex,
        };
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Identifier helpers
// ─────────────────────────────────────────────────────────────────────────────

#[inline]
fn is_ident_start_byte(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b == b'$' || b == b'\\'
        || b >= 0x80 // multi-byte UTF-8 start byte
}

#[inline]
fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == '$'
}

#[inline]
fn is_ident_continue(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$' || c == '\u{200C}' || c == '\u{200D}'
}

// ─────────────────────────────────────────────────────────────────────────────
// Keyword table
// ─────────────────────────────────────────────────────────────────────────────

/// Map a raw identifier string to its keyword token, or wrap it as `Ident`.
fn keyword_or_ident(s: &str) -> TokenKind {
    match s {
        "true"       => TokenKind::True,
        "false"      => TokenKind::False,
        "null"       => TokenKind::Null,
        "this"       => TokenKind::This,
        "super"      => TokenKind::Super,
        "new"        => TokenKind::New,
        "break"      => TokenKind::Break,
        "continue"   => TokenKind::Continue,
        "return"     => TokenKind::Return,
        "throw"      => TokenKind::Throw,
        "debugger"   => TokenKind::Debugger,
        "if"         => TokenKind::If,
        "else"       => TokenKind::Else,
        "for"        => TokenKind::For,
        "while"      => TokenKind::While,
        "do"         => TokenKind::Do,
        "switch"     => TokenKind::Switch,
        "case"       => TokenKind::Case,
        "default"    => TokenKind::Default,
        "in"         => TokenKind::In,
        "of"         => TokenKind::Of,
        "with"       => TokenKind::With,
        "var"        => TokenKind::Var,
        "let"        => TokenKind::Let,
        "const"      => TokenKind::Const,
        "function"   => TokenKind::Function,
        "class"      => TokenKind::Class,
        "extends"    => TokenKind::Extends,
        "delete"     => TokenKind::Delete,
        "typeof"     => TokenKind::Typeof,
        "void"       => TokenKind::Void,
        "instanceof" => TokenKind::Instanceof,
        "import"     => TokenKind::Import,
        "export"     => TokenKind::Export,
        "from"       => TokenKind::From,
        "as"         => TokenKind::As,
        "async"      => TokenKind::Async,
        "await"      => TokenKind::Await,
        "yield"      => TokenKind::Yield,
        "try"        => TokenKind::Try,
        "catch"      => TokenKind::Catch,
        "finally"    => TokenKind::Finally,
        "static"     => TokenKind::Static,
        "get"        => TokenKind::Get,
        "set"        => TokenKind::Set,
        "target"     => TokenKind::Target,
        "meta"       => TokenKind::Meta,
        other        => TokenKind::Ident(other.to_string()),
    }
}