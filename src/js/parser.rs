//! The parser: converts a token stream into an [`ast::Program`].
//!
//! Expression parsing uses a **Pratt / top-down operator precedence** approach
//! (see `infix_bp`). Statement and declaration parsing uses classical
//! recursive descent.

use alloc::{boxed::Box, string::String, string::ToString, vec, vec::Vec};

use super::{
    ast::*,
    error::{ParseError, ParseResult},
    lexer::Lexer,
    span::Span,
    token::{Token, TokenKind},
};

// ─────────────────────────────────────────────────────────────────────────────
// Parser struct
// ─────────────────────────────────────────────────────────────────────────────

/// The JavaScript parser.
///
/// Maintain one token of lookahead (`self.peek`) beyond the current token
/// (`self.cur`). This is enough to handle all JS syntax unambiguously.
pub struct Parser<'src> {
    source: &'src str,
    lexer: Lexer<'src>,
    /// Current token.
    cur: Token,
    /// One-token lookahead.
    peek: Token,
    // ── Parse context ─────────────────────────────────────────────────────────
    source_type: SourceType,
    in_function: bool,
    in_async: bool,
    in_generator: bool,
    in_class: bool,
    /// `false` inside the init of a `for` loop header (forbids bare `in`).
    allow_in: bool,
    strict_mode: bool,
}

impl<'src> Parser<'src> {
    /// Create a parser for the given source text.
    pub fn new(source: &'src str, source_type: SourceType) -> Self {
        let mut lexer = Lexer::new(source);
        let cur = lexer.next_token().unwrap_or_else(|_| eof(0));
        let peek = lexer.next_token().unwrap_or_else(|_| eof(cur.span.end));
        Self {
            source,
            lexer,
            cur,
            peek,
            source_type,
            in_function: false,
            in_async: false,
            in_generator: false,
            in_class: false,
            allow_in: true,
            strict_mode: false,
        }
    }

    // ── Primitives ────────────────────────────────────────────────────────────

    /// Advance by one token, returning the token we just consumed.
    fn advance(&mut self) -> ParseResult<Token> {
        let prev = self.cur.clone();
        let next = self
            .lexer
            .next_token()
            .map_err(ParseError::Lex)
            .unwrap_or_else(|_| eof(self.peek.span.end));
        self.cur = core::mem::replace(&mut self.peek, next);
        Ok(prev)
    }

    /// Return the current token's span start.
    #[inline]
    fn start(&self) -> u32 {
        self.cur.span.start
    }

    /// Build a span from `start` to the end of the most recently consumed token.
    fn span_from(&self, start: u32) -> Span {
        Span::new(start, self.peek.span.start) // approximate — good enough
    }

    /// Span that covers from `start` to where we are now.
    fn span_to_cur(&self, start: u32) -> Span {
        Span::new(start, self.cur.span.end)
    }

    /// `true` if the current token matches `kind`.
    #[inline]
    fn check(&self, kind: &TokenKind) -> bool {
        core::mem::discriminant(&self.cur.kind) == core::mem::discriminant(kind)
            || &self.cur.kind == kind
    }

    /// `true` if the current token *is exactly* `kind` (pointer equality).
    #[inline]
    fn at(&self, kind: TokenKind) -> bool {
        self.cur.kind == kind
    }

    /// Consume the current token if it matches `kind`; return whether we did.
    fn eat(&mut self, kind: TokenKind) -> ParseResult<bool> {
        if self.cur.kind == kind {
            self.advance()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Consume the current token, asserting it matches `kind`.
    fn expect(&mut self, kind: TokenKind) -> ParseResult<Token> {
        if self.cur.kind == kind {
            self.advance()
        } else {
            Err(ParseError::Expected {
                expected: kind,
                found: self.cur.kind.clone(),
                span: self.cur.span,
            })
        }
    }

    /// Consume the current token as an identifier-like token (including most
    /// contextual keywords), returning an [`Ident`].
    fn expect_ident(&mut self) -> ParseResult<Ident> {
        let span = self.cur.span;
        let name = match &self.cur.kind {
            TokenKind::Ident(s) => s.clone(),
            TokenKind::Let => "let".to_string(),
            TokenKind::Static => "static".to_string(),
            TokenKind::From => "from".to_string(),
            TokenKind::As => "as".to_string(),
            TokenKind::Of => "of".to_string(),
            TokenKind::Async => "async".to_string(),
            TokenKind::Get => "get".to_string(),
            TokenKind::Set => "set".to_string(),
            TokenKind::Target => "target".to_string(),
            TokenKind::Meta => "meta".to_string(),
            _ => {
                return Err(ParseError::Expected {
                    expected: TokenKind::Ident(String::new()),
                    found: self.cur.kind.clone(),
                    span,
                });
            }
        };
        self.advance()?;
        Ok(Ident::new(name, span))
    }

    /// Try to consume a semicolon.
    ///
    /// Implements **Automatic Semicolon Insertion** (ASI): a semicolon is
    /// implied when:
    /// 1. An explicit `;` appears.
    /// 2. The next token is preceded by a line break.
    /// 3. The next token is `}` (end of block).
    /// 4. We are at end of file.
    fn expect_semi(&mut self) -> ParseResult<()> {
        if self.at(TokenKind::Semi) {
            self.advance()?;
            return Ok(());
        }
        if self.at(TokenKind::Eof) || self.at(TokenKind::RBrace) || self.cur.preceded_by_newline {
            return Ok(());
        }
        Err(ParseError::Expected {
            expected: TokenKind::Semi,
            found: self.cur.kind.clone(),
            span: self.cur.span,
        })
    }

    /// Is the current token a keyword that can start a statement?
    fn is_stmt_keyword(&self) -> bool {
        matches!(
            self.cur.kind,
            TokenKind::If
                | TokenKind::While
                | TokenKind::Do
                | TokenKind::For
                | TokenKind::Switch
                | TokenKind::Break
                | TokenKind::Continue
                | TokenKind::Return
                | TokenKind::Throw
                | TokenKind::Try
                | TokenKind::With
                | TokenKind::Debugger
                | TokenKind::Var
                | TokenKind::Const
                | TokenKind::Function
                | TokenKind::Class
        )
    }

    /// Check if cur is `let` used as a declaration keyword (not an identifier).
    fn is_let_decl(&self) -> bool {
        if self.cur.kind != TokenKind::Let {
            return false;
        }
        // `let` is a declaration if followed by `[`, `{`, or an identifier
        matches!(
            &self.peek.kind,
            TokenKind::LBracket
                | TokenKind::LBrace
                | TokenKind::Ident(_)
                | TokenKind::Let
                | TokenKind::Yield
                | TokenKind::Await
        )
    }

    /// Is `async` here the start of `async function` or `async () =>`?
    fn is_async_fn(&self) -> bool {
        if self.cur.kind != TokenKind::Async {
            return false;
        }
        !self.peek.preceded_by_newline && matches!(&self.peek.kind, TokenKind::Function)
    }

    // ── Public entry points ───────────────────────────────────────────────────

    /// Parse the entire program into a [`Program`] node.
    pub fn parse_program(&mut self) -> ParseResult<Program> {
        let start = self.start();
        let mut body = Vec::new();

        while !self.at(TokenKind::Eof) {
            body.push(self.parse_program_item()?);
        }

        Ok(Program {
            source_type: self.source_type,
            body,
            span: Span::new(start, self.cur.span.end),
        })
    }

    /// Parse a single expression (for `parse_expression` API).
    pub fn parse_expr_only(&mut self) -> ParseResult<Expr> {
        let expr = self.parse_expr(0)?;
        if !self.at(TokenKind::Eof) {
            return Err(ParseError::Unexpected {
                found: self.cur.kind.clone(),
                span: self.cur.span,
                hint: "expected end of input after expression".into(),
            });
        }
        Ok(expr)
    }

    // ── Program items ─────────────────────────────────────────────────────────

    fn parse_program_item(&mut self) -> ParseResult<ProgramItem> {
        if self.source_type == SourceType::Module {
            if self.at(TokenKind::Import) {
                // Could be `import(...)` dynamic import expression or import decl
                if !matches!(self.peek.kind, TokenKind::LParen | TokenKind::Dot) {
                    return Ok(ProgramItem::Import(self.parse_import_decl()?));
                }
            }
            if self.at(TokenKind::Export) {
                return Ok(ProgramItem::Export(self.parse_export_decl()?));
            }
        }
        Ok(ProgramItem::Stmt(self.parse_stmt()?))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Statements
    // ─────────────────────────────────────────────────────────────────────────

    pub(crate) fn parse_stmt(&mut self) -> ParseResult<Stmt> {
        match &self.cur.kind.clone() {
            TokenKind::LBrace => Ok(Stmt::Block(self.parse_block()?)),
            TokenKind::Semi => {
                let s = self.cur.span;
                self.advance()?;
                Ok(Stmt::Empty(s))
            }
            TokenKind::If => self.parse_if(),
            TokenKind::While => self.parse_while(),
            TokenKind::Do => self.parse_do_while(),
            TokenKind::For => self.parse_for(),
            TokenKind::Switch => self.parse_switch(),
            TokenKind::Break => self.parse_break(),
            TokenKind::Continue => self.parse_continue(),
            TokenKind::Return => self.parse_return(),
            TokenKind::Throw => self.parse_throw(),
            TokenKind::Try => self.parse_try(),
            TokenKind::With => self.parse_with(),
            TokenKind::Debugger => {
                let s = self.cur.span;
                self.advance()?;
                self.expect_semi()?;
                Ok(Stmt::Debugger(s))
            }
            TokenKind::Var => Ok(Stmt::Decl(Decl::Var(
                self.parse_var_decl(VarKind::Var, true)?,
            ))),
            TokenKind::Const => Ok(Stmt::Decl(Decl::Var(
                self.parse_var_decl(VarKind::Const, true)?,
            ))),
            TokenKind::Function => Ok(Stmt::Decl(Decl::Fn(self.parse_fn_decl(false, false)?))),
            TokenKind::Class => Ok(Stmt::Decl(Decl::Class(self.parse_class_decl(false)?))),
            TokenKind::Async if self.is_async_fn() => {
                Ok(Stmt::Decl(Decl::Fn(self.parse_fn_decl(true, false)?)))
            }
            _ if self.is_let_decl() => Ok(Stmt::Decl(Decl::Var(
                self.parse_var_decl(VarKind::Let, true)?,
            ))),
            _ => self.parse_expr_or_labeled_stmt(),
        }
    }

    fn parse_expr_or_labeled_stmt(&mut self) -> ParseResult<Stmt> {
        let start = self.start();
        // Check for labeled statement: `identifier ':'`
        let is_label =
            matches!(&self.cur.kind, TokenKind::Ident(_)) && self.peek.kind == TokenKind::Colon;
        if is_label {
            let label = self.expect_ident()?;
            self.advance()?; // consume ':'
            let body = Box::new(self.parse_stmt()?);
            return Ok(Stmt::Label(LabeledStmt {
                label,
                body,
                span: self.span_from(start),
            }));
        }

        let expr = self.parse_expr(0)?;
        let span = self.span_to_cur(start);

        // Detect directive prologues: string literals at function/module start
        let directive = if let Expr::Lit(Lit::Str(s), _) = &expr {
            Some(s.clone())
        } else {
            None
        };

        self.expect_semi()?;
        Ok(Stmt::Expr(ExprStmt {
            expr,
            span,
            directive,
        }))
    }

    // ── Block ─────────────────────────────────────────────────────────────────

    pub(crate) fn parse_block(&mut self) -> ParseResult<BlockStmt> {
        let start = self.start();
        self.expect(TokenKind::LBrace)?;
        let mut body = Vec::new();
        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            body.push(self.parse_stmt()?);
        }
        let end = self.cur.span.end;
        self.expect(TokenKind::RBrace)?;
        Ok(BlockStmt {
            body,
            span: Span::new(start, end),
        })
    }

    // ── if ────────────────────────────────────────────────────────────────────

    fn parse_if(&mut self) -> ParseResult<Stmt> {
        let start = self.start();
        self.expect(TokenKind::If)?;
        self.expect(TokenKind::LParen)?;
        let test = self.parse_expr(0)?;
        self.expect(TokenKind::RParen)?;
        let consequent = Box::new(self.parse_stmt()?);
        let alternate = if self.eat(TokenKind::Else)? {
            Some(Box::new(self.parse_stmt()?))
        } else {
            None
        };
        Ok(Stmt::If(IfStmt {
            test,
            consequent,
            alternate,
            span: self.span_from(start),
        }))
    }

    // ── while ─────────────────────────────────────────────────────────────────

    fn parse_while(&mut self) -> ParseResult<Stmt> {
        let start = self.start();
        self.expect(TokenKind::While)?;
        self.expect(TokenKind::LParen)?;
        let test = self.parse_expr(0)?;
        self.expect(TokenKind::RParen)?;
        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt::While(WhileStmt {
            test,
            body,
            span: self.span_from(start),
        }))
    }

    // ── do-while ──────────────────────────────────────────────────────────────

    fn parse_do_while(&mut self) -> ParseResult<Stmt> {
        let start = self.start();
        self.expect(TokenKind::Do)?;
        let body = Box::new(self.parse_stmt()?);
        self.expect(TokenKind::While)?;
        self.expect(TokenKind::LParen)?;
        let test = self.parse_expr(0)?;
        self.expect(TokenKind::RParen)?;
        self.expect_semi()?;
        Ok(Stmt::DoWhile(DoWhileStmt {
            body,
            test,
            span: self.span_from(start),
        }))
    }

    // ── for ───────────────────────────────────────────────────────────────────

    fn parse_for(&mut self) -> ParseResult<Stmt> {
        let start = self.start();
        self.expect(TokenKind::For)?;
        let is_await = if self.in_async && self.at(TokenKind::Await) {
            self.advance()?;
            true
        } else {
            false
        };
        self.expect(TokenKind::LParen)?;

        // Parse the init / left-hand side
        let (kind, decl_or_pat, is_for_in_of) = self.parse_for_head()?;

        match is_for_in_of {
            Some(is_of) => {
                // for-in / for-of
                let right = self.parse_expr(0)?;
                self.expect(TokenKind::RParen)?;
                let body = Box::new(self.parse_stmt()?);
                let span = self.span_from(start);
                if is_of {
                    Ok(Stmt::ForOf(ForOfStmt {
                        left: decl_or_pat.unwrap(),
                        right,
                        body,
                        is_await,
                        span,
                    }))
                } else {
                    Ok(Stmt::ForIn(ForInStmt {
                        left: decl_or_pat.unwrap(),
                        right,
                        body,
                        span,
                    }))
                }
            }
            None => {
                // Classic for(init; test; update)
                let init = kind;
                self.expect(TokenKind::Semi)?;
                let test = if self.at(TokenKind::Semi) {
                    None
                } else {
                    Some(self.parse_expr(0)?)
                };
                self.expect(TokenKind::Semi)?;
                let update = if self.at(TokenKind::RParen) {
                    None
                } else {
                    Some(self.parse_expr(0)?)
                };
                self.expect(TokenKind::RParen)?;
                let body = Box::new(self.parse_stmt()?);
                Ok(Stmt::For(ForStmt {
                    init,
                    test,
                    update,
                    body,
                    span: self.span_from(start),
                }))
            }
        }
    }

    /// Parse the header of a `for` loop up to (but not including) the `)`.
    ///
    /// Returns `(init_or_none, lhs_or_none, is_for_in_of)` where:
    /// - `is_for_in_of = Some(true)` → `for-of`
    /// - `is_for_in_of = Some(false)` → `for-in`
    /// - `is_for_in_of = None` → classic `for`
    fn parse_for_head(
        &mut self,
    ) -> ParseResult<(Option<ForInit>, Option<ForInLeft>, Option<bool>)> {
        if self.at(TokenKind::Semi) {
            return Ok((None, None, None));
        }

        // Variable declaration
        if matches!(self.cur.kind, TokenKind::Var | TokenKind::Const) || self.is_let_decl() {
            let kind = match &self.cur.kind {
                TokenKind::Var => VarKind::Var,
                TokenKind::Const => VarKind::Const,
                _ => VarKind::Let,
            };
            let decl_start = self.start();
            self.advance()?;
            let id = self.parse_binding_pat()?;

            // for (var x in ...) or for (var x of ...)
            if self.at(TokenKind::In) || (self.at(TokenKind::Of) && kind != VarKind::Var) {
                let is_of = self.at(TokenKind::Of);
                self.advance()?;
                let left = ForInLeft::Decl(VarDecl {
                    kind,
                    decls: vec![VarDeclarator {
                        id,
                        init: None,
                        span: self.span_from(decl_start),
                    }],
                    span: self.span_from(decl_start),
                });
                return Ok((None, Some(left), Some(is_of)));
            }

            // Classic for — parse the rest of the init declaration
            let mut decls = vec![];
            let init_val = if self.eat(TokenKind::Eq)? {
                Some(self.parse_assign_expr()?)
            } else {
                None
            };
            decls.push(VarDeclarator {
                id,
                init: init_val,
                span: self.span_from(decl_start),
            });

            while self.eat(TokenKind::Comma)? {
                let d_start = self.start();
                let pat = self.parse_binding_pat()?;
                let init = if self.eat(TokenKind::Eq)? {
                    Some(self.parse_assign_expr()?)
                } else {
                    None
                };
                decls.push(VarDeclarator {
                    id: pat,
                    init,
                    span: self.span_from(d_start),
                });
            }

            let var_decl = VarDecl {
                kind,
                decls,
                span: self.span_from(decl_start),
            };
            return Ok((Some(ForInit::Decl(var_decl)), None, None));
        }

        // Expression init — temporarily disallow `in` operator
        let saved_allow_in = self.allow_in;
        self.allow_in = false;
        let expr = self.parse_expr(0)?;
        self.allow_in = saved_allow_in;

        if self.at(TokenKind::In) {
            self.advance()?;
            let left = ForInLeft::Pat(self.expr_to_pat(expr)?);
            return Ok((None, Some(left), Some(false)));
        }
        if self.at(TokenKind::Of) {
            self.advance()?;
            let left = ForInLeft::Pat(self.expr_to_pat(expr)?);
            return Ok((None, Some(left), Some(true)));
        }

        Ok((Some(ForInit::Expr(expr)), None, None))
    }

    // ── switch ────────────────────────────────────────────────────────────────

    fn parse_switch(&mut self) -> ParseResult<Stmt> {
        let start = self.start();
        self.expect(TokenKind::Switch)?;
        self.expect(TokenKind::LParen)?;
        let discriminant = self.parse_expr(0)?;
        self.expect(TokenKind::RParen)?;
        self.expect(TokenKind::LBrace)?;

        let mut cases = Vec::new();
        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            let case_start = self.start();
            let test = if self.at(TokenKind::Case) {
                self.advance()?;
                Some(self.parse_expr(0)?)
            } else {
                self.expect(TokenKind::Default)?;
                None
            };
            self.expect(TokenKind::Colon)?;

            let mut consequent = Vec::new();
            while !matches!(
                self.cur.kind,
                TokenKind::Case | TokenKind::Default | TokenKind::RBrace
            ) && !self.at(TokenKind::Eof)
            {
                consequent.push(self.parse_stmt()?);
            }
            cases.push(SwitchCase {
                test,
                consequent,
                span: self.span_from(case_start),
            });
        }
        let end = self.cur.span.end;
        self.expect(TokenKind::RBrace)?;
        Ok(Stmt::Switch(SwitchStmt {
            discriminant,
            cases,
            span: Span::new(start, end),
        }))
    }

    // ── break / continue ──────────────────────────────────────────────────────

    fn parse_break(&mut self) -> ParseResult<Stmt> {
        let start = self.start();
        self.expect(TokenKind::Break)?;
        let label = if !self.cur.preceded_by_newline && matches!(self.cur.kind, TokenKind::Ident(_))
        {
            Some(self.expect_ident()?)
        } else {
            None
        };
        self.expect_semi()?;
        Ok(Stmt::Break(BreakStmt {
            label,
            span: self.span_from(start),
        }))
    }

    fn parse_continue(&mut self) -> ParseResult<Stmt> {
        let start = self.start();
        self.expect(TokenKind::Continue)?;
        let label = if !self.cur.preceded_by_newline && matches!(self.cur.kind, TokenKind::Ident(_))
        {
            Some(self.expect_ident()?)
        } else {
            None
        };
        self.expect_semi()?;
        Ok(Stmt::Continue(ContinueStmt {
            label,
            span: self.span_from(start),
        }))
    }

    // ── return / throw ────────────────────────────────────────────────────────

    fn parse_return(&mut self) -> ParseResult<Stmt> {
        let start = self.start();
        self.expect(TokenKind::Return)?;
        let argument = if !self.cur.preceded_by_newline
            && !self.at(TokenKind::Semi)
            && !self.at(TokenKind::RBrace)
            && !self.at(TokenKind::Eof)
        {
            Some(self.parse_expr(0)?)
        } else {
            None
        };
        self.expect_semi()?;
        Ok(Stmt::Return(ReturnStmt {
            argument,
            span: self.span_from(start),
        }))
    }

    fn parse_throw(&mut self) -> ParseResult<Stmt> {
        let start = self.start();
        self.expect(TokenKind::Throw)?;
        if self.cur.preceded_by_newline {
            return Err(ParseError::InvalidContext {
                msg: "no line break allowed after 'throw'".into(),
                span: self.cur.span,
            });
        }
        let argument = self.parse_expr(0)?;
        self.expect_semi()?;
        Ok(Stmt::Throw(ThrowStmt {
            argument,
            span: self.span_from(start),
        }))
    }

    // ── try ───────────────────────────────────────────────────────────────────

    fn parse_try(&mut self) -> ParseResult<Stmt> {
        let start = self.start();
        self.expect(TokenKind::Try)?;
        let block = self.parse_block()?;

        let handler = if self.at(TokenKind::Catch) {
            let h_start = self.start();
            self.advance()?;
            let param = if self.eat(TokenKind::LParen)? {
                let p = self.parse_binding_pat()?;
                self.expect(TokenKind::RParen)?;
                Some(p)
            } else {
                None
            };
            let body = self.parse_block()?;
            Some(CatchClause {
                param,
                body,
                span: self.span_from(h_start),
            })
        } else {
            None
        };

        let finalizer = if self.at(TokenKind::Finally) {
            self.advance()?;
            Some(self.parse_block()?)
        } else {
            None
        };

        if handler.is_none() && finalizer.is_none() {
            return Err(ParseError::InvalidContext {
                msg: "try statement must have catch or finally".into(),
                span: self.span_from(start),
            });
        }

        Ok(Stmt::Try(TryStmt {
            block,
            handler,
            finalizer,
            span: self.span_from(start),
        }))
    }

    // ── with ──────────────────────────────────────────────────────────────────

    fn parse_with(&mut self) -> ParseResult<Stmt> {
        let start = self.start();
        self.expect(TokenKind::With)?;
        self.expect(TokenKind::LParen)?;
        let object = self.parse_expr(0)?;
        self.expect(TokenKind::RParen)?;
        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt::With(WithStmt {
            object,
            body,
            span: self.span_from(start),
        }))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Declarations
    // ─────────────────────────────────────────────────────────────────────────

    /// `var x = 1, y = 2;`
    pub(crate) fn parse_var_decl(
        &mut self,
        kind: VarKind,
        consume_semi: bool,
    ) -> ParseResult<VarDecl> {
        let start = self.start();
        self.advance()?; // consume var/let/const
        let mut decls = Vec::new();
        loop {
            let d_start = self.start();
            let id = self.parse_binding_pat()?;
            let init = if self.eat(TokenKind::Eq)? {
                Some(self.parse_assign_expr()?)
            } else {
                None
            };
            decls.push(VarDeclarator {
                id,
                init,
                span: self.span_from(d_start),
            });
            if !self.eat(TokenKind::Comma)? {
                break;
            }
        }
        if consume_semi {
            self.expect_semi()?;
        }
        Ok(VarDecl {
            kind,
            decls,
            span: self.span_from(start),
        })
    }

    /// `function [*] [name] (params) { body }`
    pub(crate) fn parse_fn_decl(
        &mut self,
        is_async: bool,
        allow_missing_id: bool,
    ) -> ParseResult<FnDecl> {
        let start = self.start();
        if is_async {
            self.advance()?;
        } // consume `async`
        self.expect(TokenKind::Function)?;
        let is_generator = self.eat(TokenKind::Star)?;
        let id = if matches!(
            self.cur.kind,
            TokenKind::Ident(_)
                | TokenKind::Let
                | TokenKind::Static
                | TokenKind::Async
                | TokenKind::Yield
                | TokenKind::Await
        ) {
            Some(self.expect_ident()?)
        } else if allow_missing_id {
            None
        } else {
            return Err(ParseError::Expected {
                expected: TokenKind::Ident(String::new()),
                found: self.cur.kind.clone(),
                span: self.cur.span,
            });
        };
        let function = self.parse_function(is_async, is_generator)?;
        Ok(FnDecl {
            id,
            function,
            span: self.span_from(start),
        })
    }

    /// Parse `(params) { body }` — the parenthesised parameter list and block.
    fn parse_function(&mut self, is_async: bool, is_generator: bool) -> ParseResult<Function> {
        let start = self.start();
        let saved = (self.in_function, self.in_async, self.in_generator);
        self.in_function = true;
        self.in_async = is_async;
        self.in_generator = is_generator;

        let params = self.parse_params()?;
        let body = self.parse_block()?;

        self.in_function = saved.0;
        self.in_async = saved.1;
        self.in_generator = saved.2;

        Ok(Function {
            params,
            body,
            is_async,
            is_generator,
            span: self.span_from(start),
        })
    }

    fn parse_params(&mut self) -> ParseResult<Vec<Param>> {
        self.expect(TokenKind::LParen)?;
        let mut params = Vec::new();
        while !self.at(TokenKind::RParen) && !self.at(TokenKind::Eof) {
            let p_start = self.start();
            if self.at(TokenKind::Spread) {
                self.advance()?;
                let arg = self.parse_binding_pat()?;
                params.push(Param {
                    pat: Pat::Rest(RestPat {
                        argument: Box::new(arg),
                        span: self.span_from(p_start),
                    }),
                    span: self.span_from(p_start),
                });
                self.eat(TokenKind::Comma)?; // trailing comma after rest
                break;
            }
            let pat = self.parse_binding_pat()?;
            let pat = if self.eat(TokenKind::Eq)? {
                let right = self.parse_assign_expr()?;
                let span = self.span_from(p_start);
                Pat::Assign(AssignPat {
                    left: Box::new(pat),
                    right: Box::new(right),
                    span,
                })
            } else {
                pat
            };
            params.push(Param {
                span: pat.span(),
                pat,
            });
            if !self.eat(TokenKind::Comma)? {
                break;
            }
        }
        self.expect(TokenKind::RParen)?;
        Ok(params)
    }

    /// `class [Name] [extends Expr] { body }`
    pub(crate) fn parse_class_decl(&mut self, allow_missing_id: bool) -> ParseResult<ClassDecl> {
        let start = self.start();
        self.expect(TokenKind::Class)?;
        let id = if matches!(
            self.cur.kind,
            TokenKind::Ident(_) | TokenKind::Let | TokenKind::Static | TokenKind::Async
        ) {
            Some(self.expect_ident()?)
        } else if allow_missing_id {
            None
        } else {
            return Err(ParseError::Expected {
                expected: TokenKind::Ident(String::new()),
                found: self.cur.kind.clone(),
                span: self.cur.span,
            });
        };

        let super_class = if self.eat(TokenKind::Extends)? {
            Some(Box::new(self.parse_expr(35)?)) // high bp — no infix in extends clause
        } else {
            None
        };

        let body = self.parse_class_body()?;
        Ok(ClassDecl {
            id,
            super_class,
            body,
            span: self.span_from(start),
        })
    }

    fn parse_class_body(&mut self) -> ParseResult<ClassBody> {
        let start = self.start();
        self.expect(TokenKind::LBrace)?;
        let saved_in_class = self.in_class;
        self.in_class = true;
        let mut body = Vec::new();

        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            if self.eat(TokenKind::Semi)? {
                continue;
            }
            body.push(self.parse_class_member()?);
        }

        self.in_class = saved_in_class;
        let end = self.cur.span.end;
        self.expect(TokenKind::RBrace)?;
        Ok(ClassBody {
            body,
            span: Span::new(start, end),
        })
    }

    fn parse_class_member(&mut self) -> ParseResult<ClassMember> {
        let start = self.start();
        let is_static =
            if self.at(TokenKind::Static) && !matches!(self.peek.kind, TokenKind::LParen) {
                // static { } block
                if self.peek.kind == TokenKind::LBrace {
                    self.advance()?; // consume `static`
                    self.advance()?; // consume `{`
                    let mut block_body = Vec::new();
                    while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
                        block_body.push(self.parse_stmt()?);
                    }
                    self.expect(TokenKind::RBrace)?;
                    return Ok(ClassMember::StaticBlock(StaticBlock {
                        body: block_body,
                        span: self.span_from(start),
                    }));
                }
                self.advance()?;
                true
            } else {
                false
            };

        // get / set accessor
        let method_kind = if self.at(TokenKind::Get)
            && !matches!(
                self.peek.kind,
                TokenKind::LParen | TokenKind::Eq | TokenKind::Semi
            ) {
            self.advance()?;
            MethodKind::Get
        } else if self.at(TokenKind::Set)
            && !matches!(
                self.peek.kind,
                TokenKind::LParen | TokenKind::Eq | TokenKind::Semi
            )
        {
            self.advance()?;
            MethodKind::Set
        } else {
            MethodKind::Method
        };

        let is_async = if method_kind == MethodKind::Method
            && self.at(TokenKind::Async)
            && !self.peek.preceded_by_newline
            && !matches!(
                self.peek.kind,
                TokenKind::LParen | TokenKind::Eq | TokenKind::Semi
            ) {
            self.advance()?;
            true
        } else {
            false
        };

        let is_generator = is_async && self.eat(TokenKind::Star)?
            || (method_kind == MethodKind::Method && self.eat(TokenKind::Star)?);

        let (key, computed) = self.parse_class_element_name()?;

        // Field declaration (no `(` follows)
        if !self.at(TokenKind::LParen) {
            let value = if self.eat(TokenKind::Eq)? {
                Some(self.parse_assign_expr()?)
            } else {
                None
            };
            self.expect_semi()?;
            return Ok(ClassMember::Field(ClassField {
                key,
                value,
                computed,
                is_static,
                span: self.span_from(start),
            }));
        }

        // Method
        let real_kind = if is_generator {
            MethodKind::Method
        } else {
            method_kind
        };
        let function = self.parse_function(is_async, is_generator)?;
        Ok(ClassMember::Method(ClassMethod {
            key,
            value: function,
            kind: real_kind,
            computed,
            is_static,
            span: self.span_from(start),
        }))
    }

    fn parse_class_element_name(&mut self) -> ParseResult<(PropKey, bool)> {
        if self.at(TokenKind::LBracket) {
            self.advance()?;
            let expr = self.parse_assign_expr()?;
            self.expect(TokenKind::RBracket)?;
            return Ok((PropKey::Computed(Box::new(expr)), true));
        }
        if self.at(TokenKind::Hash) {
            let start = self.start();
            self.advance()?;
            let name = self.expect_ident()?;
            return Ok((
                PropKey::Private(Ident::new(name.name, Span::new(start, name.span.end))),
                false,
            ));
        }
        let key = self.parse_prop_key_static()?;
        Ok((key, false))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Module declarations
    // ─────────────────────────────────────────────────────────────────────────

    fn parse_import_decl(&mut self) -> ParseResult<ImportDecl> {
        let start = self.start();
        self.expect(TokenKind::Import)?;

        // import "source"  (side-effect only)
        if let TokenKind::Str(s) = &self.cur.kind.clone() {
            let source = s.clone();
            self.advance()?;
            self.expect_semi()?;
            return Ok(ImportDecl {
                specifiers: vec![],
                source,
                span: self.span_from(start),
            });
        }

        let mut specifiers = Vec::new();

        // import defaultName ...
        if matches!(
            self.cur.kind,
            TokenKind::Ident(_) | TokenKind::Let | TokenKind::Static | TokenKind::Async
        ) {
            let local = self.expect_ident()?;
            let spec_span = local.span;
            specifiers.push(ImportSpec::Default(ImportDefaultSpec {
                local,
                span: spec_span,
            }));
            if !self.eat(TokenKind::Comma)? {
                let source = self.parse_from_clause()?;
                self.expect_semi()?;
                return Ok(ImportDecl {
                    specifiers,
                    source,
                    span: self.span_from(start),
                });
            }
        }

        // import * as ns
        if self.at(TokenKind::Star) {
            let ns_start = self.start();
            self.advance()?;
            self.expect(TokenKind::As)?;
            let local = self.expect_ident()?;
            specifiers.push(ImportSpec::Namespace(ImportNamespaceSpec {
                local,
                span: self.span_from(ns_start),
            }));
        }
        // import { a, b as c }
        else if self.at(TokenKind::LBrace) {
            self.advance()?;
            while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
                let spec_start = self.start();
                let imported = self.parse_module_export_name()?;
                let local = if self.eat(TokenKind::As)? {
                    self.expect_ident()?
                } else {
                    match &imported {
                        ModuleExportName::Ident(i) => i.clone(),
                        _ => {
                            return Err(ParseError::InvalidContext {
                                msg: "string export name requires 'as'".into(),
                                span: self.cur.span,
                            });
                        }
                    }
                };
                specifiers.push(ImportSpec::Named(ImportNamedSpec {
                    imported,
                    local,
                    span: self.span_from(spec_start),
                }));
                if !self.eat(TokenKind::Comma)? {
                    break;
                }
            }
            self.expect(TokenKind::RBrace)?;
        }

        let source = self.parse_from_clause()?;
        self.expect_semi()?;
        Ok(ImportDecl {
            specifiers,
            source,
            span: self.span_from(start),
        })
    }

    fn parse_from_clause(&mut self) -> ParseResult<String> {
        self.expect(TokenKind::From)?;
        match self.cur.kind.clone() {
            TokenKind::Str(s) => {
                self.advance()?;
                Ok(s)
            }
            _ => Err(ParseError::Expected {
                expected: TokenKind::Str(String::new()),
                found: self.cur.kind.clone(),
                span: self.cur.span,
            }),
        }
    }

    fn parse_module_export_name(&mut self) -> ParseResult<ModuleExportName> {
        match self.cur.kind.clone() {
            TokenKind::Str(s) => {
                let span = self.cur.span;
                self.advance()?;
                Ok(ModuleExportName::Str(s, span))
            }
            _ => Ok(ModuleExportName::Ident(self.expect_ident()?)),
        }
    }

    fn parse_export_decl(&mut self) -> ParseResult<ExportDecl> {
        let start = self.start();
        self.expect(TokenKind::Export)?;

        // export default ...
        if self.at(TokenKind::Default) {
            self.advance()?;
            let decl_start = self.start();
            let declaration = if self.at(TokenKind::Function)
                || (self.at(TokenKind::Async) && matches!(self.peek.kind, TokenKind::Function))
            {
                let is_async = self.eat(TokenKind::Async)?;
                ExportDefault::Fn(self.parse_fn_decl(is_async, true)?)
            } else if self.at(TokenKind::Class) {
                ExportDefault::Class(self.parse_class_decl(true)?)
            } else {
                ExportDefault::Expr(self.parse_assign_expr()?)
            };
            self.expect_semi()?;
            return Ok(ExportDecl::Default(ExportDefaultDecl {
                declaration,
                span: self.span_from(start),
            }));
        }

        // export * from "..."   or   export * as ns from "..."
        if self.at(TokenKind::Star) {
            self.advance()?;
            let exported = if self.eat(TokenKind::As)? {
                Some(self.parse_module_export_name()?)
            } else {
                None
            };
            let source = self.parse_from_clause()?;
            self.expect_semi()?;
            return Ok(ExportDecl::All(ExportAllDecl {
                exported,
                source,
                span: self.span_from(start),
            }));
        }

        // export { a, b as c } [from "..."]
        if self.at(TokenKind::LBrace) {
            self.advance()?;
            let mut specifiers = Vec::new();
            while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
                let sp_start = self.start();
                let local = self.parse_module_export_name()?;
                let exported = if self.eat(TokenKind::As)? {
                    self.parse_module_export_name()?
                } else {
                    local.clone()
                };
                specifiers.push(ExportSpec {
                    local,
                    exported,
                    span: self.span_from(sp_start),
                });
                if !self.eat(TokenKind::Comma)? {
                    break;
                }
            }
            self.expect(TokenKind::RBrace)?;
            let source = if self.at(TokenKind::From) {
                Some(self.parse_from_clause()?)
            } else {
                None
            };
            self.expect_semi()?;
            return Ok(ExportDecl::Named(ExportNamedDecl {
                specifiers,
                source,
                span: self.span_from(start),
            }));
        }

        // export var/let/const/function/class declaration
        let decl = match &self.cur.kind.clone() {
            TokenKind::Var => Decl::Var(self.parse_var_decl(VarKind::Var, true)?),
            TokenKind::Const => Decl::Var(self.parse_var_decl(VarKind::Const, true)?),
            _ if self.is_let_decl() => Decl::Var(self.parse_var_decl(VarKind::Let, true)?),
            TokenKind::Function => Decl::Fn(self.parse_fn_decl(false, false)?),
            TokenKind::Async if matches!(self.peek.kind, TokenKind::Function) => {
                Decl::Fn(self.parse_fn_decl(true, false)?)
            }
            TokenKind::Class => Decl::Class(self.parse_class_decl(false)?),
            _ => {
                return Err(ParseError::Unexpected {
                    found: self.cur.kind.clone(),
                    span: self.cur.span,
                    hint: "expected declaration after export".into(),
                });
            }
        };
        let span = self.span_from(start);
        Ok(ExportDecl::Decl(decl, span))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Binding patterns
    // ─────────────────────────────────────────────────────────────────────────

    /// Parse any binding pattern: identifier, array destructuring, or object
    /// destructuring.
    pub(crate) fn parse_binding_pat(&mut self) -> ParseResult<Pat> {
        match &self.cur.kind.clone() {
            TokenKind::LBracket => self.parse_array_binding_pat(),
            TokenKind::LBrace => self.parse_object_binding_pat(),
            _ => Ok(Pat::Ident(self.expect_ident()?)),
        }
    }

    fn parse_array_binding_pat(&mut self) -> ParseResult<Pat> {
        let start = self.start();
        self.expect(TokenKind::LBracket)?;
        let mut elements = Vec::new();
        while !self.at(TokenKind::RBracket) && !self.at(TokenKind::Eof) {
            if self.at(TokenKind::Comma) {
                elements.push(None);
                self.advance()?;
                continue;
            }
            if self.at(TokenKind::Spread) {
                let r_start = self.start();
                self.advance()?;
                let arg = self.parse_binding_pat()?;
                elements.push(Some(Pat::Rest(RestPat {
                    argument: Box::new(arg),
                    span: self.span_from(r_start),
                })));
                self.eat(TokenKind::Comma)?;
                break;
            }
            let pat = self.parse_binding_pat()?;
            let pat = if self.eat(TokenKind::Eq)? {
                let right = self.parse_assign_expr()?;
                let span = pat.span().merge(right.span());
                Pat::Assign(AssignPat {
                    left: Box::new(pat),
                    right: Box::new(right),
                    span,
                })
            } else {
                pat
            };
            elements.push(Some(pat));
            if !self.eat(TokenKind::Comma)? {
                break;
            }
        }
        self.expect(TokenKind::RBracket)?;
        Ok(Pat::Array(ArrayPat {
            elements,
            span: self.span_from(start),
        }))
    }

    fn parse_object_binding_pat(&mut self) -> ParseResult<Pat> {
        let start = self.start();
        self.expect(TokenKind::LBrace)?;
        let mut props = Vec::new();
        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            if self.at(TokenKind::Spread) {
                let r_start = self.start();
                self.advance()?;
                let ident = self.expect_ident()?;
                props.push(ObjectPatProp::Rest(RestPat {
                    argument: Box::new(Pat::Ident(ident)),
                    span: self.span_from(r_start),
                }));
                self.eat(TokenKind::Comma)?;
                break;
            }
            let prop_start = self.start();
            let computed = self.at(TokenKind::LBracket);
            let key = if computed {
                self.advance()?;
                let e = self.parse_assign_expr()?;
                self.expect(TokenKind::RBracket)?;
                PropKey::Computed(Box::new(e))
            } else {
                self.parse_prop_key_static()?
            };
            let (value, shorthand) = if self.eat(TokenKind::Colon)? {
                let pat = self.parse_binding_pat()?;
                let pat = if self.eat(TokenKind::Eq)? {
                    let r = self.parse_assign_expr()?;
                    let sp = pat.span().merge(r.span());
                    Pat::Assign(AssignPat {
                        left: Box::new(pat),
                        right: Box::new(r),
                        span: sp,
                    })
                } else {
                    pat
                };
                (pat, false)
            } else {
                let name = match &key {
                    PropKey::Ident(i) => Pat::Ident(i.clone()),
                    _ => {
                        return Err(ParseError::InvalidContext {
                            msg: "shorthand property must be an identifier".into(),
                            span: self.cur.span,
                        });
                    }
                };
                let name = if self.eat(TokenKind::Eq)? {
                    let r = self.parse_assign_expr()?;
                    let sp = name.span().merge(r.span());
                    Pat::Assign(AssignPat {
                        left: Box::new(name),
                        right: Box::new(r),
                        span: sp,
                    })
                } else {
                    name
                };
                (name, true)
            };
            props.push(ObjectPatProp::Keyed(KeyedPatProp {
                key,
                value,
                computed,
                shorthand,
                span: self.span_from(prop_start),
            }));
            if !self.eat(TokenKind::Comma)? {
                break;
            }
        }
        self.expect(TokenKind::RBrace)?;
        Ok(Pat::Object(ObjectPat {
            props,
            span: self.span_from(start),
        }))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Helper utilities
    // ─────────────────────────────────────────────────────────────────────────

    /// Property key that cannot be a computed expression (used in class names,
    /// object literal static keys). Returns `PropKey`.
    pub(crate) fn parse_prop_key_static(&mut self) -> ParseResult<PropKey> {
        match self.cur.kind.clone() {
            TokenKind::Number(n) => {
                let span = self.cur.span;
                self.advance()?;
                Ok(PropKey::Number(n, span))
            }
            TokenKind::Str(s) => {
                let span = self.cur.span;
                self.advance()?;
                Ok(PropKey::Str(s, span))
            }
            _ => Ok(PropKey::Ident(self.expect_ident()?)),
        }
    }

    /// Convert an expression to a binding pattern (for destructuring assignment).
    pub(crate) fn expr_to_pat(&mut self, expr: Expr) -> ParseResult<Pat> {
        match expr {
            Expr::Ident(i) => Ok(Pat::Ident(i)),
            Expr::Array(a) => {
                let mut elements = Vec::new();
                for elem in a.elements {
                    elements.push(match elem {
                        None => None,
                        Some(Expr::Spread(s)) => Some(Pat::Rest(RestPat {
                            argument: Box::new(self.expr_to_pat(*s.argument)?),
                            span: s.span,
                        })),
                        Some(Expr::Assign(ae)) => {
                            let left = self.expr_to_pat(match ae.left {
                                AssignTarget::Simple(e) => *e,
                                AssignTarget::Destructure(p) => {
                                    return Ok(Pat::Assign(AssignPat {
                                        left: Box::new(p),
                                        right: ae.right,
                                        span: ae.span,
                                    }));
                                }
                            })?;
                            Some(Pat::Assign(AssignPat {
                                left: Box::new(left),
                                right: ae.right,
                                span: ae.span,
                            }))
                        }
                        Some(e) => Some(self.expr_to_pat(e)?),
                    });
                }
                Ok(Pat::Array(ArrayPat {
                    elements,
                    span: a.span,
                }))
            }
            Expr::Object(o) => {
                let mut props = Vec::new();
                for prop in o.props {
                    match prop {
                        ObjectProp::Spread(s) => props.push(ObjectPatProp::Rest(RestPat {
                            argument: Box::new(self.expr_to_pat(*s.argument)?),
                            span: s.span,
                        })),
                        ObjectProp::Keyed(k) => {
                            let value = self.expr_to_pat(k.value)?;
                            props.push(ObjectPatProp::Keyed(KeyedPatProp {
                                key: k.key,
                                value,
                                computed: k.computed,
                                shorthand: k.shorthand,
                                span: k.span,
                            }));
                        }
                        ObjectProp::Method(_) => {
                            return Err(ParseError::InvalidDestructuring(o.span));
                        }
                    }
                }
                Ok(Pat::Object(ObjectPat {
                    props,
                    span: o.span,
                }))
            }
            Expr::Assign(ae) => {
                let left = self.expr_to_pat(match ae.left {
                    AssignTarget::Simple(e) => *e,
                    AssignTarget::Destructure(p) => {
                        return Ok(Pat::Assign(AssignPat {
                            left: Box::new(p),
                            right: ae.right,
                            span: ae.span,
                        }));
                    }
                })?;
                Ok(Pat::Assign(AssignPat {
                    left: Box::new(left),
                    right: ae.right,
                    span: ae.span,
                }))
            }
            other => Err(ParseError::InvalidDestructuring(other.span())),
        }
    }

    /// Parse an assignment-level expression (no comma operator at top level).
    pub(crate) fn parse_assign_expr(&mut self) -> ParseResult<Expr> {
        self.parse_expr(2)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Small helpers
// ─────────────────────────────────────────────────────────────────────────────

fn eof(pos: u32) -> Token {
    Token::new(TokenKind::Eof, Span::new(pos, pos), false)
}
