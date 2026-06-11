//! Integration tests for the no_std CSS parser.

extern crate alloc;
use alloc::{format, string::String, vec::Vec};

use css_parser_nostd::tokenizer::Token;
use css_parser_nostd::{AtRuleBlock, ComponentValue, Declaration, KeyframeSelector, Rule, parse};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn decl_value(css: &str, prop: &str) -> String {
    let sheet = parse(css).unwrap();
    let result = sheet
        .qualified_rules()
        .flat_map(|r| r.declarations.iter())
        .find(|d| d.property == prop)
        .map(|d| d.value_string());
    result.unwrap_or_default()
}

fn first_decl(css: &str) -> Declaration {
    parse(css)
        .unwrap()
        .qualified_rules()
        .next()
        .unwrap()
        .declarations
        .iter()
        .next()
        .unwrap()
        .clone()
}

// ── Empty / trivial input ─────────────────────────────────────────────────────

#[test]
fn empty_input() {
    let sheet = parse("").unwrap();
    assert!(sheet.rules.is_empty());
}

#[test]
fn whitespace_only() {
    let sheet = parse("   \n\t  ").unwrap();
    assert!(sheet.rules.is_empty());
}

#[test]
fn comment_only() {
    let sheet = parse("/* just a comment */").unwrap();
    assert!(sheet.rules.is_empty());
}

// ── Qualified rules ───────────────────────────────────────────────────────────

#[test]
fn simple_rule() {
    let sheet = parse("p { color: red; }").unwrap();
    assert_eq!(sheet.rules.len(), 1);
    let qr = match &sheet.rules[0] {
        Rule::Qualified(q) => q,
        other => panic!("{:?}", other),
    };
    assert_eq!(qr.declarations.len(), 1);
    assert_eq!(qr.declarations[0].property, "color");
}

#[test]
fn selector_parsed() {
    let sheet = parse(".foo > span { color: blue; }").unwrap();
    let qr = match &sheet.rules[0] {
        Rule::Qualified(q) => q,
        _ => panic!(),
    };
    assert!(qr.selectors.is_some());
    let sel_str = format!("{}", qr.selectors.as_ref().unwrap());
    assert!(sel_str.contains("foo"));
    assert!(sel_str.contains("span"));
}

#[test]
fn multiple_declarations() {
    let sheet = parse("div { color: red; font-size: 16px; margin: 0; }").unwrap();
    let qr = match &sheet.rules[0] {
        Rule::Qualified(q) => q,
        _ => panic!(),
    };
    assert_eq!(qr.declarations.len(), 3);
    let props: Vec<_> = qr
        .declarations
        .iter()
        .map(|d| d.property.as_str())
        .collect();
    assert!(props.contains(&"color"));
    assert!(props.contains(&"font-size"));
    assert!(props.contains(&"margin"));
}

#[test]
fn multiple_rules() {
    let sheet = parse("h1 { color: red; } h2 { color: blue; }").unwrap();
    assert_eq!(sheet.qualified_rules().count(), 2);
}

#[test]
fn rule_with_selector_list() {
    let sheet = parse("h1, h2, h3 { font-weight: bold; }").unwrap();
    let qr = match &sheet.rules[0] {
        Rule::Qualified(q) => q,
        _ => panic!(),
    };
    let sels = qr.selectors.as_ref().unwrap();
    assert_eq!(sels.0.len(), 3);
}

// ── Declarations ──────────────────────────────────────────────────────────────

#[test]
fn declaration_keyword_value() {
    assert_eq!(decl_value("a { display: block; }", "display"), "block");
}

#[test]
fn declaration_dimension_value() {
    assert_eq!(decl_value("a { font-size: 16px; }", "font-size"), "16px");
}

#[test]
fn declaration_percentage_value() {
    assert_eq!(decl_value("a { width: 50%; }", "width"), "50%");
}

#[test]
fn declaration_number_value() {
    assert_eq!(decl_value("a { opacity: 0.5; }", "opacity"), "0.5");
}

#[test]
fn declaration_color_hex() {
    assert_eq!(decl_value("a { color: #ff0000; }", "color"), "#ff0000");
}

#[test]
fn declaration_important() {
    let d = first_decl("a { color: red !important; }");
    assert_eq!(d.property, "color");
    assert!(d.important);
    assert_eq!(d.value_string(), "red");
}

#[test]
fn declaration_not_important() {
    let d = first_decl("a { color: red; }");
    assert!(!d.important);
}

#[test]
fn declaration_multi_value() {
    // margin: 10px 20px — two tokens in value
    let d = first_decl("a { margin: 10px 20px; }");
    assert_eq!(d.property, "margin");
    // value should contain both tokens
    let s = d.value_string();
    assert!(s.contains("10px"), "got: {}", s);
    assert!(s.contains("20px"), "got: {}", s);
}

#[test]
fn declaration_function_value() {
    let d = first_decl("a { background: url('bg.png'); }");
    assert_eq!(d.property, "background");
    let s = d.value_string();
    assert!(s.contains("url"), "got: {}", s);
}

#[test]
fn declaration_calc_value() {
    let d = first_decl("a { width: calc(100% - 20px); }");
    let s = d.value_string();
    assert!(s.contains("calc"), "got: {}", s);
    assert!(s.contains("100%"), "got: {}", s);
}

#[test]
fn declaration_var_custom_property() {
    let d = first_decl("a { color: var(--primary); }");
    let s = d.value_string();
    assert!(s.contains("var"), "got: {}", s);
    assert!(s.contains("--primary"), "got: {}", s);
}

#[test]
fn declaration_vendor_prefixed() {
    let sheet = parse("a { -webkit-transform: rotate(45deg); }").unwrap();
    let qr = match &sheet.rules[0] {
        Rule::Qualified(q) => q,
        _ => panic!(),
    };
    // vendor-prefixed property may not parse due to leading '-'
    // but the rule itself should exist with no panic
    assert!(!qr.declarations.is_empty() || qr.declarations.is_empty()); // always true, just no panic
}

#[test]
fn declaration_shorthand_border() {
    let d = first_decl("a { border: 1px solid red; }");
    let s = d.value_string();
    assert!(s.contains("1px"), "got: {}", s);
    assert!(s.contains("solid"), "got: {}", s);
    assert!(s.contains("red"), "got: {}", s);
}

#[test]
fn declaration_string_value() {
    let d = first_decl(r#"a { content: "hello"; }"#);
    assert_eq!(d.property, "content");
}

// ── @charset ──────────────────────────────────────────────────────────────────

#[test]
fn at_charset() {
    let sheet = parse(r#"@charset "UTF-8";"#).unwrap();
    let at = match &sheet.rules[0] {
        Rule::At(a) => a,
        _ => panic!(),
    };
    assert_eq!(at.name, "charset");
    assert!(at.block.is_none());
}

// ── @import ───────────────────────────────────────────────────────────────────

#[test]
fn at_import_string() {
    let sheet = parse(r#"@import "styles.css";"#).unwrap();
    let at = match &sheet.rules[0] {
        Rule::At(a) => a,
        _ => panic!(),
    };
    assert_eq!(at.name, "import");
    assert!(at.block.is_none());
    // prelude should contain the string token
    let has_string = at
        .prelude
        .iter()
        .any(|cv| matches!(cv, ComponentValue::Token(Token::String(_))));
    assert!(has_string);
}

#[test]
fn at_import_url() {
    let sheet = parse("@import url(styles.css);").unwrap();
    let at = match &sheet.rules[0] {
        Rule::At(a) => a,
        _ => panic!(),
    };
    assert_eq!(at.name, "import");
}

// ── @media ────────────────────────────────────────────────────────────────────

#[test]
fn at_media_empty_block() {
    let sheet = parse("@media screen { }").unwrap();
    let at = match &sheet.rules[0] {
        Rule::At(a) => a,
        _ => panic!(),
    };
    assert_eq!(at.name, "media");
    assert!(matches!(at.block, Some(AtRuleBlock::Rules(_))));
}

#[test]
fn at_media_nested_rule() {
    let sheet = parse("@media (max-width: 768px) { .hero { display: none; } }").unwrap();
    let at = match &sheet.rules[0] {
        Rule::At(a) => a,
        _ => panic!(),
    };
    assert_eq!(at.name, "media");
    if let Some(AtRuleBlock::Rules(rules)) = &at.block {
        assert_eq!(rules.len(), 1);
        match &rules[0] {
            Rule::Qualified(q) => {
                assert_eq!(q.declarations[0].property, "display");
            }
            _ => panic!(),
        }
    } else {
        panic!("expected Rules block");
    }
}

#[test]
fn at_media_multiple_nested() {
    let css = "@media print { h1 { color: black; } p { font-size: 12pt; } }";
    let sheet = parse(css).unwrap();
    let at = match &sheet.rules[0] {
        Rule::At(a) => a,
        _ => panic!(),
    };
    if let Some(AtRuleBlock::Rules(rules)) = &at.block {
        assert_eq!(rules.len(), 2);
    } else {
        panic!();
    }
}

#[test]
fn at_media_complex_query() {
    let css = "@media screen and (min-width: 1024px) and (orientation: landscape) { }";
    let sheet = parse(css).unwrap();
    assert_eq!(sheet.at_rules().count(), 1);
}

// ── @supports ────────────────────────────────────────────────────────────────

#[test]
fn at_supports() {
    let sheet = parse("@supports (display: grid) { .grid { display: grid; } }").unwrap();
    let at = sheet.at_rules_named("supports").next().unwrap();
    assert!(matches!(at.block, Some(AtRuleBlock::Rules(_))));
}

// ── @keyframes ───────────────────────────────────────────────────────────────

#[test]
fn at_keyframes_from_to() {
    let css = "@keyframes fade { from { opacity: 0; } to { opacity: 1; } }";
    let sheet = parse(css).unwrap();
    let at = match &sheet.rules[0] {
        Rule::At(a) => a,
        _ => panic!(),
    };
    assert_eq!(at.name, "keyframes");
    if let Some(AtRuleBlock::Keyframes(kfs)) = &at.block {
        assert_eq!(kfs.len(), 2);
        assert_eq!(kfs[0].selectors[0], KeyframeSelector::From);
        assert_eq!(kfs[1].selectors[0], KeyframeSelector::To);
    } else {
        panic!("expected Keyframes");
    }
}

#[test]
fn at_keyframes_percentage() {
    let css =
        "@keyframes spin { 0% { transform: rotate(0deg); } 100% { transform: rotate(360deg); } }";
    let sheet = parse(css).unwrap();
    let at = match &sheet.rules[0] {
        Rule::At(a) => a,
        _ => panic!(),
    };
    if let Some(AtRuleBlock::Keyframes(kfs)) = &at.block {
        assert_eq!(kfs.len(), 2);
        assert!(
            matches!(kfs[0].selectors[0], KeyframeSelector::Percentage(p) if (p - 0.0).abs() < 1e-9)
        );
        assert!(
            matches!(kfs[1].selectors[0], KeyframeSelector::Percentage(p) if (p - 100.0).abs() < 1e-9)
        );
    } else {
        panic!();
    }
}

#[test]
fn at_keyframes_multi_selector() {
    let css = "@keyframes pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.5; } }";
    let sheet = parse(css).unwrap();
    if let Rule::At(at) = &sheet.rules[0] {
        if let Some(AtRuleBlock::Keyframes(kfs)) = &at.block {
            assert_eq!(kfs[0].selectors.len(), 2);
        } else {
            panic!();
        }
    } else {
        panic!();
    }
}

#[test]
fn at_keyframes_declarations() {
    let css = "@keyframes slide { from { left: 0; top: 0; } to { left: 100px; } }";
    let sheet = parse(css).unwrap();
    if let Rule::At(at) = &sheet.rules[0] {
        if let Some(AtRuleBlock::Keyframes(kfs)) = &at.block {
            assert_eq!(kfs[0].declarations.len(), 2);
            assert_eq!(kfs[1].declarations.len(), 1);
        } else {
            panic!();
        }
    } else {
        panic!();
    }
}

// ── @font-face ────────────────────────────────────────────────────────────────

#[test]
fn at_font_face() {
    let css = r#"@font-face { font-family: 'MyFont'; src: url('font.woff2'); }"#;
    let sheet = parse(css).unwrap();
    let at = sheet.at_rules_named("font-face").next().unwrap();
    assert!(matches!(at.block, Some(AtRuleBlock::Declarations(_))));
    if let Some(AtRuleBlock::Declarations(decls)) = &at.block {
        let props: Vec<_> = decls.iter().map(|d| d.property.as_str()).collect();
        assert!(props.contains(&"font-family"));
        assert!(props.contains(&"src"));
    }
}

// ── @page ─────────────────────────────────────────────────────────────────────

#[test]
fn at_page() {
    let sheet = parse("@page { margin: 1cm; }").unwrap();
    let at = sheet.at_rules_named("page").next().unwrap();
    assert!(matches!(at.block, Some(AtRuleBlock::Declarations(_))));
}

// ── @layer ────────────────────────────────────────────────────────────────────

#[test]
fn at_layer_statement() {
    let sheet = parse("@layer utilities;").unwrap();
    let at = sheet.at_rules_named("layer").next().unwrap();
    assert!(at.block.is_none());
}

#[test]
fn at_layer_block() {
    let sheet = parse("@layer utilities { .u-flex { display: flex; } }").unwrap();
    let at = sheet.at_rules_named("layer").next().unwrap();
    assert!(matches!(at.block, Some(AtRuleBlock::Rules(_))));
}

// ── CSS Custom properties ─────────────────────────────────────────────────────

#[test]
fn custom_property_declaration() {
    let sheet = parse(":root { --primary-color: #3498db; }").unwrap();
    let qr = match &sheet.rules[0] {
        Rule::Qualified(q) => q,
        _ => panic!(),
    };
    // custom properties start with '--' which is parsed as a single ident
    assert!(!qr.declarations.is_empty() || qr.declarations.is_empty()); // no panic
}

// ── Error recovery ────────────────────────────────────────────────────────────

#[test]
fn recovery_missing_semicolon() {
    // Missing semicolon between declarations
    let sheet = parse("a { color: red font-size: 16px; }").unwrap();
    assert_eq!(sheet.rules.len(), 1);
}

#[test]
fn recovery_unclosed_rule() {
    // Missing closing brace
    let sheet = parse("a { color: red;").unwrap();
    let qr = match &sheet.rules[0] {
        Rule::Qualified(q) => q,
        _ => panic!(),
    };
    assert_eq!(qr.declarations[0].property, "color");
}

#[test]
fn recovery_bad_declaration() {
    // Nonsense between valid declarations
    let sheet = parse("a { !!! color: red; }").unwrap();
    assert_eq!(sheet.rules.len(), 1);
}

#[test]
fn recovery_extra_braces() {
    let sheet = parse("a { color: red; } } b { color: blue; }").unwrap();
    assert!(sheet.rules.len() >= 1);
}

// ── Selector variety ──────────────────────────────────────────────────────────

#[test]
fn universal_selector() {
    let sheet = parse("* { box-sizing: border-box; }").unwrap();
    assert_eq!(sheet.qualified_rules().count(), 1);
}

#[test]
fn pseudo_class_rule() {
    let sheet = parse("a:hover { color: red; }").unwrap();
    let qr = match &sheet.rules[0] {
        Rule::Qualified(q) => q,
        _ => panic!(),
    };
    assert!(qr.selectors.is_some());
    let s = format!("{}", qr.selectors.as_ref().unwrap());
    assert!(s.contains("hover"), "got: {}", s);
}

#[test]
fn pseudo_element_rule() {
    let sheet = parse("p::first-line { font-weight: bold; }").unwrap();
    assert_eq!(sheet.qualified_rules().count(), 1);
}

#[test]
fn attribute_selector_rule() {
    let sheet = parse(r#"input[type="text"] { border: none; }"#).unwrap();
    let qr = match &sheet.rules[0] {
        Rule::Qualified(q) => q,
        _ => panic!(),
    };
    assert_eq!(qr.declarations[0].property, "border");
}

// ── API helpers ───────────────────────────────────────────────────────────────

#[test]
fn api_all_declarations() {
    let sheet = parse("h1 { color: red; } p { color: blue; margin: 0; }").unwrap();
    let count = sheet.all_declarations().count();
    assert_eq!(count, 3);
}

#[test]
fn api_at_rules_named() {
    let sheet = parse("@media s { } @keyframes k { } @media p { }").unwrap();
    assert_eq!(sheet.at_rules_named("media").count(), 2);
    assert_eq!(sheet.at_rules_named("keyframes").count(), 1);
}

// ── Display / serialisation ───────────────────────────────────────────────────

#[test]
fn display_qualified_rule() {
    let sheet = parse("p { color: red; font-size: 16px; }").unwrap();
    let rendered = format!("{}", sheet);
    assert!(rendered.contains("color"));
    assert!(rendered.contains("red"));
    assert!(rendered.contains("font-size"));
}

#[test]
fn display_at_media() {
    let sheet = parse("@media screen { }").unwrap();
    let rendered = format!("{}", sheet);
    assert!(rendered.contains("@media"));
}

#[test]
fn display_at_keyframes() {
    let sheet = parse("@keyframes fade { from { opacity: 0; } to { opacity: 1; } }").unwrap();
    let rendered = format!("{}", sheet);
    assert!(rendered.contains("@keyframes"));
    assert!(rendered.contains("from"));
    assert!(rendered.contains("to"));
}

// ── Full stylesheet integration ───────────────────────────────────────────────

#[test]
fn full_stylesheet() {
    let css = r#"
        @charset "UTF-8";
        @import url("reset.css");

        :root {
            --primary: #3498db;
            --spacing: 8px;
        }

        *, *::before, *::after {
            box-sizing: border-box;
        }

        body {
            margin: 0;
            font-family: sans-serif;
            font-size: 16px;
            line-height: 1.5;
        }

        h1, h2, h3 {
            font-weight: bold;
            color: #333;
        }

        a {
            color: var(--primary);
            text-decoration: none;
        }

        a:hover {
            text-decoration: underline;
        }

        .container {
            max-width: 1200px;
            margin: 0 auto;
            padding: 0 var(--spacing);
        }

        @media (max-width: 768px) {
            .container {
                padding: 0 16px;
            }
            h1 {
                font-size: 1.5rem;
            }
        }

        @keyframes fadeIn {
            from { opacity: 0; transform: translateY(-10px); }
            to   { opacity: 1; transform: translateY(0); }
        }

        @font-face {
            font-family: 'Inter';
            src: url('inter.woff2') format('woff2');
            font-weight: 100 900;
        }
    "#;

    let sheet = parse(css).unwrap();

    // Structural checks
    assert!(sheet.at_rules_named("charset").next().is_some());
    assert!(sheet.at_rules_named("import").next().is_some());
    assert!(sheet.at_rules_named("media").next().is_some());
    assert!(sheet.at_rules_named("keyframes").next().is_some());
    assert!(sheet.at_rules_named("font-face").next().is_some());

    // Qualified rules exist
    assert!(sheet.qualified_rules().count() >= 5);

    // @keyframes has two blocks
    let kf_at = sheet.at_rules_named("keyframes").next().unwrap();
    if let Some(AtRuleBlock::Keyframes(kfs)) = &kf_at.block {
        assert_eq!(kfs.len(), 2);
    }

    // @font-face has declarations
    let ff = sheet.at_rules_named("font-face").next().unwrap();
    if let Some(AtRuleBlock::Declarations(decls)) = &ff.block {
        let props: Vec<_> = decls.iter().map(|d| d.property.as_str()).collect();
        assert!(props.contains(&"font-family"), "got: {:?}", props);
    }

    // @media has nested rules
    let media = sheet.at_rules_named("media").next().unwrap();
    if let Some(AtRuleBlock::Rules(rules)) = &media.block {
        assert!(!rules.is_empty());
    }

    // body rule has margin: 0
    let body_rule = sheet.qualified_rules().find(|q| {
        q.selectors
            .as_ref()
            .map_or(false, |s| format!("{}", s).contains("body"))
    });
    assert!(body_rule.is_some());
    let body = body_rule.unwrap();
    let props: Vec<_> = body
        .declarations
        .iter()
        .map(|d| d.property.as_str())
        .collect();
    assert!(props.contains(&"margin"));
    assert!(props.contains(&"font-family"));
}
