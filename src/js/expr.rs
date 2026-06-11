//! Expression parsing — Pratt / top-down operator-precedence algorithm.
//!
//! ## Binding-power table
//!
//! | Operator(s)                       | Left BP | Right BP | Assoc |
//! |-----------------------------------|---------|----------|-------|
//! | `,` (sequence)                    |    1    |    2     | left  |
//! | `=`, `+=`, … (assignment)         |    4    |    3     | right |
//! | `?:` (conditional)                |    5    |   (3)    | right |
//! | `??` (nullish)                    |    6    |    7     | left  |
//! | `\|\|`                            |    8    |    9     | left  |
//! | `&&`                              |   10    |   11     | left  |
//! | `\|`                              |   12    |   13     | left  |
//! | `^`                               |   14    |   15     | left  |
//! | `&`                               |   16    |   17     | left  |
//! | `==` `!=` `===` `!==`            |   18    |   19     | left  |
//! | `<` `<=` `>` `>=` `in` `instof`  |   20    |   21     | left  |
//! | `<<` `>>` `>>>`                   |   22    |   23     | left  |
//! | `+` `-`                           |   24    |   25     | left  |
//! | `*` `/` `%`                       |   26    |   27     | left  |
//! | `**`                              |   28    |   27     | right |
//! | Unary prefix                      |   —     |   29     | —     |
//! | Postfix `++` `--`                 |   30    |   —      | —     |
//! | `new` / call / member             |   35    |   —      | —     |

use super::Parser;

#[cfg(feature = "alloc")]
use alloc::{boxed::Box, string::String, string::ToString, vec, vec::Vec};

use crate::{
    ast::*,
    error::{ParseError, ParseResult},
    span::Span,
    token::TokenKind,
};

impl<'src> Parser<'src> {
    // ─────────────────────────────────────────────────────────────────────────
    // Core Pratt loop
    // ─────────────────────────────────────────────────────────────────────────

    /// Parse an expression with minimum binding power `min_bp`.
    ///
    /// - `min_bp = 0`  → full expression including the comma operator
    /// - `min_bp = 2`  → assignment-level (no comma)
    /// - Higher values → sub-expression for specific precedence levels
    pub fn parse_expr(&mut self, min_bp: u8) -> ParseResult<Expr> {
        let mut lhs = self.parse_prefix_expr()?;

        loop {
            // ── Postfix ++ / --  (no line-break allowed before) ───────────────
            if matches!(self.cur.kind, TokenKind::PlusPlus | TokenKind::MinusMinus)
                && !self.cur.preceded_by_newline
            {
                if 30 < min_bp { break; }
                let op = if self.cur.kind == TokenKind::PlusPlus { UpdateOp::Inc } else { UpdateOp::Dec };
                let start = lhs.span().start;
                let end   = self.cur.span.end;
                self.advance()?;
                lhs = Expr::Update(UpdateExpr {
                    op, argument: Box::new(lhs), prefix: false,
                    span: Span::new(start, end),
                });
                continue;
            }

            // ── Member access: `.prop` or `.#private`  ────────────────────────
            if self.cur.kind == TokenKind::Dot {
                if 37 < min_bp { break; }
                let start = lhs.span().start;
                self.advance()?;
                // Private member: `obj.#field`
                let prop = if self.cur.kind == TokenKind::Hash {
                    self.advance()?; // consume `#`
                    let name = self.expect_ident()?;
                    MemberProp::Private(name)
                } else {
                    MemberProp::Ident(self.expect_ident()?)
                };
                let span = self.span_from(start);
                lhs = Expr::Member(MemberExpr { object: Box::new(lhs), prop, computed: false, span });
                continue;
            }

            // ── Member access: `[expr]`  ───────────────────────────────────────
            if self.cur.kind == TokenKind::LBracket {
                if 37 < min_bp { break; }
                let start = lhs.span().start;
                self.advance()?;
                let key = self.parse_expr(0)?;
                let end = self.cur.span.end;
                self.expect(TokenKind::RBracket)?;
                lhs = Expr::Member(MemberExpr {
                    object: Box::new(lhs),
                    prop: MemberProp::Computed(Box::new(key)),
                    computed: true,
                    span: Span::new(start, end),
                });
                continue;
            }

            // ── Optional chaining: `?.`  ──────────────────────────────────────
            if self.cur.kind == TokenKind::QuestionDot {
                if 37 < min_bp { break; }
                let start = lhs.span().start;
                self.advance()?;
                let chain = if self.cur.kind == TokenKind::LBracket {
                    self.advance()?;
                    let key = self.parse_expr(0)?;
                    self.expect(TokenKind::RBracket)?;
                    OptChain::ComputedMember(Box::new(key))
                } else if self.cur.kind == TokenKind::LParen {
                    let args = self.parse_call_args()?;
                    OptChain::Call(args)
                } else {
                    OptChain::Member(self.expect_ident()?)
                };
                lhs = Expr::OptChain(OptChainExpr {
                    base: Box::new(lhs),
                    chain,
                    span: self.span_from(start),
                });
                continue;
            }

            // ── Call: `(args)`  ───────────────────────────────────────────────
            if self.cur.kind == TokenKind::LParen {
                if 35 < min_bp { break; }
                let start = lhs.span().start;
                let args = self.parse_call_args()?;
                lhs = Expr::Call(CallExpr {
                    callee: Box::new(lhs),
                    args,
                    span: self.span_from(start),
                });
                continue;
            }

            // ── Tagged template  ──────────────────────────────────────────────
            if matches!(self.cur.kind, TokenKind::TemplateHead(_) | TokenKind::TemplateNoSub(_)) {
                if 35 < min_bp { break; }
                let start = lhs.span().start;
                let quasi = self.parse_template_literal()?;
                lhs = Expr::TaggedTpl(TaggedTplExpr {
                    tag: Box::new(lhs),
                    quasi,
                    span: self.span_from(start),
                });
                continue;
            }

            // ── Conditional  ─────────────────────────────────────────────────
            if self.cur.kind == TokenKind::Question {
                if 5 < min_bp { break; }
                let start = lhs.span().start;
                self.advance()?; // consume `?`
                let consequent = self.parse_expr(2)?; // AssignmentExpression level
                self.expect(TokenKind::Colon)?;
                let alternate = self.parse_expr(3)?;  // right-assoc: lower than assignment
                lhs = Expr::Cond(CondExpr {
                    test: Box::new(lhs),
                    consequent: Box::new(consequent),
                    alternate: Box::new(alternate),
                    span: self.span_from(start),
                });
                continue;
            }

            // ── Infix binary / logical / assignment  ───────────────────────────
            let Some((l_bp, r_bp)) = self.infix_bp() else { break };
            if l_bp < min_bp { break; }

            let op_kind = self.cur.kind.clone();
            let op_span  = self.cur.span;
            self.advance()?;

            // Special rule: `in` disallowed in for-loop headers
            if op_kind == TokenKind::In && !self.allow_in {
                return Err(ParseError::InvalidContext {
                    msg: "'in' not allowed here".into(),
                    span: op_span,
                });
            }

            let rhs = self.parse_expr(r_bp)?;
            lhs = self.make_binary(op_kind, lhs, rhs, op_span)?;
        }

        Ok(lhs)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Prefix / primary expressions
    // ─────────────────────────────────────────────────────────────────────────

    fn parse_prefix_expr(&mut self) -> ParseResult<Expr> {
        let start = self.start();

        match self.cur.kind.clone() {
            // ── Literals ──────────────────────────────────────────────────────
            TokenKind::Number(n) => {
                let span = self.cur.span;
                self.advance()?;
                Ok(Expr::Lit(Lit::Number(n), span))
            }
            TokenKind::BigInt(s) => {
                let span = self.cur.span;
                self.advance()?;
                Ok(Expr::Lit(Lit::BigInt(s), span))
            }
            TokenKind::Str(s) => {
                let span = self.cur.span;
                self.advance()?;
                Ok(Expr::Lit(Lit::Str(s), span))
            }
            TokenKind::True => {
                let span = self.cur.span;
                self.advance()?;
                Ok(Expr::Lit(Lit::Bool(true), span))
            }
            TokenKind::False => {
                let span = self.cur.span;
                self.advance()?;
                Ok(Expr::Lit(Lit::Bool(false), span))
            }
            TokenKind::Null => {
                let span = self.cur.span;
                self.advance()?;
                Ok(Expr::Lit(Lit::Null, span))
            }
            TokenKind::Regex { pattern, flags } => {
                let span = self.cur.span;
                self.advance()?;
                Ok(Expr::Lit(Lit::Regex { pattern, flags }, span))
            }

            // ── Template literal  ─────────────────────────────────────────────
            TokenKind::TemplateNoSub(_) | TokenKind::TemplateHead(_) => {
                let tpl = self.parse_template_literal()?;
                Ok(Expr::Tpl(tpl))
            }

            // ── this / super  ─────────────────────────────────────────────────
            TokenKind::This => {
                let span = self.cur.span;
                self.advance()?;
                Ok(Expr::This(span))
            }
            TokenKind::Super => {
                let span = self.cur.span;
                self.advance()?;
                Ok(Expr::Ident(Ident::new("super", span)))
            }

            // ── Identifier  ───────────────────────────────────────────────────
            TokenKind::Ident(_) | TokenKind::Let | TokenKind::Static
            | TokenKind::From | TokenKind::As | TokenKind::Of
            | TokenKind::Get | TokenKind::Set | TokenKind::Target | TokenKind::Meta => {
                let ident = self.expect_ident()?;
                // Single-parameter arrow: `x => body`  (no parentheses)
                if self.cur.kind == TokenKind::Arrow && !self.cur.preceded_by_newline {
                    let arrow_start = ident.span.start;
                    let param_span  = ident.span;
                    self.advance()?; // consume `=>`
                    let params = vec![Param { pat: Pat::Ident(ident), span: param_span }];
                    return self.finish_arrow(params, false, arrow_start);
                }
                Ok(Expr::Ident(ident))
            }

            // ── Unary prefix operators  ───────────────────────────────────────
            TokenKind::Bang | TokenKind::Tilde | TokenKind::Plus | TokenKind::Minus => {
                let op = token_to_unary_op(&self.cur.kind).unwrap();
                self.advance()?;
                let arg = self.parse_expr(29)?;
                Ok(Expr::Unary(UnaryExpr {
                    op, argument: Box::new(arg), span: self.span_from(start),
                }))
            }
            TokenKind::Typeof => {
                self.advance()?;
                let arg = self.parse_expr(29)?;
                Ok(Expr::Unary(UnaryExpr { op: UnaryOp::Typeof, argument: Box::new(arg), span: self.span_from(start) }))
            }
            TokenKind::Void => {
                self.advance()?;
                let arg = self.parse_expr(29)?;
                Ok(Expr::Unary(UnaryExpr { op: UnaryOp::Void, argument: Box::new(arg), span: self.span_from(start) }))
            }
            TokenKind::Delete => {
                self.advance()?;
                let arg = self.parse_expr(29)?;
                Ok(Expr::Unary(UnaryExpr { op: UnaryOp::Delete, argument: Box::new(arg), span: self.span_from(start) }))
            }

            // ── Prefix ++ / --  ───────────────────────────────────────────────
            TokenKind::PlusPlus | TokenKind::MinusMinus => {
                let op = if self.cur.kind == TokenKind::PlusPlus { UpdateOp::Inc } else { UpdateOp::Dec };
                self.advance()?;
                let arg = self.parse_expr(29)?;
                if !arg.is_lvalue() {
                    return Err(ParseError::InvalidContext {
                        msg: "invalid left-hand side in prefix operation".into(),
                        span: arg.span(),
                    });
                }
                Ok(Expr::Update(UpdateExpr { op, argument: Box::new(arg), prefix: true, span: self.span_from(start) }))
            }

            // ── Yield expression  ─────────────────────────────────────────────
            TokenKind::Yield if self.in_generator => {
                self.advance()?;
                let delegate = self.eat(TokenKind::Star)?;
                let argument = if !self.cur.preceded_by_newline
                    && !matches!(self.cur.kind,
                        TokenKind::Semi | TokenKind::RBrace | TokenKind::RBracket
                        | TokenKind::RParen | TokenKind::Colon | TokenKind::Comma | TokenKind::Eof)
                {
                    Some(Box::new(self.parse_expr(2)?))
                } else {
                    None
                };
                Ok(Expr::Yield(YieldExpr { argument, delegate, span: self.span_from(start) }))
            }

            // ── Await expression  ─────────────────────────────────────────────
            TokenKind::Await if self.in_async => {
                self.advance()?;
                let argument = Box::new(self.parse_expr(29)?);
                Ok(Expr::Await(AwaitExpr { argument, span: self.span_from(start) }))
            }

            // ── new  ──────────────────────────────────────────────────────────
            TokenKind::New => {
                self.parse_new_expr()
            }

            // ── import() / import.meta  ───────────────────────────────────────
            TokenKind::Import => {
                self.advance()?;
                if self.at(TokenKind::Dot) {
                    self.advance()?;
                    // import.meta
                    if matches!(self.cur.kind, TokenKind::Meta) {
                        self.advance()?;
                        return Ok(Expr::ImportMeta(self.span_from(start)));
                    }
                    return Err(ParseError::InvalidContext {
                        msg: "expected 'meta' after 'import.'".into(),
                        span: self.cur.span,
                    });
                }
                // import(source) — dynamic import
                self.expect(TokenKind::LParen)?;
                let source = self.parse_assign_expr()?;
                self.expect(TokenKind::RParen)?;
                Ok(Expr::Import(Box::new(source), self.span_from(start)))
            }

            // ── Parenthesised expression or arrow function  ───────────────────
            TokenKind::LParen => {
                self.parse_paren_or_arrow(false)
            }

            // ── Array literal  ────────────────────────────────────────────────
            TokenKind::LBracket => {
                self.parse_array_expr()
            }

            // ── Object literal  ───────────────────────────────────────────────
            TokenKind::LBrace => {
                self.parse_object_expr()
            }

            // ── Function expression  ──────────────────────────────────────────
            TokenKind::Function => {
                self.parse_function_expr(false)
            }

            // ── async function / async arrow  ─────────────────────────────────
            TokenKind::Async => {
                self.parse_async_expr()
            }

            // ── Class expression  ─────────────────────────────────────────────
            TokenKind::Class => {
                self.parse_class_expr()
            }

            // ── Spread (valid in call args, arrays, objects — not top-level)  ─
            TokenKind::Spread => {
                let start = self.start();
                self.advance()?;
                let arg = self.parse_assign_expr()?;
                Ok(Expr::Spread(SpreadExpr { argument: Box::new(arg), span: self.span_from(start) }))
            }

            _ => Err(ParseError::Unexpected {
                found: self.cur.kind.clone(),
                span:  self.cur.span,
                hint:  "expected expression".into(),
            }),
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Infix binding powers
    // ─────────────────────────────────────────────────────────────────────────

    fn infix_bp(&self) -> Option<(u8, u8)> {
        use TokenKind::*;
        // Comma — sequence operator (only at min_bp=0 level, skipped for min_bp>=2)
        Some(match &self.cur.kind {
            Comma             => (1, 2),
            // Assignment (right-assoc): l > r
            k if k.is_assignment_op() => (4, 3),
            // Nullish / logical (left-assoc): r > l
            QuestionQuestion  => (6, 7),
            PipePipe          => (8, 9),
            AmpAmp            => (10, 11),
            Pipe              => (12, 13),
            Caret             => (14, 15),
            Amp               => (16, 17),
            EqEq | BangEq | EqEqEq | BangEqEq => (18, 19),
            Lt | LtEq | Gt | GtEq | In | Instanceof => (20, 21),
            LtLt | GtGt | GtGtGt => (22, 23),
            Plus | Minus      => (24, 25),
            Star | Slash | Percent => (26, 27),
            // Exponentiation (right-assoc): l > r
            StarStar          => (28, 27),
            _ => return None,
        })
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Binary / assignment node construction
    // ─────────────────────────────────────────────────────────────────────────

    fn make_binary(
        &mut self,
        op: TokenKind,
        lhs: Expr,
        rhs: Expr,
        _op_span: Span,
    ) -> ParseResult<Expr> {
        let span = lhs.span().merge(rhs.span());

        // Assignment
        if op.is_assignment_op() {
            if !lhs.is_lvalue() {
                // Could be destructuring assignment: `[a, b] = ...`
                let target = if matches!(&lhs, Expr::Array(_) | Expr::Object(_)) {
                    let pat = self.expr_to_pat(lhs)?;
                    AssignTarget::Destructure(pat)
                } else {
                    return Err(ParseError::InvalidContext {
                        msg: "invalid assignment target".into(),
                        span,
                    });
                };
                return Ok(Expr::Assign(AssignExpr {
                    op: token_to_assign_op(&op),
                    left: target,
                    right: Box::new(rhs),
                    span,
                }));
            }
            return Ok(Expr::Assign(AssignExpr {
                op: token_to_assign_op(&op),
                left: AssignTarget::Simple(Box::new(lhs)),
                right: Box::new(rhs),
                span,
            }));
        }

        // Sequence (comma)
        if op == TokenKind::Comma {
            let exprs = match lhs {
                Expr::Seq(mut s) => { s.exprs.push(rhs); s.exprs }
                other => vec![other, rhs],
            };
            return Ok(Expr::Seq(SeqExpr { exprs, span }));
        }

        // Logical
        if let Some(logical_op) = token_to_logical_op(&op) {
            // Check for forbidden mixing of ?? with || / &&
            if logical_op == LogicalOp::Nullish {
                if let Expr::Logical(ref l) = lhs {
                    if matches!(l.op, LogicalOp::Or | LogicalOp::And) {
                        return Err(ParseError::NullishMixedWithLogical(span));
                    }
                }
                if let Expr::Logical(ref r) = rhs {
                    if matches!(r.op, LogicalOp::Or | LogicalOp::And) {
                        return Err(ParseError::NullishMixedWithLogical(span));
                    }
                }
            }
            if matches!(logical_op, LogicalOp::Or | LogicalOp::And) {
                if let Expr::Logical(ref r) = rhs {
                    if r.op == LogicalOp::Nullish {
                        return Err(ParseError::NullishMixedWithLogical(span));
                    }
                }
            }
            return Ok(Expr::Logical(LogicalExpr {
                op: logical_op,
                left: Box::new(lhs),
                right: Box::new(rhs),
                span,
            }));
        }

        // Binary arithmetic / comparison / relational
        if let Some(bin_op) = token_to_binary_op(&op) {
            // ** is not allowed with unary on the left
            if bin_op == BinaryOp::Pow {
                if let Expr::Unary(_) = &lhs {
                    return Err(ParseError::ExponentWithUnary(span));
                }
            }
            return Ok(Expr::Binary(BinaryExpr {
                op: bin_op,
                left: Box::new(lhs),
                right: Box::new(rhs),
                span,
            }));
        }

        Err(ParseError::Unexpected {
            found: op,
            span,
            hint: "unexpected infix operator".into(),
        })
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Specialised expression parsers
    // ─────────────────────────────────────────────────────────────────────────

    // ── new  ──────────────────────────────────────────────────────────────────

    fn parse_new_expr(&mut self) -> ParseResult<Expr> {
        let start = self.start();
        self.expect(TokenKind::New)?;

        // new.target
        if self.cur.kind == TokenKind::Dot {
            self.advance()?;
            if matches!(self.cur.kind, TokenKind::Target) {
                self.advance()?;
                return Ok(Expr::NewTarget(self.span_from(start)));
            }
            return Err(ParseError::InvalidContext {
                msg: "expected 'target' after 'new.'".into(),
                span: self.cur.span,
            });
        }

        // Parse the callee: member access IS allowed (l_bp=37 ≥ 36), but
        // calls are NOT (l_bp=35 < 36). This correctly handles `new Foo.bar()`.
        let callee = self.parse_expr(36)?;
        let args = if self.cur.kind == TokenKind::LParen {
            Some(self.parse_call_args()?)
        } else {
            None
        };
        Ok(Expr::New(NewExpr { callee: Box::new(callee), args, span: self.span_from(start) }))
    }

    // ── Parenthesised expression or arrow function  ───────────────────────────

    pub(crate) fn parse_paren_or_arrow(&mut self, is_async: bool) -> ParseResult<Expr> {
        let start = self.start();
        self.expect(TokenKind::LParen)?;

        // `()` with an immediate `=>` — zero-argument arrow
        if self.cur.kind == TokenKind::RParen {
            self.advance()?;
            if self.cur.kind == TokenKind::Arrow && !self.cur.preceded_by_newline {
                self.advance()?;
                return self.finish_arrow(vec![], is_async, start);
            }
            // Otherwise it's an error (empty parens aren't a valid expression)
            return Err(ParseError::Unexpected {
                found: self.cur.kind.clone(),
                span:  self.cur.span,
                hint:  "unexpected empty parentheses".into(),
            });
        }

        // Parse the interior as a comma-separated list of expressions / patterns
        let mut items: Vec<Expr> = Vec::new();
        let mut has_rest = false;

        loop {
            // `...rest` — only valid as last arrow param
            if self.cur.kind == TokenKind::Spread {
                has_rest = true;
                let r_start = self.start();
                self.advance()?;
                let arg = self.parse_assign_expr()?;
                items.push(Expr::Spread(SpreadExpr { argument: Box::new(arg), span: self.span_from(r_start) }));
                self.eat(TokenKind::Comma)?;
                break;
            }

            items.push(self.parse_assign_expr()?);
            if !self.eat(TokenKind::Comma)? { break; }
            if self.cur.kind == TokenKind::RParen { break; } // trailing comma
        }

        self.expect(TokenKind::RParen)?;

        // Arrow function?
        if self.cur.kind == TokenKind::Arrow && !self.cur.preceded_by_newline {
            self.advance()?;
            let params = self.items_to_arrow_params(items, has_rest)?;
            return self.finish_arrow(params, is_async, start);
        }

        // Not an arrow — complain about rest / trailing comma
        if has_rest {
            return Err(ParseError::Unexpected {
                found: self.cur.kind.clone(),
                span:  self.cur.span,
                hint:  "spread in non-arrow parenthesised expression".into(),
            });
        }

        // Single expression or sequence
        if items.len() == 1 {
            Ok(items.remove(0))
        } else {
            let span = self.span_from(start);
            Ok(Expr::Seq(SeqExpr { exprs: items, span }))
        }
    }

    /// Convert a list of expressions (the contents of `(a, b = 1, ...c)`) to
    /// arrow-function parameter patterns.
    fn items_to_arrow_params(
        &mut self,
        items: Vec<Expr>,
        has_rest: bool,
    ) -> ParseResult<Vec<Param>> {
        let mut params = Vec::new();
        let n = items.len();
        for (i, item) in items.into_iter().enumerate() {
            let is_last = i + 1 == n;
            let span = item.span();
            if is_last && has_rest {
                if let Expr::Spread(s) = item {
                    let pat = self.expr_to_pat(*s.argument)?;
                    params.push(Param {
                        pat: Pat::Rest(RestPat { argument: Box::new(pat), span: s.span }),
                        span: s.span,
                    });
                    continue;
                }
            }
            let pat = self.expr_to_pat(item)?;
            params.push(Param { span, pat });
        }
        Ok(params)
    }

    /// Finish an arrow function body after `=>` has been consumed.
    fn finish_arrow(
        &mut self,
        params: Vec<Param>,
        is_async: bool,
        start: u32,
    ) -> ParseResult<Expr> {
        let saved = (self.in_function, self.in_async, self.in_generator);
        self.in_function  = true;
        self.in_async     = is_async;
        self.in_generator = false;

        let body = if self.cur.kind == TokenKind::LBrace {
            ArrowBody::Block(self.parse_block()?)
        } else {
            ArrowBody::Expr(Box::new(self.parse_assign_expr()?))
        };

        self.in_function  = saved.0;
        self.in_async     = saved.1;
        self.in_generator = saved.2;

        Ok(Expr::Arrow(ArrowExpr { params, body, is_async, span: self.span_from(start) }))
    }

    // ── async expression  ────────────────────────────────────────────────────

    fn parse_async_expr(&mut self) -> ParseResult<Expr> {
        let start = self.start();
        self.advance()?; // consume `async`

        // async function
        if !self.cur.preceded_by_newline && self.cur.kind == TokenKind::Function {
            return self.parse_function_expr(true);
        }

        // async (params) => body
        if !self.cur.preceded_by_newline && self.cur.kind == TokenKind::LParen {
            let result = self.parse_paren_or_arrow(true)?;
            // If it turned into an arrow, we're done
            if matches!(result, Expr::Arrow(_)) {
                return Ok(result);
            }
            // Otherwise it's `async` as an identifier, then a call
            let ident = Expr::Ident(Ident::new("async", Span::new(start, start + 5)));
            // Return the paren expr as a call
            let args = match result {
                Expr::Seq(s) => s.exprs,
                other => vec![other],
            };
            return Ok(Expr::Call(CallExpr {
                callee: Box::new(ident),
                args,
                span: self.span_from(start),
            }));
        }

        // async ident => body  (single-param async arrow)
        if !self.cur.preceded_by_newline
            && matches!(self.cur.kind, TokenKind::Ident(_) | TokenKind::Await)
            && self.peek.kind == TokenKind::Arrow
        {
            let ident = self.expect_ident()?;
            let param_span = ident.span;
            self.advance()?; // consume `=>`
            let params = vec![Param { pat: Pat::Ident(ident), span: param_span }];
            return self.finish_arrow(params, true, start);
        }

        // Just `async` as an identifier
        Ok(Expr::Ident(Ident::new("async", Span::new(start, start + 5))))
    }

    // ── function expression  ──────────────────────────────────────────────────

    fn parse_function_expr(&mut self, is_async: bool) -> ParseResult<Expr> {
        let start = if is_async { self.cur.span.start - 6 } else { self.start() };
        self.expect(TokenKind::Function)?;
        let is_generator = self.eat(TokenKind::Star)?;
        let id = if matches!(self.cur.kind, TokenKind::Ident(_) | TokenKind::Let | TokenKind::Static | TokenKind::Async) {
            Some(self.expect_ident()?)
        } else {
            None
        };
        let function = self.parse_function(is_async, is_generator)?;
        Ok(Expr::Function(FunctionExpr { id, function, span: self.span_from(start) }))
    }

    // ── class expression  ─────────────────────────────────────────────────────

    fn parse_class_expr(&mut self) -> ParseResult<Expr> {
        let start = self.start();
        self.expect(TokenKind::Class)?;
        let id = if matches!(self.cur.kind, TokenKind::Ident(_) | TokenKind::Let | TokenKind::Static | TokenKind::Async) {
            Some(self.expect_ident()?)
        } else {
            None
        };
        let super_class = if self.eat(TokenKind::Extends)? {
            Some(Box::new(self.parse_expr(35)?))
        } else {
            None
        };
        let body = self.parse_class_body()?;
        Ok(Expr::Class(ClassExpr { id, super_class, body, span: self.span_from(start) }))
    }

    // ── array literal  ────────────────────────────────────────────────────────

    fn parse_array_expr(&mut self) -> ParseResult<Expr> {
        let start = self.start();
        self.expect(TokenKind::LBracket)?;
        let mut elements: Vec<Option<Expr>> = Vec::new();

        while !self.at(TokenKind::RBracket) && !self.at(TokenKind::Eof) {
            if self.cur.kind == TokenKind::Comma {
                elements.push(None); // hole
                self.advance()?;
                continue;
            }
            let elem = self.parse_assign_expr()?;
            elements.push(Some(elem));
            if !self.eat(TokenKind::Comma)? { break; }
        }
        let end = self.cur.span.end;
        self.expect(TokenKind::RBracket)?;
        Ok(Expr::Array(ArrayExpr { elements, span: Span::new(start, end) }))
    }

    // ── object literal  ───────────────────────────────────────────────────────

    fn parse_object_expr(&mut self) -> ParseResult<Expr> {
        let start = self.start();
        self.expect(TokenKind::LBrace)?;
        let mut props: Vec<ObjectProp> = Vec::new();

        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            // Spread property: `...expr`
            if self.cur.kind == TokenKind::Spread {
                let sp_start = self.start();
                self.advance()?;
                let arg = self.parse_assign_expr()?;
                props.push(ObjectProp::Spread(SpreadExpr { argument: Box::new(arg), span: self.span_from(sp_start) }));
                self.eat(TokenKind::Comma)?;
                continue;
            }

            let prop_start = self.start();

            // Detect method modifiers
            let is_generator = self.eat(TokenKind::Star)?;
            let is_async = !is_generator
                && self.at(TokenKind::Async)
                && !self.peek.preceded_by_newline
                && !matches!(self.peek.kind, TokenKind::Colon | TokenKind::LParen | TokenKind::Comma | TokenKind::RBrace | TokenKind::Eq);

            if is_async { self.advance()?; }
            let is_generator2 = is_async && self.eat(TokenKind::Star)?;
            let is_generator = is_generator || is_generator2;

            // get / set accessor
            let (method_kind, key, computed) = if !is_async && !is_generator
                && matches!(self.cur.kind, TokenKind::Get | TokenKind::Set)
                && !matches!(self.peek.kind, TokenKind::LParen | TokenKind::Colon | TokenKind::Comma | TokenKind::RBrace)
            {
                let mk = if self.cur.kind == TokenKind::Get { MethodKind::Get } else { MethodKind::Set };
                self.advance()?;
                let (k, c) = self.parse_computed_key()?;
                (mk, k, c)
            } else {
                let (k, c) = self.parse_computed_key()?;
                (MethodKind::Method, k, c)
            };

            // Method shorthand: `{ key(params) { body } }`
            if self.cur.kind == TokenKind::LParen || is_generator || is_async || method_kind != MethodKind::Method {
                let function = self.parse_function(is_async, is_generator)?;
                props.push(ObjectProp::Method(MethodProp {
                    key, function, kind: method_kind, computed,
                    span: self.span_from(prop_start),
                }));
            }
            // Shorthand: `{ x }` or `{ x = default }`
            else if !computed && self.cur.kind != TokenKind::Colon {
                let ident = match &key {
                    PropKey::Ident(i) => i.clone(),
                    _ => return Err(ParseError::InvalidContext {
                        msg: "shorthand property key must be an identifier".into(),
                        span: self.cur.span,
                    }),
                };
                let value = if self.eat(TokenKind::Eq)? {
                    let right = self.parse_assign_expr()?;
                    let span = ident.span.merge(right.span());
                    // Shorthand with default: `{ x = 1 }` parses as `{ x: x = 1 }`
                    Expr::Assign(AssignExpr {
                        op: AssignOp::Assign,
                        left: AssignTarget::Simple(Box::new(Expr::Ident(ident.clone()))),
                        right: Box::new(right),
                        span,
                    })
                } else {
                    Expr::Ident(ident.clone())
                };
                props.push(ObjectProp::Keyed(KeyedProp {
                    key, value, computed: false, shorthand: true,
                    span: self.span_from(prop_start),
                }));
            }
            // Regular keyed: `{ key: value }`
            else {
                self.expect(TokenKind::Colon)?;
                let value = self.parse_assign_expr()?;
                props.push(ObjectProp::Keyed(KeyedProp {
                    key, value, computed, shorthand: false,
                    span: self.span_from(prop_start),
                }));
            }

            if !self.eat(TokenKind::Comma)? { break; }
        }
        let end = self.cur.span.end;
        self.expect(TokenKind::RBrace)?;
        Ok(Expr::Object(ObjectExpr { props, span: Span::new(start, end) }))
    }

    /// Parse an optionally-computed property key: `[expr]` or a static key.
    fn parse_computed_key(&mut self) -> ParseResult<(PropKey, bool)> {
        if self.cur.kind == TokenKind::LBracket {
            self.advance()?;
            let expr = self.parse_assign_expr()?;
            self.expect(TokenKind::RBracket)?;
            Ok((PropKey::Computed(Box::new(expr)), true))
        } else {
            Ok((self.parse_prop_key_static()?, false))
        }
    }

    // ── Template literal  ────────────────────────────────────────────────────

    pub(crate) fn parse_template_literal(&mut self) -> ParseResult<TemplateLit> {
        let start = self.start();
        match self.cur.kind.clone() {
            TokenKind::TemplateNoSub(s) => {
                let span = self.cur.span;
                self.advance()?;
                Ok(TemplateLit {
                    quasis: vec![TemplateElement { cooked: s, tail: true, span }],
                    exprs: vec![],
                    span: self.span_from(start),
                })
            }
            TokenKind::TemplateHead(s) => {
                let head_span = self.cur.span;
                self.advance()?;
                let mut quasis = vec![TemplateElement { cooked: s, tail: false, span: head_span }];
                let mut exprs  = vec![];

                loop {
                    // Parse the substitution expression
                    exprs.push(self.parse_expr(0)?);

                    // Expect a template continuation or tail
                    match self.cur.kind.clone() {
                        TokenKind::TemplateMiddle(s) => {
                            let q_span = self.cur.span;
                            self.advance()?;
                            quasis.push(TemplateElement { cooked: s, tail: false, span: q_span });
                        }
                        TokenKind::TemplateTail(s) => {
                            let q_span = self.cur.span;
                            self.advance()?;
                            quasis.push(TemplateElement { cooked: s, tail: true, span: q_span });
                            break;
                        }
                        _ => return Err(ParseError::UnexpectedEof(self.cur.span)),
                    }
                }
                Ok(TemplateLit { quasis, exprs, span: self.span_from(start) })
            }
            _ => Err(ParseError::Unexpected {
                found: self.cur.kind.clone(),
                span:  self.cur.span,
                hint:  "expected template literal".into(),
            }),
        }
    }

    // ── Call argument list  ───────────────────────────────────────────────────

    fn parse_call_args(&mut self) -> ParseResult<Vec<Expr>> {
        self.expect(TokenKind::LParen)?;
        let mut args = Vec::new();
        while !self.at(TokenKind::RParen) && !self.at(TokenKind::Eof) {
            if self.cur.kind == TokenKind::Spread {
                let sp_start = self.start();
                self.advance()?;
                let arg = self.parse_assign_expr()?;
                args.push(Expr::Spread(SpreadExpr { argument: Box::new(arg), span: self.span_from(sp_start) }));
            } else {
                args.push(self.parse_assign_expr()?);
            }
            if !self.eat(TokenKind::Comma)? { break; }
        }
        self.expect(TokenKind::RParen)?;
        Ok(args)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Operator mapping helpers
// ─────────────────────────────────────────────────────────────────────────────

fn token_to_unary_op(kind: &TokenKind) -> Option<UnaryOp> {
    Some(match kind {
        TokenKind::Minus  => UnaryOp::Neg,
        TokenKind::Plus   => UnaryOp::Pos,
        TokenKind::Bang   => UnaryOp::Not,
        TokenKind::Tilde  => UnaryOp::BitNot,
        TokenKind::Typeof => UnaryOp::Typeof,
        TokenKind::Void   => UnaryOp::Void,
        TokenKind::Delete => UnaryOp::Delete,
        _ => return None,
    })
}

fn token_to_logical_op(kind: &TokenKind) -> Option<LogicalOp> {
    Some(match kind {
        TokenKind::AmpAmp           => LogicalOp::And,
        TokenKind::PipePipe         => LogicalOp::Or,
        TokenKind::QuestionQuestion => LogicalOp::Nullish,
        _ => return None,
    })
}

fn token_to_binary_op(kind: &TokenKind) -> Option<BinaryOp> {
    Some(match kind {
        TokenKind::Plus        => BinaryOp::Add,
        TokenKind::Minus       => BinaryOp::Sub,
        TokenKind::Star        => BinaryOp::Mul,
        TokenKind::Slash       => BinaryOp::Div,
        TokenKind::Percent     => BinaryOp::Rem,
        TokenKind::StarStar    => BinaryOp::Pow,
        TokenKind::Amp         => BinaryOp::BitAnd,
        TokenKind::Pipe        => BinaryOp::BitOr,
        TokenKind::Caret       => BinaryOp::BitXor,
        TokenKind::LtLt        => BinaryOp::Shl,
        TokenKind::GtGt        => BinaryOp::Shr,
        TokenKind::GtGtGt      => BinaryOp::UShr,
        TokenKind::EqEq        => BinaryOp::Eq,
        TokenKind::BangEq      => BinaryOp::NotEq,
        TokenKind::EqEqEq      => BinaryOp::StrictEq,
        TokenKind::BangEqEq    => BinaryOp::StrictNotEq,
        TokenKind::Lt          => BinaryOp::Lt,
        TokenKind::LtEq        => BinaryOp::LtEq,
        TokenKind::Gt          => BinaryOp::Gt,
        TokenKind::GtEq        => BinaryOp::GtEq,
        TokenKind::In          => BinaryOp::In,
        TokenKind::Instanceof  => BinaryOp::Instanceof,
        _ => return None,
    })
}

fn token_to_assign_op(kind: &TokenKind) -> AssignOp {
    match kind {
        TokenKind::Eq            => AssignOp::Assign,
        TokenKind::PlusEq        => AssignOp::AddAssign,
        TokenKind::MinusEq       => AssignOp::SubAssign,
        TokenKind::StarEq        => AssignOp::MulAssign,
        TokenKind::SlashEq       => AssignOp::DivAssign,
        TokenKind::PercentEq     => AssignOp::RemAssign,
        TokenKind::StarStarEq    => AssignOp::PowAssign,
        TokenKind::AmpEq         => AssignOp::BitAndAssign,
        TokenKind::PipeEq        => AssignOp::BitOrAssign,
        TokenKind::CaretEq       => AssignOp::BitXorAssign,
        TokenKind::LtLtEq        => AssignOp::ShlAssign,
        TokenKind::GtGtEq        => AssignOp::ShrAssign,
        TokenKind::GtGtGtEq      => AssignOp::UShrAssign,
        TokenKind::AmpAmpEq      => AssignOp::AndAssign,
        TokenKind::PipePipeEq    => AssignOp::OrAssign,
        TokenKind::QuestionQuestionEq => AssignOp::NullishAssign,
        _ => AssignOp::Assign,
    }
}