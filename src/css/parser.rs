//! CSS stylesheet parser — builds a [`Stylesheet`] from a token stream.
//!
//! Implements the CSS Syntax Level 3 parsing algorithms.

use alloc::{string::String, vec::Vec};

use super::ast::{
    AtRule, AtRuleBlock, Block, BlockKind, ComponentValue, Declaration, FunctionBlock,
    KeyframeBlock, KeyframeSelector, QualifiedRule, Rule, Stylesheet,
};
use super::error::ParseError;
use super::selector::parse_selector_list;
use super::tokenizer::{Token, Tokenizer};

// ── At-rule classification ────────────────────────────────────────────────────

const DECLARATION_AT_RULES: &[&str] = &[
    "font-face",
    "page",
    "counter-style",
    "font-palette-values",
    "viewport",
    "-ms-viewport",
];
const KEYFRAME_AT_RULES: &[&str] = &["keyframes", "-webkit-keyframes", "-moz-keyframes"];

// ── Parser ────────────────────────────────────────────────────────────────────

/// CSS stylesheet parser.
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    /// Create a new parser for `input`.
    pub fn new(input: &str) -> Self {
        let tokens: Vec<Token> = Tokenizer::new(input).collect();
        Self { tokens, pos: 0 }
    }

    // ── Token stream ─────────────────────────────────────────────────────

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<Token> {
        let t = self.tokens.get(self.pos)?.clone();
        self.pos += 1;
        Some(t)
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(t) if t.is_whitespace()) {
            self.pos += 1;
        }
    }

    fn eat_if<P: Fn(&Token) -> bool>(&mut self, p: P) -> bool {
        if self.peek().map_or(false, &p) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    // ── Entry point ───────────────────────────────────────────────────────

    /// Parse the input into a [`Stylesheet`].
    pub fn parse(mut self) -> Result<Stylesheet, ParseError> {
        let rules = self.parse_rule_list(true);
        Ok(Stylesheet { rules })
    }

    // ── Rule list ─────────────────────────────────────────────────────────

    fn parse_rule_list(&mut self, top_level: bool) -> Vec<Rule> {
        let mut rules = Vec::new();
        loop {
            self.skip_ws();
            match self.peek() {
                None => break,
                Some(Token::RightBrace) if !top_level => break,
                Some(Token::Cdo) | Some(Token::Cdc) if top_level => {
                    self.advance();
                }
                Some(Token::AtKeyword(_)) => {
                    if let Some(r) = self.parse_at_rule() {
                        rules.push(Rule::At(r));
                    }
                }
                _ => {
                    if let Some(r) = self.parse_qualified_rule() {
                        rules.push(Rule::Qualified(r));
                    }
                }
            }
        }
        rules
    }

    // ── Qualified rule ────────────────────────────────────────────────────

    fn parse_qualified_rule(&mut self) -> Option<QualifiedRule> {
        let prelude = self.cvs_until_brace();
        if !self.eat_if(|t| matches!(t, Token::LeftBrace)) {
            return None;
        }
        let declarations = self.parse_decl_list();
        self.eat_if(|t| matches!(t, Token::RightBrace));

        let toks: Vec<Token> = prelude
            .iter()
            .filter_map(|cv| match cv {
                ComponentValue::Token(t) => Some(t.clone()),
                _ => None,
            })
            .collect();
        let selectors = if toks.is_empty() {
            None
        } else {
            let s = parse_selector_list(&toks);
            if s.0.is_empty() { None } else { Some(s) }
        };
        Some(QualifiedRule {
            selectors,
            prelude,
            declarations,
        })
    }

    // ── At-rule ───────────────────────────────────────────────────────────

    fn parse_at_rule(&mut self) -> Option<AtRule> {
        let name = match self.advance()? {
            Token::AtKeyword(n) => n.to_ascii_lowercase(),
            _ => return None,
        };
        let prelude = self.cvs_until_brace_or_semi();
        match self.peek() {
            Some(Token::Semicolon) | None => {
                self.eat_if(|t| matches!(t, Token::Semicolon));
                Some(AtRule {
                    name,
                    prelude,
                    block: None,
                })
            }
            Some(Token::LeftBrace) => {
                self.advance();
                let block = self.parse_at_block(&name);
                self.eat_if(|t| matches!(t, Token::RightBrace));
                Some(AtRule {
                    name,
                    prelude,
                    block: Some(block),
                })
            }
            _ => Some(AtRule {
                name,
                prelude,
                block: None,
            }),
        }
    }

    fn parse_at_block(&mut self, name: &str) -> AtRuleBlock {
        if KEYFRAME_AT_RULES.contains(&name) {
            AtRuleBlock::Keyframes(self.parse_keyframes())
        } else if DECLARATION_AT_RULES.contains(&name) {
            AtRuleBlock::Declarations(self.parse_decl_list())
        } else {
            AtRuleBlock::Rules(self.parse_rule_list(false))
        }
    }

    // ── Keyframes ─────────────────────────────────────────────────────────

    fn parse_keyframes(&mut self) -> Vec<KeyframeBlock> {
        let mut blocks = Vec::new();
        loop {
            self.skip_ws();
            match self.peek() {
                None | Some(Token::RightBrace) => break,
                _ => {
                    if let Some(k) = self.parse_keyframe() {
                        blocks.push(k);
                    }
                }
            }
        }
        blocks
    }

    fn parse_keyframe(&mut self) -> Option<KeyframeBlock> {
        let selectors = self.parse_keyframe_selectors();
        if selectors.is_empty() {
            return None;
        }
        if !self.eat_if(|t| matches!(t, Token::LeftBrace)) {
            return None;
        }
        let declarations = self.parse_decl_list();
        self.eat_if(|t| matches!(t, Token::RightBrace));
        Some(KeyframeBlock {
            selectors,
            declarations,
        })
    }

    fn parse_keyframe_selectors(&mut self) -> Vec<KeyframeSelector> {
        let mut sels = Vec::new();
        loop {
            self.skip_ws();
            match self.peek() {
                Some(Token::Ident(s)) if s.eq_ignore_ascii_case("from") => {
                    self.advance();
                    sels.push(KeyframeSelector::From);
                }
                Some(Token::Ident(s)) if s.eq_ignore_ascii_case("to") => {
                    self.advance();
                    sels.push(KeyframeSelector::To);
                }
                Some(Token::Percentage { value, .. }) => {
                    let v = *value;
                    self.advance();
                    sels.push(KeyframeSelector::Percentage(v));
                }
                Some(Token::LeftBrace) | None => break,
                _ => {
                    self.advance();
                }
            }
            self.skip_ws();
            if !self.eat_if(|t| matches!(t, Token::Comma)) {
                break;
            }
        }
        sels
    }

    // ── Declaration list ──────────────────────────────────────────────────

    fn parse_decl_list(&mut self) -> Vec<Declaration> {
        let mut decls = Vec::new();
        loop {
            self.skip_ws();
            match self.peek() {
                None | Some(Token::RightBrace) => break,
                Some(Token::Semicolon) => {
                    self.advance();
                }
                Some(Token::AtKeyword(_)) => {
                    self.cvs_until_brace_or_semi();
                    self.eat_if(|t| matches!(t, Token::Semicolon));
                }
                _ => {
                    if let Some(d) = self.parse_decl() {
                        decls.push(d);
                    }
                    self.skip_ws();
                    self.eat_if(|t| matches!(t, Token::Semicolon));
                }
            }
        }
        decls
    }

    fn parse_decl(&mut self) -> Option<Declaration> {
        self.skip_ws();
        // Property name — handle regular idents and custom properties (--foo)
        let property = match self.advance()? {
            Token::Ident(p) => p.to_ascii_lowercase(),
            // Custom properties start with -- which tokenizes as Ident("--foo")
            // or as Delim('-') + Delim('-') + Ident in some tokenizers
            _ => {
                self.skip_to_semi();
                return None;
            }
        };
        self.skip_ws();
        if !self.eat_if(|t| matches!(t, Token::Colon)) {
            self.skip_to_semi();
            return None;
        }
        self.skip_ws();
        let mut value = self.cvs_declaration_value();
        let important = Self::strip_important(&mut value);
        // trim trailing whitespace
        while value
            .last()
            .map_or(false, |cv: &ComponentValue| cv.is_whitespace())
        {
            value.pop();
        }
        Some(Declaration {
            property,
            value,
            important,
        })
    }

    fn strip_important(value: &mut Vec<ComponentValue>) -> bool {
        // trim trailing ws
        while value
            .last()
            .map_or(false, |cv: &ComponentValue| cv.is_whitespace())
        {
            value.pop();
        }
        if !matches!(value.last(), Some(ComponentValue::Token(Token::Ident(s))) if s.eq_ignore_ascii_case("important"))
        {
            return false;
        }
        value.pop();
        while value
            .last()
            .map_or(false, |cv: &ComponentValue| cv.is_whitespace())
        {
            value.pop();
        }
        if !matches!(value.last(), Some(ComponentValue::Token(Token::Delim('!')))) {
            return false;
        }
        value.pop();
        true
    }

    // ── Component value consumers ─────────────────────────────────────────

    fn cvs_until_brace(&mut self) -> Vec<ComponentValue> {
        let mut cvs = Vec::new();
        loop {
            match self.peek() {
                None | Some(Token::LeftBrace) => break,
                _ => {
                    if let Some(cv) = self.consume_cv() {
                        cvs.push(cv);
                    }
                }
            }
        }
        cvs
    }

    fn cvs_until_brace_or_semi(&mut self) -> Vec<ComponentValue> {
        let mut cvs = Vec::new();
        loop {
            match self.peek() {
                None | Some(Token::LeftBrace) | Some(Token::Semicolon) => break,
                _ => {
                    if let Some(cv) = self.consume_cv() {
                        cvs.push(cv);
                    }
                }
            }
        }
        cvs
    }

    fn cvs_declaration_value(&mut self) -> Vec<ComponentValue> {
        let mut cvs = Vec::new();
        loop {
            match self.peek() {
                None | Some(Token::Semicolon) | Some(Token::RightBrace) => break,
                _ => {
                    if let Some(cv) = self.consume_cv() {
                        cvs.push(cv);
                    }
                }
            }
        }
        cvs
    }

    fn consume_cv(&mut self) -> Option<ComponentValue> {
        match self.peek()? {
            Token::LeftBrace => Some(ComponentValue::Block(self.consume_block(BlockKind::Curly))),
            Token::LeftBracket => {
                Some(ComponentValue::Block(self.consume_block(BlockKind::Square)))
            }
            Token::LeftParen => Some(ComponentValue::Block(self.consume_block(BlockKind::Paren))),
            Token::Function(_) => Some(ComponentValue::Function(self.consume_function())),
            _ => self.advance().map(ComponentValue::Token),
        }
    }

    fn consume_block(&mut self, kind: BlockKind) -> Block {
        self.advance(); // opening bracket
        let (_, close) = kind.delimiters();
        let mut value = Vec::new();
        loop {
            let done = match self.peek() {
                None => true,
                Some(Token::RightBrace) => close == '}',
                Some(Token::RightBracket) => close == ']',
                Some(Token::RightParen) => close == ')',
                _ => false,
            };
            if done {
                self.eat_if(|_| true);
                break;
            }
            if let Some(cv) = self.consume_cv() {
                value.push(cv);
            }
        }
        Block { kind, value }
    }

    fn consume_function(&mut self) -> FunctionBlock {
        let name = match self.advance() {
            Some(Token::Function(n)) => n.to_ascii_lowercase(),
            _ => String::new(),
        };
        let mut value = Vec::new();
        loop {
            match self.peek() {
                None | Some(Token::RightParen) => {
                    self.eat_if(|t| matches!(t, Token::RightParen));
                    break;
                }
                _ => {
                    if let Some(cv) = self.consume_cv() {
                        value.push(cv);
                    }
                }
            }
        }
        FunctionBlock { name, value }
    }

    fn skip_to_semi(&mut self) {
        loop {
            match self.peek() {
                None | Some(Token::Semicolon) | Some(Token::RightBrace) => break,
                Some(Token::LeftBrace) => {
                    self.consume_block(BlockKind::Curly);
                }
                _ => {
                    self.advance();
                }
            }
        }
    }
}
