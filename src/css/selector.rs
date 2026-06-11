//! CSS Selector Level 4 parser.
//!
//! Parses a slice of [`Token`]s into a [`SelectorList`] AST.

use alloc::{string::String, vec::Vec};
use core::fmt;

use super::tokenizer::Token;

// ── AST types ─────────────────────────────────────────────────────────────────

/// A comma-separated list of complex selectors: `.a, .b`.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectorList(pub Vec<ComplexSelector>);

/// A complex selector: combinators between compound selectors.
#[derive(Debug, Clone, PartialEq)]
pub struct ComplexSelector(pub Vec<SelectorPart>);

/// A compound selector with its preceding combinator.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectorPart {
    /// `None` for the first item in the chain.
    pub combinator: Option<Combinator>,
    /// The compound selector.
    pub compound: CompoundSelector,
}

/// Combinator between two compound selectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combinator {
    /// ` ` — descendant
    Descendant,
    /// `>` — direct child
    Child,
    /// `+` — adjacent sibling
    AdjacentSibling,
    /// `~` — general sibling
    GeneralSibling,
    /// `||` — grid column
    Column,
}

/// A compound selector: sequence of simple selectors, e.g. `div.foo:hover`.
#[derive(Debug, Clone, PartialEq)]
pub struct CompoundSelector {
    /// Simple selectors in this compound.
    pub parts: Vec<SimpleSelector>,
}

/// A single simple selector.
#[derive(Debug, Clone, PartialEq)]
pub enum SimpleSelector {
    /// `*`
    Universal,
    /// `div`, `span`
    Type(String),
    /// `.foo`
    Class(String),
    /// `#bar`
    Id(String),
    /// `[attr]`, `[attr=value]`
    Attribute(AttributeSelector),
    /// `:hover`, `:nth-child(…)`
    PseudoClass(PseudoSelector),
    /// `::before`
    PseudoElement(PseudoSelector),
}

/// An attribute selector: `[attr op value flag]`.
#[derive(Debug, Clone, PartialEq)]
pub struct AttributeSelector {
    /// Attribute name.
    pub attr: String,
    /// Optional namespace prefix.
    pub namespace: Option<String>,
    /// Match condition, if any.
    pub matcher: Option<AttributeMatcher>,
    /// Case-sensitivity flag.
    pub case: CaseSensitivity,
}

/// The match condition of an attribute selector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributeMatcher {
    /// Matching operator.
    pub operator: AttributeOperator,
    /// Value to match against.
    pub value: String,
}

/// Attribute selector operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeOperator {
    /// `=`   exact match
    Equal,
    /// `~=`  word in space-separated list
    Includes,
    /// `|=`  exact or dash-prefix
    DashMatch,
    /// `^=`  starts with
    Prefix,
    /// `$=`  ends with
    Suffix,
    /// `*=`  substring
    Substring,
}

/// Case-sensitivity flag (`i` / `s` / default).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseSensitivity {
    /// Browser/document default.
    Default,
    /// `s` — force case-sensitive.
    Sensitive,
    /// `i` — force case-insensitive.
    Insensitive,
}

/// A pseudo-class or pseudo-element selector.
#[derive(Debug, Clone, PartialEq)]
pub struct PseudoSelector {
    /// Name without `:` / `::`, lowercased.
    pub name: String,
    /// Argument tokens for functional pseudos like `:nth-child(2n+1)`.
    pub argument: Option<Vec<Token>>,
}

// ── Display ───────────────────────────────────────────────────────────────────

impl fmt::Display for SelectorList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, s) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", s)?;
        }
        Ok(())
    }
}

impl fmt::Display for ComplexSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for part in &self.0 {
            if let Some(c) = part.combinator {
                match c {
                    Combinator::Descendant => write!(f, " ")?,
                    Combinator::Child => write!(f, " > ")?,
                    Combinator::AdjacentSibling => write!(f, " + ")?,
                    Combinator::GeneralSibling => write!(f, " ~ ")?,
                    Combinator::Column => write!(f, " || ")?,
                }
            }
            write!(f, "{}", part.compound)?;
        }
        Ok(())
    }
}

impl fmt::Display for CompoundSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for p in &self.parts {
            write!(f, "{}", p)?;
        }
        Ok(())
    }
}

impl fmt::Display for SimpleSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SimpleSelector::Universal => write!(f, "*"),
            SimpleSelector::Type(t) => write!(f, "{}", t),
            SimpleSelector::Class(c) => write!(f, ".{}", c),
            SimpleSelector::Id(i) => write!(f, "#{}", i),
            SimpleSelector::Attribute(a) => write!(f, "{}", a),
            SimpleSelector::PseudoClass(p) => write!(f, ":{}", p),
            SimpleSelector::PseudoElement(p) => write!(f, "::{}", p),
        }
    }
}

impl fmt::Display for AttributeSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        if let Some(ns) = &self.namespace {
            write!(f, "{}|", ns)?;
        }
        write!(f, "{}", self.attr)?;
        if let Some(m) = &self.matcher {
            let op = match m.operator {
                AttributeOperator::Equal => "=",
                AttributeOperator::Includes => "~=",
                AttributeOperator::DashMatch => "|=",
                AttributeOperator::Prefix => "^=",
                AttributeOperator::Suffix => "$=",
                AttributeOperator::Substring => "*=",
            };
            write!(f, r#"{}"{}"#, op, m.value)?;
        }
        match self.case {
            CaseSensitivity::Insensitive => write!(f, " i")?,
            CaseSensitivity::Sensitive => write!(f, " s")?,
            CaseSensitivity::Default => {}
        }
        write!(f, "]")
    }
}

impl fmt::Display for PseudoSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)?;
        if self.argument.is_some() {
            write!(f, "(…)")?;
        }
        Ok(())
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Parse a CSS selector list from a token slice.
pub fn parse_selector_list(tokens: &[Token]) -> SelectorList {
    SelectorParser::new(tokens).parse_list()
}

// ── Parser impl ───────────────────────────────────────────────────────────────

struct SelectorParser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> SelectorParser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn peek_at(&self, n: usize) -> Option<&Token> {
        self.tokens.get(self.pos + n)
    }

    fn advance(&mut self) -> Option<&Token> {
        let t = self.tokens.get(self.pos)?;
        self.pos += 1;
        Some(t)
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(Token::Whitespace)) {
            self.pos += 1;
        }
    }

    fn eat_if<P: Fn(&Token) -> bool>(&mut self, pred: P) -> bool {
        if self.peek().map_or(false, &pred) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    // ── grammar ──────────────────────────────────────────────────────────

    fn parse_list(&mut self) -> SelectorList {
        let mut list = Vec::new();
        loop {
            self.skip_whitespace();
            if self.pos >= self.tokens.len() {
                break;
            }
            let c = self.parse_complex();
            if !c.0.is_empty() {
                list.push(c);
            }
            self.skip_whitespace();
            if !self.eat_if(|t| matches!(t, Token::Comma)) {
                break;
            }
        }
        SelectorList(list)
    }

    fn parse_complex(&mut self) -> ComplexSelector {
        let mut parts = Vec::new();
        let first = self.parse_compound();
        if !first.parts.is_empty() {
            parts.push(SelectorPart {
                combinator: None,
                compound: first,
            });
        }
        loop {
            let had_ws = matches!(self.peek(), Some(Token::Whitespace));
            self.skip_whitespace();
            let combinator = match self.peek() {
                Some(Token::Delim('>')) => {
                    self.advance();
                    self.skip_whitespace();
                    Some(Combinator::Child)
                }
                Some(Token::Delim('+')) => {
                    self.advance();
                    self.skip_whitespace();
                    Some(Combinator::AdjacentSibling)
                }
                Some(Token::Delim('~')) => {
                    self.advance();
                    self.skip_whitespace();
                    Some(Combinator::GeneralSibling)
                }
                Some(Token::Delim('|')) => {
                    self.advance();
                    if self.eat_if(|t| matches!(t, Token::Delim('|'))) {
                        self.skip_whitespace();
                        Some(Combinator::Column)
                    } else {
                        break;
                    }
                }
                Some(t) if had_ws && is_compound_start(t) => Some(Combinator::Descendant),
                _ => break,
            };
            if !self.peek().map_or(false, is_compound_start) {
                break;
            }
            let c = self.parse_compound();
            if c.parts.is_empty() {
                break;
            }
            parts.push(SelectorPart {
                combinator,
                compound: c,
            });
        }
        ComplexSelector(parts)
    }

    fn parse_compound(&mut self) -> CompoundSelector {
        let mut parts = Vec::new();
        let mut first = true;
        loop {
            match self.peek() {
                Some(Token::Delim('*')) if first => {
                    self.advance();
                    parts.push(SimpleSelector::Universal);
                    first = false;
                }
                Some(Token::Ident(_)) if first => {
                    if let Some(Token::Ident(n)) = self.advance() {
                        parts.push(SimpleSelector::Type(n.clone()));
                    }
                    first = false;
                }
                Some(Token::Delim('.')) => {
                    self.advance();
                    if let Some(Token::Ident(n)) = self.advance() {
                        parts.push(SimpleSelector::Class(n.clone()));
                        first = false;
                    } else {
                        break;
                    }
                }
                Some(Token::Hash { .. }) => {
                    if let Some(Token::Hash { value, .. }) = self.advance() {
                        parts.push(SimpleSelector::Id(value.clone()));
                        first = false;
                    }
                }
                Some(Token::LeftBracket) => {
                    self.advance();
                    parts.push(SimpleSelector::Attribute(self.parse_attr()));
                    first = false;
                }
                Some(Token::Colon) => {
                    self.advance();
                    let is_elem = self.eat_if(|t| matches!(t, Token::Colon));
                    match self.advance() {
                        Some(Token::Ident(n)) => {
                            let p = PseudoSelector {
                                name: n.clone(),
                                argument: None,
                            };
                            parts.push(if is_elem {
                                SimpleSelector::PseudoElement(p)
                            } else {
                                SimpleSelector::PseudoClass(p)
                            });
                        }
                        Some(Token::Function(n)) => {
                            let name_owned = n.clone();
                            let args = self.collect_to_paren();
                            let p = PseudoSelector {
                                name: name_owned,
                                argument: Some(args),
                            };
                            parts.push(if is_elem {
                                SimpleSelector::PseudoElement(p)
                            } else {
                                SimpleSelector::PseudoClass(p)
                            });
                        }
                        _ => break,
                    }
                    first = false;
                }
                _ => break,
            }
        }
        CompoundSelector { parts }
    }

    fn parse_attr(&mut self) -> AttributeSelector {
        self.skip_whitespace();
        // Peek for ns|attr pattern: Ident '|' Ident (NOT Ident '|' '=')
        let (namespace, attr) = match (self.peek(), self.peek_at(1), self.peek_at(2)) {
            (Some(Token::Ident(_)), Some(Token::Delim('|')), Some(Token::Ident(_)))
            | (Some(Token::Ident(_)), Some(Token::Delim('|')), Some(Token::Delim('*'))) => {
                if let Some(Token::Ident(ns)) = self.advance() {
                    let ns = ns.clone();
                    self.advance(); // '|'
                    match self.advance() {
                        Some(Token::Ident(a)) => (Some(ns), a.clone()),
                        _ => {
                            self.drain_bracket();
                            return default_attr();
                        }
                    }
                } else {
                    unreachable!()
                }
            }
            _ => match self.advance() {
                Some(Token::Ident(a)) => (None, a.clone()),
                _ => {
                    self.drain_bracket();
                    return default_attr();
                }
            },
        };
        self.skip_whitespace();
        // Check for end or operator
        if self.eat_if(|t| matches!(t, Token::RightBracket)) {
            return AttributeSelector {
                attr,
                namespace,
                matcher: None,
                case: CaseSensitivity::Default,
            };
        }
        let operator = self.parse_attr_op();
        self.skip_whitespace();
        let value = match self.advance() {
            Some(Token::Ident(v)) | Some(Token::String(v)) => v.clone(),
            _ => String::new(),
        };
        self.skip_whitespace();
        let case = match self.peek() {
            Some(Token::Ident(s)) if s.eq_ignore_ascii_case("i") => {
                self.advance();
                CaseSensitivity::Insensitive
            }
            Some(Token::Ident(s)) if s.eq_ignore_ascii_case("s") => {
                self.advance();
                CaseSensitivity::Sensitive
            }
            _ => CaseSensitivity::Default,
        };
        self.skip_whitespace();
        self.eat_if(|t| matches!(t, Token::RightBracket));
        AttributeSelector {
            attr,
            namespace,
            matcher: Some(AttributeMatcher { operator, value }),
            case,
        }
    }

    fn parse_attr_op(&mut self) -> AttributeOperator {
        match self.advance() {
            Some(Token::Delim('=')) => AttributeOperator::Equal,
            Some(Token::Delim('~')) => {
                self.eat_if(|t| matches!(t, Token::Delim('=')));
                AttributeOperator::Includes
            }
            Some(Token::Delim('|')) => {
                self.eat_if(|t| matches!(t, Token::Delim('=')));
                AttributeOperator::DashMatch
            }
            Some(Token::Delim('^')) => {
                self.eat_if(|t| matches!(t, Token::Delim('=')));
                AttributeOperator::Prefix
            }
            Some(Token::Delim('$')) => {
                self.eat_if(|t| matches!(t, Token::Delim('=')));
                AttributeOperator::Suffix
            }
            Some(Token::Delim('*')) => {
                self.eat_if(|t| matches!(t, Token::Delim('=')));
                AttributeOperator::Substring
            }
            _ => AttributeOperator::Equal,
        }
    }

    fn collect_to_paren(&mut self) -> Vec<Token> {
        let mut depth = 0usize;
        let mut out = Vec::new();
        loop {
            match self.peek() {
                None => break,
                Some(Token::Function(_)) | Some(Token::LeftParen) => {
                    depth += 1;
                    out.push(self.advance().unwrap().clone());
                }
                Some(Token::RightParen) => {
                    if depth == 0 {
                        self.advance();
                        break;
                    }
                    depth -= 1;
                    out.push(self.advance().unwrap().clone());
                }
                _ => {
                    out.push(self.advance().unwrap().clone());
                }
            }
        }
        out
    }

    fn drain_bracket(&mut self) {
        loop {
            match self.peek() {
                None | Some(Token::RightBracket) => {
                    self.eat_if(|t| matches!(t, Token::RightBracket));
                    break;
                }
                _ => {
                    self.advance();
                }
            }
        }
    }
}

fn default_attr() -> AttributeSelector {
    AttributeSelector {
        attr: String::new(),
        namespace: None,
        matcher: None,
        case: CaseSensitivity::Default,
    }
}

fn is_compound_start(t: &Token) -> bool {
    matches!(
        t,
        Token::Ident(_)
            | Token::Delim('.')
            | Token::Delim('*')
            | Token::Hash { .. }
            | Token::LeftBracket
            | Token::Colon
    )
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::tokenizer::Tokenizer;

    fn parse(src: &str) -> SelectorList {
        let toks: Vec<Token> = Tokenizer::new(src).collect();
        parse_selector_list(&toks)
    }
    fn first_compound(src: &str) -> CompoundSelector {
        parse(src)
            .0
            .into_iter()
            .next()
            .unwrap()
            .0
            .into_iter()
            .next()
            .unwrap()
            .compound
    }
    fn first_simple(src: &str) -> SimpleSelector {
        first_compound(src).parts.into_iter().next().unwrap()
    }
    fn first_complex(src: &str) -> ComplexSelector {
        parse(src).0.into_iter().next().unwrap()
    }

    #[test]
    fn type_sel() {
        assert_eq!(first_simple("div"), SimpleSelector::Type("div".into()));
    }
    #[test]
    fn universal_sel() {
        assert_eq!(first_simple("*"), SimpleSelector::Universal);
    }
    #[test]
    fn class_sel() {
        assert_eq!(first_simple(".foo"), SimpleSelector::Class("foo".into()));
    }
    #[test]
    fn id_sel() {
        assert_eq!(first_simple("#main"), SimpleSelector::Id("main".into()));
    }

    #[test]
    fn compound_type_and_class() {
        let c = first_compound("div.foo");
        assert_eq!(c.parts.len(), 2);
        assert_eq!(c.parts[0], SimpleSelector::Type("div".into()));
        assert_eq!(c.parts[1], SimpleSelector::Class("foo".into()));
    }

    #[test]
    fn compound_full() {
        let c = first_compound("a.nav#link[href]:hover");
        assert_eq!(c.parts.len(), 5);
        assert!(matches!(c.parts[0], SimpleSelector::Type(_)));
        assert!(matches!(c.parts[1], SimpleSelector::Class(_)));
        assert!(matches!(c.parts[2], SimpleSelector::Id(_)));
        assert!(matches!(c.parts[3], SimpleSelector::Attribute(_)));
        assert!(matches!(c.parts[4], SimpleSelector::PseudoClass(_)));
    }

    #[test]
    fn attr_presence() {
        match first_simple("[disabled]") {
            SimpleSelector::Attribute(a) => {
                assert_eq!(a.attr, "disabled");
                assert!(a.matcher.is_none());
            }
            o => panic!("{:?}", o),
        }
    }

    #[test]
    fn attr_equal() {
        match first_simple("[type=text]") {
            SimpleSelector::Attribute(a) => {
                let m = a.matcher.unwrap();
                assert_eq!(m.operator, AttributeOperator::Equal);
                assert_eq!(m.value, "text");
            }
            o => panic!("{:?}", o),
        }
    }

    #[test]
    fn attr_operators() {
        let cases = [
            ("[a~=b]", AttributeOperator::Includes),
            ("[a|=b]", AttributeOperator::DashMatch),
            ("[a^=b]", AttributeOperator::Prefix),
            ("[a$=b]", AttributeOperator::Suffix),
            ("[a*=b]", AttributeOperator::Substring),
        ];
        for (src, op) in cases {
            match first_simple(src) {
                SimpleSelector::Attribute(a) => {
                    assert_eq!(a.matcher.unwrap().operator, op, "{}", src)
                }
                o => panic!("{:?}", o),
            }
        }
    }

    #[test]
    fn attr_case_flag() {
        match first_simple("[lang=en i]") {
            SimpleSelector::Attribute(a) => assert_eq!(a.case, CaseSensitivity::Insensitive),
            o => panic!("{:?}", o),
        }
    }

    #[test]
    fn pseudo_class() {
        match first_simple(":hover") {
            SimpleSelector::PseudoClass(p) => assert_eq!(p.name, "hover"),
            o => panic!("{:?}", o),
        }
    }

    #[test]
    fn pseudo_class_with_arg() {
        match first_simple(":nth-child(2n+1)") {
            SimpleSelector::PseudoClass(p) => {
                assert_eq!(p.name, "nth-child");
                assert!(p.argument.is_some());
            }
            o => panic!("{:?}", o),
        }
    }

    #[test]
    fn pseudo_element() {
        match first_simple("::after") {
            SimpleSelector::PseudoElement(p) => assert_eq!(p.name, "after"),
            o => panic!("{:?}", o),
        }
    }

    #[test]
    fn descendant() {
        let c = first_complex("div span");
        assert_eq!(c.0.len(), 2);
        assert_eq!(c.0[1].combinator, Some(Combinator::Descendant));
    }

    #[test]
    fn child() {
        assert_eq!(
            first_complex("div > span").0[1].combinator,
            Some(Combinator::Child)
        );
    }
    #[test]
    fn adjacent() {
        assert_eq!(
            first_complex("h1 + p").0[1].combinator,
            Some(Combinator::AdjacentSibling)
        );
    }
    #[test]
    fn sibling() {
        assert_eq!(
            first_complex("h1 ~ p").0[1].combinator,
            Some(Combinator::GeneralSibling)
        );
    }

    #[test]
    fn selector_list_count() {
        assert_eq!(parse(".a, .b, .c").0.len(), 3);
    }

    #[test]
    fn deep_chain() {
        let c = first_complex("nav > ul > li > a");
        assert_eq!(c.0.len(), 4);
        for p in &c.0[1..] {
            assert_eq!(p.combinator, Some(Combinator::Child));
        }
    }

    #[test]
    fn display_output() {
        let src = ".foo, div > span";
        let rendered = alloc::format!("{}", parse(src));
        assert!(rendered.contains(".foo"));
        assert!(rendered.contains("div"));
        assert!(rendered.contains("span"));
    }
}
