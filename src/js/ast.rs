//! Abstract Syntax Tree node definitions.
//!
//! The AST closely follows the [ESTree spec](https://github.com/estree/estree)
//! with extensions for ECMAScript 2022 (class fields, static blocks, `#name`).
//!
//! All heap-allocated nodes use `Box<T>` and `Vec<T>` from the `alloc` crate,
//! so the entire AST lives in the caller-provided heap — correct for `no_std`.

use alloc::{boxed::Box, string::String, vec::Vec};

use super::span::Span;

// ─────────────────────────────────────────────────────────────────────────────
// Top-level
// ─────────────────────────────────────────────────────────────────────────────

/// The top-level parse result.
#[derive(Debug, Clone)]
pub struct Program {
    /// `Script` or `Module`.
    pub source_type: SourceType,
    /// Top-level statements and (if `Module`) import/export declarations.
    pub body: Vec<ProgramItem>,
    pub span: Span,
}

/// Whether the source text is a classic script or an ES module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceType {
    Script,
    Module,
}

/// An item at the top level of a program.
#[derive(Debug, Clone)]
pub enum ProgramItem {
    Stmt(Stmt),
    Import(ImportDecl),
    Export(ExportDecl),
}

// ─────────────────────────────────────────────────────────────────────────────
// Identifiers
// ─────────────────────────────────────────────────────────────────────────────

/// A bound or reference identifier.
#[derive(Debug, Clone)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

impl Ident {
    pub fn new(name: impl Into<String>, span: Span) -> Self {
        Self { name: name.into(), span }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Literals
// ─────────────────────────────────────────────────────────────────────────────

/// A JavaScript literal value.
#[derive(Debug, Clone)]
pub enum Lit {
    /// `42`, `3.14`, `0xFF`
    Number(f64),
    /// `"hello"`, `'world'`
    Str(String),
    /// `true` or `false`
    Bool(bool),
    /// `null`
    Null,
    /// `/abc/gi`
    Regex { pattern: String, flags: String },
    /// `9007199254740993n`
    BigInt(String),
}

// ─────────────────────────────────────────────────────────────────────────────
// Patterns (used in destructuring, function params)
// ─────────────────────────────────────────────────────────────────────────────

/// A binding or assignment pattern.
#[derive(Debug, Clone)]
pub enum Pat {
    /// `x`
    Ident(Ident),
    /// `[a, b, c]`
    Array(ArrayPat),
    /// `{ x, y: z }`
    Object(ObjectPat),
    /// `x = default_value`
    Assign(AssignPat),
    /// `...rest`
    Rest(RestPat),
}

impl Pat {
    pub fn span(&self) -> Span {
        match self {
            Self::Ident(i) => i.span,
            Self::Array(p) => p.span,
            Self::Object(p) => p.span,
            Self::Assign(p) => p.span,
            Self::Rest(p) => p.span,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ArrayPat {
    /// Elements, `None` for holes (e.g. `[, b]`).
    pub elements: Vec<Option<Pat>>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ObjectPat {
    pub props: Vec<ObjectPatProp>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ObjectPatProp {
    /// `key: pattern` or shorthand `key`
    Keyed(KeyedPatProp),
    /// `...rest`
    Rest(RestPat),
}

#[derive(Debug, Clone)]
pub struct KeyedPatProp {
    pub key: PropKey,
    pub value: Pat,
    pub computed: bool,
    pub shorthand: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct AssignPat {
    pub left: Box<Pat>,
    pub right: Box<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct RestPat {
    pub argument: Box<Pat>,
    pub span: Span,
}

// ─────────────────────────────────────────────────────────────────────────────
// Expressions
// ─────────────────────────────────────────────────────────────────────────────

/// Every possible JavaScript expression node.
#[derive(Debug, Clone)]
pub enum Expr {
    // ── Primary ──────────────────────────────────────────────────────────────
    This(Span),
    Ident(Ident),
    Lit(Lit, Span),
    /// Template literal: `` `hello ${name}!` ``
    Tpl(TemplateLit),
    /// Tagged template: `tag\`...\``
    TaggedTpl(TaggedTplExpr),

    // ── Compound primary ─────────────────────────────────────────────────────
    Array(ArrayExpr),
    Object(ObjectExpr),
    Function(FunctionExpr),
    Arrow(ArrowExpr),
    Class(ClassExpr),

    // ── Meta properties ───────────────────────────────────────────────────────
    /// `new.target`
    NewTarget(Span),
    /// `import.meta`
    ImportMeta(Span),
    /// `import(source)` — dynamic import
    Import(Box<Expr>, Span),

    // ── Member / call ─────────────────────────────────────────────────────────
    Member(MemberExpr),
    /// `a?.b`, `a?.[x]`, `a?.()`
    OptChain(OptChainExpr),
    Call(CallExpr),
    New(NewExpr),

    // ── Unary / update ────────────────────────────────────────────────────────
    Unary(UnaryExpr),
    Update(UpdateExpr),

    // ── Binary ───────────────────────────────────────────────────────────────
    Binary(BinaryExpr),
    Logical(LogicalExpr),
    /// `cond ? then : else`
    Cond(CondExpr),

    // ── Assignment ────────────────────────────────────────────────────────────
    Assign(AssignExpr),

    // ── Sequence ──────────────────────────────────────────────────────────────
    Seq(SeqExpr),

    // ── Async / generator ─────────────────────────────────────────────────────
    Yield(YieldExpr),
    Await(AwaitExpr),

    // ── Spread (used inside call args / array / object literals) ──────────────
    Spread(SpreadExpr),
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Self::This(s)         => *s,
            Self::Ident(i)        => i.span,
            Self::Lit(_, s)       => *s,
            Self::Tpl(t)          => t.span,
            Self::TaggedTpl(t)    => t.span,
            Self::Array(a)        => a.span,
            Self::Object(o)       => o.span,
            Self::Function(f)     => f.span,
            Self::Arrow(a)        => a.span,
            Self::Class(c)        => c.span,
            Self::NewTarget(s)    => *s,
            Self::ImportMeta(s)   => *s,
            Self::Import(_, s)    => *s,
            Self::Member(m)       => m.span,
            Self::OptChain(o)     => o.span,
            Self::Call(c)         => c.span,
            Self::New(n)          => n.span,
            Self::Unary(u)        => u.span,
            Self::Update(u)       => u.span,
            Self::Binary(b)       => b.span,
            Self::Logical(l)      => l.span,
            Self::Cond(c)         => c.span,
            Self::Assign(a)       => a.span,
            Self::Seq(s)          => s.span,
            Self::Yield(y)        => y.span,
            Self::Await(a)        => a.span,
            Self::Spread(s)       => s.span,
        }
    }

    /// `true` if this expression is a valid assignment target (`=`).
    pub fn is_lvalue(&self) -> bool {
        matches!(self,
            Self::Ident(_)
            | Self::Member(_)
            | Self::OptChain(_)
            | Self::Array(_)   // destructuring assignment
            | Self::Object(_)  // destructuring assignment
        )
    }
}

// ── Template literals ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TemplateLit {
    pub quasis: Vec<TemplateElement>,
    pub exprs: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TemplateElement {
    pub cooked: String,
    pub tail: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TaggedTplExpr {
    pub tag: Box<Expr>,
    pub quasi: TemplateLit,
    pub span: Span,
}

// ── Array / object expressions ────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ArrayExpr {
    /// `None` entries represent holes: `[, 1, , 2]`.
    pub elements: Vec<Option<Expr>>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ObjectExpr {
    pub props: Vec<ObjectProp>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ObjectProp {
    /// `key: value`, shorthand `x`, or computed `[expr]: value`
    Keyed(KeyedProp),
    /// `get key() { … }` / `set key(v) { … }`
    Method(MethodProp),
    /// `...spread`
    Spread(SpreadExpr),
}

#[derive(Debug, Clone)]
pub struct KeyedProp {
    pub key: PropKey,
    pub value: Expr,
    pub computed: bool,
    pub shorthand: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MethodProp {
    pub key: PropKey,
    pub function: Function,
    pub kind: MethodKind,
    pub computed: bool,
    pub span: Span,
}

/// The key of an object property or class member.
#[derive(Debug, Clone)]
pub enum PropKey {
    Ident(Ident),
    Str(String, Span),
    Number(f64, Span),
    Computed(Box<Expr>),
    Private(Ident), // `#name`
}

impl PropKey {
    pub fn span(&self) -> Span {
        match self {
            Self::Ident(i) => i.span,
            Self::Str(_, s) | Self::Number(_, s) => *s,
            Self::Computed(e) => e.span(),
            Self::Private(i) => i.span,
        }
    }
}

// ── Functions ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FunctionExpr {
    pub id: Option<Ident>,
    pub function: Function,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub params: Vec<Param>,
    pub body: BlockStmt,
    pub is_async: bool,
    pub is_generator: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub pat: Pat,
    pub span: Span,
}

// ── Arrow functions ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ArrowExpr {
    pub params: Vec<Param>,
    pub body: ArrowBody,
    pub is_async: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ArrowBody {
    Expr(Box<Expr>),
    Block(BlockStmt),
}

// ── Class expressions ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ClassExpr {
    pub id: Option<Ident>,
    pub super_class: Option<Box<Expr>>,
    pub body: ClassBody,
    pub span: Span,
}

// ── Member expressions ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MemberExpr {
    pub object: Box<Expr>,
    pub prop: MemberProp,
    pub computed: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum MemberProp {
    Ident(Ident),
    Computed(Box<Expr>),
    Private(Ident),
}

// ── Optional chaining ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct OptChainExpr {
    pub base: Box<Expr>,
    pub chain: OptChain,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum OptChain {
    /// `?.prop`
    Member(Ident),
    /// `?.[expr]`
    ComputedMember(Box<Expr>),
    /// `?.(args)`
    Call(Vec<Expr>),
}

// ── Call / new ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CallExpr {
    pub callee: Box<Expr>,
    pub args: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct NewExpr {
    pub callee: Box<Expr>,
    /// `None` when no argument list is present: `new Foo`.
    pub args: Option<Vec<Expr>>,
    pub span: Span,
}

// ── Unary / update ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct UnaryExpr {
    pub op: UnaryOp,
    pub argument: Box<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    /// `-`
    Neg,
    /// `+`
    Pos,
    /// `!`
    Not,
    /// `~`
    BitNot,
    /// `typeof`
    Typeof,
    /// `void`
    Void,
    /// `delete`
    Delete,
}

#[derive(Debug, Clone)]
pub struct UpdateExpr {
    pub op: UpdateOp,
    pub argument: Box<Expr>,
    pub prefix: bool,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateOp { Inc, Dec }

// ── Binary / logical ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BinaryExpr {
    pub op: BinaryOp,
    pub left: Box<Expr>,
    pub right: Box<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    // Arithmetic
    Add, Sub, Mul, Div, Rem, Pow,
    // Bitwise
    BitAnd, BitOr, BitXor, Shl, Shr, UShr,
    // Comparison
    Eq, NotEq, StrictEq, StrictNotEq,
    Lt, LtEq, Gt, GtEq,
    // Relational
    In, Instanceof,
}

#[derive(Debug, Clone)]
pub struct LogicalExpr {
    pub op: LogicalOp,
    pub left: Box<Expr>,
    pub right: Box<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalOp {
    /// `&&`
    And,
    /// `||`
    Or,
    /// `??`
    Nullish,
}

// ── Conditional ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CondExpr {
    pub test: Box<Expr>,
    pub consequent: Box<Expr>,
    pub alternate: Box<Expr>,
    pub span: Span,
}

// ── Assignment ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AssignExpr {
    pub op: AssignOp,
    pub left: AssignTarget,
    pub right: Box<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    Assign,
    AddAssign, SubAssign, MulAssign, DivAssign, RemAssign, PowAssign,
    BitAndAssign, BitOrAssign, BitXorAssign,
    ShlAssign, ShrAssign, UShrAssign,
    AndAssign, OrAssign, NullishAssign,
}

#[derive(Debug, Clone)]
pub enum AssignTarget {
    /// Simple identifier or member expression.
    Simple(Box<Expr>),
    /// Destructuring: `[a, b] = ...` or `{ x } = ...`.
    Destructure(Pat),
}

// ── Sequence ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SeqExpr {
    pub exprs: Vec<Expr>,
    pub span: Span,
}

// ── Yield / await ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct YieldExpr {
    pub argument: Option<Box<Expr>>,
    pub delegate: bool, // `yield*`
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct AwaitExpr {
    pub argument: Box<Expr>,
    pub span: Span,
}

// ── Spread ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SpreadExpr {
    pub argument: Box<Expr>,
    pub span: Span,
}

// ─────────────────────────────────────────────────────────────────────────────
// Statements
// ─────────────────────────────────────────────────────────────────────────────

/// Every possible JavaScript statement node.
#[derive(Debug, Clone)]
pub enum Stmt {
    /// `{ … }`
    Block(BlockStmt),
    /// `;`
    Empty(Span),
    /// An expression used as a statement.
    Expr(ExprStmt),
    /// `if (test) cons [else alt]`
    If(IfStmt),
    /// `while (test) body`
    While(WhileStmt),
    /// `do body while (test);`
    DoWhile(DoWhileStmt),
    /// `for (…) body`
    For(ForStmt),
    /// `for (lhs in rhs) body`
    ForIn(ForInStmt),
    /// `for (lhs of rhs) body`
    ForOf(ForOfStmt),
    /// `switch (disc) { cases }`
    Switch(SwitchStmt),
    /// `break [label];`
    Break(BreakStmt),
    /// `continue [label];`
    Continue(ContinueStmt),
    /// `return [value];`
    Return(ReturnStmt),
    /// `throw expr;`
    Throw(ThrowStmt),
    /// `try { } catch (e) { } finally { }`
    Try(TryStmt),
    /// `with (obj) body`
    With(WithStmt),
    /// `label: body`
    Label(LabeledStmt),
    /// `debugger;`
    Debugger(Span),
    /// Declaration used as a statement.
    Decl(Decl),
}

impl Stmt {
    pub fn span(&self) -> Span {
        match self {
            Self::Block(s)    => s.span,
            Self::Empty(s)    => *s,
            Self::Expr(s)     => s.span,
            Self::If(s)       => s.span,
            Self::While(s)    => s.span,
            Self::DoWhile(s)  => s.span,
            Self::For(s)      => s.span,
            Self::ForIn(s)    => s.span,
            Self::ForOf(s)    => s.span,
            Self::Switch(s)   => s.span,
            Self::Break(s)    => s.span,
            Self::Continue(s) => s.span,
            Self::Return(s)   => s.span,
            Self::Throw(s)    => s.span,
            Self::Try(s)      => s.span,
            Self::With(s)     => s.span,
            Self::Label(s)    => s.span,
            Self::Debugger(s) => *s,
            Self::Decl(d)     => d.span(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BlockStmt {
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ExprStmt {
    pub expr: Expr,
    pub span: Span,
    /// Directive prologues: `"use strict";`
    pub directive: Option<String>,
}

#[derive(Debug, Clone)]
pub struct IfStmt {
    pub test: Expr,
    pub consequent: Box<Stmt>,
    pub alternate: Option<Box<Stmt>>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct WhileStmt {
    pub test: Expr,
    pub body: Box<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct DoWhileStmt {
    pub body: Box<Stmt>,
    pub test: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ForStmt {
    pub init: Option<ForInit>,
    pub test: Option<Expr>,
    pub update: Option<Expr>,
    pub body: Box<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ForInit {
    Decl(VarDecl),
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub struct ForInStmt {
    pub left: ForInLeft,
    pub right: Expr,
    pub body: Box<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ForOfStmt {
    pub left: ForInLeft,
    pub right: Expr,
    pub body: Box<Stmt>,
    pub is_await: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ForInLeft {
    Decl(VarDecl),
    Pat(Pat),
}

#[derive(Debug, Clone)]
pub struct SwitchStmt {
    pub discriminant: Expr,
    pub cases: Vec<SwitchCase>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct SwitchCase {
    /// `None` for `default:`.
    pub test: Option<Expr>,
    pub consequent: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct BreakStmt {
    pub label: Option<Ident>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ContinueStmt {
    pub label: Option<Ident>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ReturnStmt {
    pub argument: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ThrowStmt {
    pub argument: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TryStmt {
    pub block: BlockStmt,
    pub handler: Option<CatchClause>,
    pub finalizer: Option<BlockStmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct CatchClause {
    /// `catch (e)` — `None` for optional-catch `catch { }`.
    pub param: Option<Pat>,
    pub body: BlockStmt,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct WithStmt {
    pub object: Expr,
    pub body: Box<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct LabeledStmt {
    pub label: Ident,
    pub body: Box<Stmt>,
    pub span: Span,
}

// ─────────────────────────────────────────────────────────────────────────────
// Declarations
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Decl {
    Var(VarDecl),
    Fn(FnDecl),
    Class(ClassDecl),
}

impl Decl {
    pub fn span(&self) -> Span {
        match self {
            Self::Var(d)   => d.span,
            Self::Fn(d)    => d.span,
            Self::Class(d) => d.span,
        }
    }
}

// ── Variable declarations ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct VarDecl {
    pub kind: VarKind,
    pub decls: Vec<VarDeclarator>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarKind { Var, Let, Const }

#[derive(Debug, Clone)]
pub struct VarDeclarator {
    pub id: Pat,
    pub init: Option<Expr>,
    pub span: Span,
}

// ── Function declarations ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FnDecl {
    /// `None` only for `export default function() {}`.
    pub id: Option<Ident>,
    pub function: Function,
    pub span: Span,
}

// ── Class declarations ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ClassDecl {
    /// `None` only for `export default class {}`.
    pub id: Option<Ident>,
    pub super_class: Option<Box<Expr>>,
    pub body: ClassBody,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ClassBody {
    pub body: Vec<ClassMember>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ClassMember {
    /// `methodName() { }`, `get prop() { }`, `async *gen() { }`
    Method(ClassMethod),
    /// `fieldName = value;` — ES2022 class fields
    Field(ClassField),
    /// `static { … }` — ES2022 static initialisation block
    StaticBlock(StaticBlock),
}

#[derive(Debug, Clone)]
pub struct ClassMethod {
    pub key: PropKey,
    pub value: Function,
    pub kind: MethodKind,
    pub computed: bool,
    pub is_static: bool,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodKind {
    Constructor,
    Method,
    Get,
    Set,
}

#[derive(Debug, Clone)]
pub struct ClassField {
    pub key: PropKey,
    pub value: Option<Expr>,
    pub computed: bool,
    pub is_static: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct StaticBlock {
    pub body: Vec<Stmt>,
    pub span: Span,
}

// ─────────────────────────────────────────────────────────────────────────────
// Module declarations (import / export)
// ─────────────────────────────────────────────────────────────────────────────

// ── Import ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ImportDecl {
    pub specifiers: Vec<ImportSpec>,
    pub source: String,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ImportSpec {
    /// `import defaultExport from "…"`
    Default(ImportDefaultSpec),
    /// `import * as ns from "…"`
    Namespace(ImportNamespaceSpec),
    /// `import { foo, bar as baz } from "…"`
    Named(ImportNamedSpec),
}

#[derive(Debug, Clone)]
pub struct ImportDefaultSpec {
    pub local: Ident,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ImportNamespaceSpec {
    pub local: Ident,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ImportNamedSpec {
    pub imported: ModuleExportName,
    pub local: Ident,
    pub span: Span,
}

/// The name used in `{ foo as bar }` — can be a string literal in ES2022.
#[derive(Debug, Clone)]
pub enum ModuleExportName {
    Ident(Ident),
    Str(String, Span),
}

// ── Export ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ExportDecl {
    /// `export { foo, bar as baz } [from "…"]`
    Named(ExportNamedDecl),
    /// `export default expr` or `export default function … {}`
    Default(ExportDefaultDecl),
    /// `export * from "…"` or `export * as ns from "…"`
    All(ExportAllDecl),
    /// `export const …`, `export function …`, `export class …`
    Decl(Decl, Span),
}

#[derive(Debug, Clone)]
pub struct ExportNamedDecl {
    pub specifiers: Vec<ExportSpec>,
    /// Present when `from "source"` is used.
    pub source: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ExportSpec {
    pub local: ModuleExportName,
    pub exported: ModuleExportName,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ExportDefaultDecl {
    pub declaration: ExportDefault,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ExportDefault {
    Fn(FnDecl),
    Class(ClassDecl),
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub struct ExportAllDecl {
    pub exported: Option<ModuleExportName>,
    pub source: String,
    pub span: Span,
}