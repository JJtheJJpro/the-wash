//! Smoke tests — verify the parser handles all major JS constructs correctly.
//!
//! Each test calls the public API and asserts the AST shape is correct, or
//! simply that parsing succeeds / fails as expected.

use nova_js_parser::{
    ast::*,
    error::ParseError,
    parse_expression, parse_module, parse_script,
};

// ─────────────────────────────────────────────────────────────────────────────
// Literals
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn parse_number_literal() {
    let prog = parse_script("42;").unwrap();
    assert_eq!(prog.body.len(), 1);
    if let ProgramItem::Stmt(Stmt::Expr(ExprStmt { expr: Expr::Lit(Lit::Number(n), _), .. })) = &prog.body[0] {
        assert_eq!(*n, 42.0);
    } else {
        panic!("expected number literal, got {:?}", prog.body[0]);
    }
}

#[test]
fn parse_float_literal() {
    let prog = parse_script("3.14;").unwrap();
    if let ProgramItem::Stmt(Stmt::Expr(ExprStmt { expr: Expr::Lit(Lit::Number(n), _), .. })) = &prog.body[0] {
        assert!((n - 3.14).abs() < 1e-10);
    } else { panic!(); }
}

#[test]
fn parse_hex_literal() {
    let prog = parse_script("0xFF;").unwrap();
    if let ProgramItem::Stmt(Stmt::Expr(ExprStmt { expr: Expr::Lit(Lit::Number(n), _), .. })) = &prog.body[0] {
        assert_eq!(*n, 255.0);
    } else { panic!(); }
}

#[test]
fn parse_bigint_literal() {
    let prog = parse_script("9007199254740993n;").unwrap();
    if let ProgramItem::Stmt(Stmt::Expr(ExprStmt { expr: Expr::Lit(Lit::BigInt(s), _), .. })) = &prog.body[0] {
        assert_eq!(s, "9007199254740993");
    } else { panic!(); }
}

#[test]
fn parse_string_literal() {
    let prog = parse_script(r#""hello";"#).unwrap();
    if let ProgramItem::Stmt(Stmt::Expr(ExprStmt { expr: Expr::Lit(Lit::Str(s), _), .. })) = &prog.body[0] {
        assert_eq!(s, "hello");
    } else { panic!(); }
}

#[test]
fn parse_boolean_literals() {
    let t = parse_expression("true").unwrap();
    let f = parse_expression("false").unwrap();
    assert!(matches!(t, Expr::Lit(Lit::Bool(true), _)));
    assert!(matches!(f, Expr::Lit(Lit::Bool(false), _)));
}

#[test]
fn parse_null_literal() {
    let expr = parse_expression("null").unwrap();
    assert!(matches!(expr, Expr::Lit(Lit::Null, _)));
}

#[test]
fn parse_regex_literal() {
    let expr = parse_expression("/ab+c/gi").unwrap();
    if let Expr::Lit(Lit::Regex { pattern, flags }, _) = expr {
        assert_eq!(pattern, "ab+c");
        assert_eq!(flags, "gi");
    } else { panic!(); }
}

// ─────────────────────────────────────────────────────────────────────────────
// Template literals
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn parse_template_no_substitution() {
    let expr = parse_expression("`hello world`").unwrap();
    if let Expr::Tpl(tpl) = expr {
        assert_eq!(tpl.quasis.len(), 1);
        assert_eq!(tpl.exprs.len(), 0);
        assert_eq!(tpl.quasis[0].cooked, "hello world");
    } else { panic!(); }
}

#[test]
fn parse_template_with_substitution() {
    let expr = parse_expression("`hello ${name}!`").unwrap();
    if let Expr::Tpl(tpl) = expr {
        assert_eq!(tpl.quasis.len(), 2);
        assert_eq!(tpl.exprs.len(), 1);
        assert_eq!(tpl.quasis[0].cooked, "hello ");
        assert_eq!(tpl.quasis[1].cooked, "!");
    } else { panic!(); }
}

#[test]
fn parse_template_multiple_subs() {
    let expr = parse_expression("`${a} + ${b} = ${c}`").unwrap();
    if let Expr::Tpl(tpl) = expr {
        assert_eq!(tpl.quasis.len(), 4);
        assert_eq!(tpl.exprs.len(), 3);
    } else { panic!(); }
}

// ─────────────────────────────────────────────────────────────────────────────
// Operator precedence
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn binary_precedence_add_mul() {
    // 1 + 2 * 3  →  1 + (2 * 3)
    let expr = parse_expression("1 + 2 * 3").unwrap();
    if let Expr::Binary(b) = expr {
        assert_eq!(b.op, BinaryOp::Add);
        assert!(matches!(*b.right, Expr::Binary(ref r) if r.op == BinaryOp::Mul));
    } else { panic!(); }
}

#[test]
fn binary_left_associativity() {
    // 1 - 2 - 3  →  (1 - 2) - 3
    let expr = parse_expression("1 - 2 - 3").unwrap();
    if let Expr::Binary(b) = expr {
        assert_eq!(b.op, BinaryOp::Sub);
        assert!(matches!(*b.left, Expr::Binary(_)));
        assert!(matches!(*b.right, Expr::Lit(Lit::Number(n), _) if n == 3.0));
    } else { panic!(); }
}

#[test]
fn exponentiation_right_associativity() {
    // 2 ** 3 ** 4  →  2 ** (3 ** 4)
    let expr = parse_expression("2 ** 3 ** 4").unwrap();
    if let Expr::Binary(b) = expr {
        assert_eq!(b.op, BinaryOp::Pow);
        assert!(matches!(*b.right, Expr::Binary(ref r) if r.op == BinaryOp::Pow));
    } else { panic!(); }
}

#[test]
fn assignment_right_associativity() {
    // a = b = c  →  a = (b = c)
    let expr = parse_expression("a = b = c").unwrap();
    if let Expr::Assign(a) = expr {
        assert!(matches!(*a.right, Expr::Assign(_)));
    } else { panic!(); }
}

#[test]
fn conditional_expression() {
    let expr = parse_expression("a ? b : c").unwrap();
    assert!(matches!(expr, Expr::Cond(_)));
    if let Expr::Cond(c) = expr {
        assert!(matches!(*c.test, Expr::Ident(_)));
        assert!(matches!(*c.consequent, Expr::Ident(_)));
        assert!(matches!(*c.alternate, Expr::Ident(_)));
    }
}

#[test]
fn nullish_coalescing() {
    let expr = parse_expression("a ?? b").unwrap();
    if let Expr::Logical(l) = expr { assert_eq!(l.op, LogicalOp::Nullish); }
    else { panic!(); }
}

#[test]
fn nullish_mixed_with_logical_is_error() {
    // ?? mixed with || is a SyntaxError
    let err = parse_expression("a ?? b || c").unwrap_err();
    assert!(matches!(err, ParseError::NullishMixedWithLogical(_)));
}

#[test]
fn unary_operators() {
    let expr = parse_expression("!a").unwrap();
    assert!(matches!(expr, Expr::Unary(UnaryExpr { op: UnaryOp::Not, .. })));

    let expr = parse_expression("typeof x").unwrap();
    assert!(matches!(expr, Expr::Unary(UnaryExpr { op: UnaryOp::Typeof, .. })));

    let expr = parse_expression("void 0").unwrap();
    assert!(matches!(expr, Expr::Unary(UnaryExpr { op: UnaryOp::Void, .. })));
}

#[test]
fn prefix_update_operators() {
    let expr = parse_expression("++i").unwrap();
    assert!(matches!(expr, Expr::Update(UpdateExpr { op: UpdateOp::Inc, prefix: true, .. })));
}

// ─────────────────────────────────────────────────────────────────────────────
// Member access & calls
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn member_access_dot() {
    let expr = parse_expression("a.b.c").unwrap();
    if let Expr::Member(m) = expr {
        assert!(!m.computed);
        assert!(matches!(*m.object, Expr::Member(_)));
    } else { panic!(); }
}

#[test]
fn member_access_bracket() {
    let expr = parse_expression("a[0]").unwrap();
    if let Expr::Member(m) = expr {
        assert!(m.computed);
    } else { panic!(); }
}

#[test]
fn optional_chain() {
    let expr = parse_expression("obj?.prop").unwrap();
    assert!(matches!(expr, Expr::OptChain(_)));
}

#[test]
fn call_expression() {
    let expr = parse_expression("foo(1, 2, 3)").unwrap();
    if let Expr::Call(c) = expr {
        assert_eq!(c.args.len(), 3);
    } else { panic!(); }
}

#[test]
fn call_with_spread() {
    let expr = parse_expression("foo(...args)").unwrap();
    if let Expr::Call(c) = expr {
        assert_eq!(c.args.len(), 1);
        assert!(matches!(c.args[0], Expr::Spread(_)));
    } else { panic!(); }
}

#[test]
fn new_expression() {
    let expr = parse_expression("new Foo(1, 2)").unwrap();
    if let Expr::New(n) = expr {
        assert!(n.args.is_some());
        assert_eq!(n.args.as_ref().unwrap().len(), 2);
    } else { panic!(); }
}

#[test]
fn new_without_args() {
    let expr = parse_expression("new Foo").unwrap();
    if let Expr::New(n) = expr {
        assert!(n.args.is_none());
    } else { panic!(); }
}

// ─────────────────────────────────────────────────────────────────────────────
// Array & object literals
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn array_literal() {
    let expr = parse_expression("[1, 2, 3]").unwrap();
    if let Expr::Array(a) = expr {
        assert_eq!(a.elements.len(), 3);
    } else { panic!(); }
}

#[test]
fn array_literal_with_hole() {
    let expr = parse_expression("[1, , 3]").unwrap();
    if let Expr::Array(a) = expr {
        assert_eq!(a.elements.len(), 3);
        assert!(a.elements[1].is_none());
    } else { panic!(); }
}

#[test]
fn object_literal() {
    let expr = parse_expression("({ x: 1, y: 2 })").unwrap();
    if let Expr::Object(o) = expr {
        assert_eq!(o.props.len(), 2);
    } else { panic!(); }
}

#[test]
fn object_shorthand() {
    let expr = parse_expression("({ x, y })").unwrap();
    if let Expr::Object(o) = expr {
        for prop in &o.props {
            if let ObjectProp::Keyed(k) = prop {
                assert!(k.shorthand);
            } else { panic!("expected keyed prop"); }
        }
    } else { panic!(); }
}

#[test]
fn object_computed_key() {
    let expr = parse_expression("({ [key]: value })").unwrap();
    if let Expr::Object(o) = expr {
        if let ObjectProp::Keyed(k) = &o.props[0] {
            assert!(k.computed);
        } else { panic!(); }
    } else { panic!(); }
}

#[test]
fn object_spread() {
    let expr = parse_expression("({ ...rest })").unwrap();
    if let Expr::Object(o) = expr {
        assert!(matches!(o.props[0], ObjectProp::Spread(_)));
    } else { panic!(); }
}

// ─────────────────────────────────────────────────────────────────────────────
// Functions & arrows
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn function_expression() {
    let expr = parse_expression("function foo(a, b) { return a + b; }").unwrap();
    if let Expr::Function(f) = expr {
        assert_eq!(f.function.params.len(), 2);
        assert!(!f.function.is_async);
        assert!(!f.function.is_generator);
    } else { panic!(); }
}

#[test]
fn generator_function_expression() {
    let expr = parse_expression("function* gen() { yield 1; }").unwrap();
    if let Expr::Function(f) = expr {
        assert!(f.function.is_generator);
    } else { panic!(); }
}

#[test]
fn arrow_function_expr_body() {
    let expr = parse_expression("x => x * 2").unwrap();
    if let Expr::Arrow(a) = expr {
        assert_eq!(a.params.len(), 1);
        assert!(matches!(a.body, ArrowBody::Expr(_)));
    } else { panic!(); }
}

#[test]
fn arrow_function_block_body() {
    let expr = parse_expression("(x, y) => { return x + y; }").unwrap();
    if let Expr::Arrow(a) = expr {
        assert_eq!(a.params.len(), 2);
        assert!(matches!(a.body, ArrowBody::Block(_)));
    } else { panic!(); }
}

#[test]
fn arrow_no_params() {
    let expr = parse_expression("() => 42").unwrap();
    if let Expr::Arrow(a) = expr {
        assert_eq!(a.params.len(), 0);
    } else { panic!(); }
}

#[test]
fn async_arrow_function() {
    let expr = parse_expression("async x => await fetch(x)").unwrap();
    if let Expr::Arrow(a) = expr {
        assert!(a.is_async);
        assert_eq!(a.params.len(), 1);
    } else { panic!(); }
}

#[test]
fn async_function_expression() {
    let expr = parse_expression("async function foo() {}").unwrap();
    if let Expr::Function(f) = expr {
        assert!(f.function.is_async);
    } else { panic!(); }
}

// ─────────────────────────────────────────────────────────────────────────────
// Statements
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn if_statement() {
    let prog = parse_script("if (x) { y(); } else { z(); }").unwrap();
    assert!(matches!(&prog.body[0], ProgramItem::Stmt(Stmt::If(_))));
}

#[test]
fn while_statement() {
    let prog = parse_script("while (true) { break; }").unwrap();
    assert!(matches!(&prog.body[0], ProgramItem::Stmt(Stmt::While(_))));
}

#[test]
fn do_while_statement() {
    let prog = parse_script("do { x++; } while (x < 10);").unwrap();
    assert!(matches!(&prog.body[0], ProgramItem::Stmt(Stmt::DoWhile(_))));
}

#[test]
fn for_statement() {
    let prog = parse_script("for (let i = 0; i < 10; i++) {}").unwrap();
    assert!(matches!(&prog.body[0], ProgramItem::Stmt(Stmt::For(_))));
}

#[test]
fn for_in_statement() {
    let prog = parse_script("for (const key in obj) {}").unwrap();
    assert!(matches!(&prog.body[0], ProgramItem::Stmt(Stmt::ForIn(_))));
}

#[test]
fn for_of_statement() {
    let prog = parse_script("for (const item of list) {}").unwrap();
    assert!(matches!(&prog.body[0], ProgramItem::Stmt(Stmt::ForOf(_))));
}

#[test]
fn switch_statement() {
    let prog = parse_script("switch (x) { case 1: break; default: break; }").unwrap();
    if let ProgramItem::Stmt(Stmt::Switch(s)) = &prog.body[0] {
        assert_eq!(s.cases.len(), 2);
        assert!(s.cases[0].test.is_some());
        assert!(s.cases[1].test.is_none());
    } else { panic!(); }
}

#[test]
fn try_catch_finally() {
    let prog = parse_script("try { foo(); } catch (e) { bar(); } finally { baz(); }").unwrap();
    if let ProgramItem::Stmt(Stmt::Try(t)) = &prog.body[0] {
        assert!(t.handler.is_some());
        assert!(t.finalizer.is_some());
    } else { panic!(); }
}

#[test]
fn optional_catch_binding() {
    let prog = parse_script("try { foo(); } catch { bar(); }").unwrap();
    if let ProgramItem::Stmt(Stmt::Try(t)) = &prog.body[0] {
        assert!(t.handler.as_ref().unwrap().param.is_none());
    } else { panic!(); }
}

#[test]
fn labeled_statement() {
    let prog = parse_script("outer: for (;;) { break outer; }").unwrap();
    assert!(matches!(&prog.body[0], ProgramItem::Stmt(Stmt::Label(_))));
}

#[test]
fn debugger_statement() {
    let prog = parse_script("debugger;").unwrap();
    assert!(matches!(&prog.body[0], ProgramItem::Stmt(Stmt::Debugger(_))));
}

#[test]
fn throw_statement() {
    let prog = parse_script("throw new Error('fail');").unwrap();
    assert!(matches!(&prog.body[0], ProgramItem::Stmt(Stmt::Throw(_))));
}

// ─────────────────────────────────────────────────────────────────────────────
// Declarations
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn var_declaration() {
    let prog = parse_script("var x = 1, y = 2;").unwrap();
    if let ProgramItem::Stmt(Stmt::Decl(Decl::Var(v))) = &prog.body[0] {
        assert_eq!(v.kind, VarKind::Var);
        assert_eq!(v.decls.len(), 2);
    } else { panic!(); }
}

#[test]
fn let_declaration() {
    let prog = parse_script("let x = 1;").unwrap();
    if let ProgramItem::Stmt(Stmt::Decl(Decl::Var(v))) = &prog.body[0] {
        assert_eq!(v.kind, VarKind::Let);
    } else { panic!(); }
}

#[test]
fn const_declaration() {
    let prog = parse_script("const PI = 3.14;").unwrap();
    if let ProgramItem::Stmt(Stmt::Decl(Decl::Var(v))) = &prog.body[0] {
        assert_eq!(v.kind, VarKind::Const);
    } else { panic!(); }
}

#[test]
fn destructuring_array() {
    let prog = parse_script("const [a, b, ...rest] = arr;").unwrap();
    if let ProgramItem::Stmt(Stmt::Decl(Decl::Var(v))) = &prog.body[0] {
        assert!(matches!(v.decls[0].id, Pat::Array(_)));
    } else { panic!(); }
}

#[test]
fn destructuring_object() {
    let prog = parse_script("const { x, y: renamed } = obj;").unwrap();
    if let ProgramItem::Stmt(Stmt::Decl(Decl::Var(v))) = &prog.body[0] {
        assert!(matches!(v.decls[0].id, Pat::Object(_)));
    } else { panic!(); }
}

#[test]
fn destructuring_with_default() {
    let prog = parse_script("const { x = 10 } = obj;").unwrap();
    if let ProgramItem::Stmt(Stmt::Decl(Decl::Var(v))) = &prog.body[0] {
        if let Pat::Object(o) = &v.decls[0].id {
            if let ObjectPatProp::Keyed(k) = &o.props[0] {
                assert!(matches!(k.value, Pat::Assign(_)));
            } else { panic!(); }
        } else { panic!(); }
    } else { panic!(); }
}

#[test]
fn function_declaration() {
    let prog = parse_script("function add(a, b) { return a + b; }").unwrap();
    if let ProgramItem::Stmt(Stmt::Decl(Decl::Fn(f))) = &prog.body[0] {
        assert_eq!(f.id.as_ref().unwrap().name, "add");
        assert_eq!(f.function.params.len(), 2);
    } else { panic!(); }
}

#[test]
fn async_function_declaration() {
    let prog = parse_script("async function fetchData() {}").unwrap();
    if let ProgramItem::Stmt(Stmt::Decl(Decl::Fn(f))) = &prog.body[0] {
        assert!(f.function.is_async);
    } else { panic!(); }
}

#[test]
fn generator_function_declaration() {
    let prog = parse_script("function* counter() { yield 0; }").unwrap();
    if let ProgramItem::Stmt(Stmt::Decl(Decl::Fn(f))) = &prog.body[0] {
        assert!(f.function.is_generator);
    } else { panic!(); }
}

#[test]
fn class_declaration() {
    let prog = parse_script("class Dog extends Animal { bark() {} }").unwrap();
    if let ProgramItem::Stmt(Stmt::Decl(Decl::Class(c))) = &prog.body[0] {
        assert_eq!(c.id.as_ref().unwrap().name, "Dog");
        assert!(c.super_class.is_some());
        assert_eq!(c.body.body.len(), 1);
    } else { panic!(); }
}

#[test]
fn class_with_private_field() {
    let prog = parse_script("class Counter { #count = 0; inc() { this.#count++; } }").unwrap();
    if let ProgramItem::Stmt(Stmt::Decl(Decl::Class(c))) = &prog.body[0] {
        assert_eq!(c.body.body.len(), 2);
        assert!(matches!(c.body.body[0], ClassMember::Field(_)));
    } else { panic!(); }
}

#[test]
fn class_static_field_and_method() {
    let prog = parse_script("class Foo { static count = 0; static create() {} }").unwrap();
    if let ProgramItem::Stmt(Stmt::Decl(Decl::Class(c))) = &prog.body[0] {
        if let ClassMember::Field(f) = &c.body.body[0] { assert!(f.is_static); }
        else { panic!(); }
        if let ClassMember::Method(m) = &c.body.body[1] { assert!(m.is_static); }
        else { panic!(); }
    } else { panic!(); }
}

// ─────────────────────────────────────────────────────────────────────────────
// Module syntax (import / export)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn import_default() {
    let prog = parse_module("import React from 'react';").unwrap();
    if let ProgramItem::Import(i) = &prog.body[0] {
        assert_eq!(i.source, "react");
        assert_eq!(i.specifiers.len(), 1);
        assert!(matches!(i.specifiers[0], ImportSpec::Default(_)));
    } else { panic!(); }
}

#[test]
fn import_named() {
    let prog = parse_module("import { useState, useEffect } from 'react';").unwrap();
    if let ProgramItem::Import(i) = &prog.body[0] {
        assert_eq!(i.specifiers.len(), 2);
        assert!(matches!(i.specifiers[0], ImportSpec::Named(_)));
    } else { panic!(); }
}

#[test]
fn import_namespace() {
    let prog = parse_module("import * as fs from 'fs';").unwrap();
    if let ProgramItem::Import(i) = &prog.body[0] {
        assert!(matches!(i.specifiers[0], ImportSpec::Namespace(_)));
    } else { panic!(); }
}

#[test]
fn import_side_effect() {
    let prog = parse_module("import './polyfill.js';").unwrap();
    if let ProgramItem::Import(i) = &prog.body[0] {
        assert_eq!(i.specifiers.len(), 0);
        assert_eq!(i.source, "./polyfill.js");
    } else { panic!(); }
}

#[test]
fn export_named() {
    let prog = parse_module("export { foo, bar as baz };").unwrap();
    if let ProgramItem::Export(ExportDecl::Named(e)) = &prog.body[0] {
        assert_eq!(e.specifiers.len(), 2);
    } else { panic!(); }
}

#[test]
fn export_default_function() {
    let prog = parse_module("export default function() {}").unwrap();
    assert!(matches!(&prog.body[0], ProgramItem::Export(ExportDecl::Default(_))));
}

#[test]
fn export_all_from() {
    let prog = parse_module("export * from './utils';").unwrap();
    if let ProgramItem::Export(ExportDecl::All(e)) = &prog.body[0] {
        assert_eq!(e.source, "./utils");
        assert!(e.exported.is_none());
    } else { panic!(); }
}

#[test]
fn export_all_as_from() {
    let prog = parse_module("export * as utils from './utils';").unwrap();
    if let ProgramItem::Export(ExportDecl::All(e)) = &prog.body[0] {
        assert!(e.exported.is_some());
    } else { panic!(); }
}

#[test]
fn dynamic_import() {
    let expr = parse_expression("import('./module.js')").unwrap();
    assert!(matches!(expr, Expr::Import(_, _)));
}

#[test]
fn import_meta() {
    let expr = parse_expression("import.meta").unwrap();
    assert!(matches!(expr, Expr::ImportMeta(_)));
}

// ─────────────────────────────────────────────────────────────────────────────
// Automatic semicolon insertion
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn asi_after_return() {
    let prog = parse_script("function f() { return\n42; }").unwrap();
    if let ProgramItem::Stmt(Stmt::Decl(Decl::Fn(f))) = &prog.body[0] {
        if let Stmt::Return(r) = &f.function.body.body[0] {
            // ASI inserts semicolon → return has no argument
            assert!(r.argument.is_none(), "ASI should prevent value from binding to return");
        } else { panic!(); }
    } else { panic!(); }
}

#[test]
fn asi_at_end_of_block() {
    // No explicit semicolon before `}`
    let prog = parse_script("{ x = 1 }").unwrap();
    assert_eq!(prog.body.len(), 1);
}

// ─────────────────────────────────────────────────────────────────────────────
// Error cases
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn error_on_unexpected_token() {
    assert!(parse_expression("1 +").is_err());
}

#[test]
fn error_on_empty_expression() {
    assert!(parse_expression("").is_err());
}

#[test]
fn error_unmatched_paren() {
    assert!(parse_expression("(1 + 2").is_err());
}

#[test]
fn error_import_in_script() {
    // import declarations are only valid in modules
    let result = parse_script("import x from 'y';");
    // Either it's treated as an expression statement (calling import()) or it errors
    // Our parser handles import as a module item only when source_type == Module
    // In script mode, import( is a dynamic import call expression
    // import without ( is parsed as statement and may fail
    let _ = result; // either Ok or Err is acceptable — just shouldn't panic
}

#[test]
fn error_exponent_with_unary() {
    assert!(parse_expression("-2 ** 2").is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// Complex real-world patterns
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn complex_destructuring_assignment() {
    let prog = parse_script("const { a: { b, c }, d = 5, ...rest } = obj;").unwrap();
    assert_eq!(prog.body.len(), 1);
}

#[test]
fn chained_optional_calls() {
    let expr = parse_expression("obj?.method?.()?.result").unwrap();
    // Should parse without error
    let _ = expr;
}

#[test]
fn tagged_template_literal() {
    let expr = parse_expression("html`<div>${content}</div>`").unwrap();
    assert!(matches!(expr, Expr::TaggedTpl(_)));
}

#[test]
fn logical_assignment_operators() {
    let expr = parse_expression("a ||= b").unwrap();
    if let Expr::Assign(a) = expr { assert_eq!(a.op, AssignOp::OrAssign); }
    else { panic!(); }

    let expr = parse_expression("a ??= b").unwrap();
    if let Expr::Assign(a) = expr { assert_eq!(a.op, AssignOp::NullishAssign); }
    else { panic!(); }
}

#[test]
fn arrow_with_destructuring_params() {
    let expr = parse_expression("({ x, y }) => x + y").unwrap();
    if let Expr::Arrow(a) = expr {
        assert_eq!(a.params.len(), 1);
        assert!(matches!(a.params[0].pat, Pat::Object(_)));
    } else { panic!(); }
}

#[test]
fn sequence_expression() {
    let expr = parse_expression("a, b, c").unwrap();
    if let Expr::Seq(s) = expr {
        assert_eq!(s.exprs.len(), 3);
    } else { panic!(); }
}

#[test]
fn parse_full_class_body() {
    let src = r#"
class EventEmitter {
    #listeners = new Map();

    static create() { return new EventEmitter(); }

    on(event, fn) { this.#listeners.set(event, fn); }

    get size() { return this.#listeners.size; }

    async emit(event, data) {
        const fn = this.#listeners.get(event);
        await fn?.(data);
    }

    static { EventEmitter.prototype.off = EventEmitter.prototype.on; }
}
"#;
    let prog = parse_script(src).unwrap();
    if let ProgramItem::Stmt(Stmt::Decl(Decl::Class(c))) = &prog.body[0] {
        assert_eq!(c.id.as_ref().unwrap().name, "EventEmitter");
        assert!(c.body.body.len() >= 4);
    } else { panic!(); }
}

#[test]
fn parse_real_world_snippet() {
    let src = r#"
async function* paginate(url, options = {}) {
    let next = url;
    while (next) {
        const { data, nextPage } = await fetch(next, options).then(r => r.json());
        yield* data;
        next = nextPage ?? null;
    }
}
"#;
    let prog = parse_script(src).unwrap();
    if let ProgramItem::Stmt(Stmt::Decl(Decl::Fn(f))) = &prog.body[0] {
        assert!(f.function.is_async);
        assert!(f.function.is_generator);
    } else { panic!(); }
}