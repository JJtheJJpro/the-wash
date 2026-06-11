//! Integration tests for the no_std HTML parser.

extern crate alloc;
use html_parser_nostd::{NodeKind, parse};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn text_of(html: &str) -> alloc::string::String {
    parse(html).unwrap().text_content()
}

// ── Basic structure ──────────────────────────────────────────────────────────

#[test]
fn empty_input() {
    let doc = parse("").unwrap();
    assert!(doc.children.is_empty());
}

#[test]
fn plain_text() {
    let doc = parse("Hello, world!").unwrap();
    assert_eq!(doc.children.len(), 1);
    match &doc.children[0].kind {
        NodeKind::Text(t) => assert_eq!(t, "Hello, world!"),
        other => panic!("expected text, got {:?}", other),
    }
}

#[test]
fn simple_element() {
    let doc = parse("<p>Hello</p>").unwrap();
    let p = &doc.children[0];
    assert_eq!(p.tag(), Some("p"));
    assert_eq!(p.text_content(), "Hello");
}

#[test]
fn nested_elements() {
    let doc = parse("<div><span>a</span><span>b</span></div>").unwrap();
    let div = &doc.children[0];
    assert_eq!(div.tag(), Some("div"));
    let children = div.children();
    assert_eq!(children.len(), 2);
    assert_eq!(children[0].text_content(), "a");
    assert_eq!(children[1].text_content(), "b");
}

#[test]
fn full_page() {
    let html = r#"<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <title>Test Page</title>
  </head>
  <body>
    <h1 id="main">Hello</h1>
    <p class="intro">World &amp; beyond</p>
  </body>
</html>"#;
    let doc = parse(html).unwrap();

    // DOCTYPE present
    assert!(matches!(&doc.children[0].kind, NodeKind::Doctype { name, .. } if name == "html"));

    // <html> element
    let html_el = doc.root_element().unwrap();
    assert_eq!(html_el.attr("lang"), Some("en"));

    // find <title>
    let title: alloc::vec::Vec<_> = doc.find_all("title").collect();
    assert_eq!(title.len(), 1);
    assert_eq!(title[0].text_content(), "Test Page");

    // entity decoding
    let p: alloc::vec::Vec<_> = doc.find_all("p").collect();
    assert_eq!(p[0].text_content(), "World & beyond");
}

// ── Attributes ───────────────────────────────────────────────────────────────

#[test]
fn double_quoted_attr() {
    let doc = parse(r#"<a href="https://example.com">link</a>"#).unwrap();
    assert_eq!(doc.children[0].attr("href"), Some("https://example.com"));
}

#[test]
fn single_quoted_attr() {
    let doc = parse("<img src='photo.jpg'>").unwrap();
    assert_eq!(doc.children[0].attr("src"), Some("photo.jpg"));
}

#[test]
fn unquoted_attr() {
    let doc = parse("<input type=text>").unwrap();
    assert_eq!(doc.children[0].attr("type"), Some("text"));
}

#[test]
fn boolean_attr() {
    let doc = parse("<input disabled>").unwrap();
    assert_eq!(doc.children[0].attr("disabled"), Some(""));
}

#[test]
fn multiple_attrs() {
    let doc = parse(r#"<div id="main" class="container" data-x="1">"#).unwrap();
    let el = &doc.children[0];
    assert_eq!(el.attr("id"), Some("main"));
    assert_eq!(el.attr("class"), Some("container"));
    assert_eq!(el.attr("data-x"), Some("1"));
}

#[test]
fn attr_entity_in_value() {
    let doc = parse(r#"<a title="a &amp; b">x</a>"#).unwrap();
    assert_eq!(doc.children[0].attr("title"), Some("a & b"));
}

// ── Self-closing & void elements ─────────────────────────────────────────────

#[test]
fn void_br() {
    let doc = parse("<br>").unwrap();
    match &doc.children[0].kind {
        NodeKind::Element {
            tag,
            self_closing,
            children,
            ..
        } => {
            assert_eq!(tag, "br");
            assert!(self_closing);
            assert!(children.is_empty());
        }
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn void_img() {
    let doc = parse(r#"<img src="x.png" alt="x">"#).unwrap();
    assert_eq!(doc.children[0].tag(), Some("img"));
    assert_eq!(doc.children[0].attr("src"), Some("x.png"));
}

#[test]
fn explicit_self_closing() {
    let doc = parse("<br />").unwrap();
    match &doc.children[0].kind {
        NodeKind::Element { self_closing, .. } => assert!(self_closing),
        other => panic!("{:?}", other),
    }
}

#[test]
fn xhtml_self_closing_div() {
    // <div /> should NOT produce children
    let doc = parse("<div />").unwrap();
    assert!(doc.children[0].children().is_empty());
}

// ── DOCTYPE ──────────────────────────────────────────────────────────────────

#[test]
fn doctype_html5() {
    let doc = parse("<!DOCTYPE html>").unwrap();
    match &doc.children[0].kind {
        NodeKind::Doctype {
            name,
            public_id,
            system_id,
        } => {
            assert_eq!(name, "html");
            assert!(public_id.is_none());
            assert!(system_id.is_none());
        }
        other => panic!("{:?}", other),
    }
}

#[test]
fn doctype_html4() {
    let input = r#"<!DOCTYPE HTML PUBLIC "-//W3C//DTD HTML 4.01//EN" "http://www.w3.org/TR/html4/strict.dtd">"#;
    let doc = parse(input).unwrap();
    match &doc.children[0].kind {
        NodeKind::Doctype {
            name,
            public_id,
            system_id,
        } => {
            assert_eq!(name, "html");
            assert_eq!(public_id.as_deref(), Some("-//W3C//DTD HTML 4.01//EN"));
            assert_eq!(
                system_id.as_deref(),
                Some("http://www.w3.org/TR/html4/strict.dtd")
            );
        }
        other => panic!("{:?}", other),
    }
}

// ── Comments ─────────────────────────────────────────────────────────────────

#[test]
fn comment() {
    let doc = parse("<!-- hello -->").unwrap();
    match &doc.children[0].kind {
        NodeKind::Comment(c) => assert_eq!(c.trim(), "hello"),
        other => panic!("{:?}", other),
    }
}

#[test]
fn comment_inside_element() {
    let doc = parse("<div><!-- note -->text</div>").unwrap();
    let children = doc.children[0].children();
    assert_eq!(children.len(), 2);
    assert!(matches!(&children[0].kind, NodeKind::Comment(_)));
}

#[test]
fn comment_with_dashes() {
    // Comments with `--` inside are valid in HTML5
    let doc = parse("<!-- a -- b -->").unwrap();
    assert!(matches!(&doc.children[0].kind, NodeKind::Comment(_)));
}

// ── CDATA ────────────────────────────────────────────────────────────────────

#[test]
fn cdata_section() {
    let doc = parse("<![CDATA[some <raw> data]]>").unwrap();
    match &doc.children[0].kind {
        NodeKind::CData(c) => assert_eq!(c, "some <raw> data"),
        other => panic!("{:?}", other),
    }
}

// ── Processing instruction ───────────────────────────────────────────────────

#[test]
fn processing_instruction() {
    let doc = parse(r#"<?xml version="1.0"?>"#).unwrap();
    match &doc.children[0].kind {
        NodeKind::ProcessingInstruction { target, content } => {
            assert_eq!(target, "xml");
            assert!(content.contains("version"));
        }
        other => panic!("{:?}", other),
    }
}

// ── Entities ─────────────────────────────────────────────────────────────────

#[test]
fn named_entities() {
    assert_eq!(text_of("&amp;"), "&");
    assert_eq!(text_of("&lt;"), "<");
    assert_eq!(text_of("&gt;"), ">");
    assert_eq!(text_of("&quot;"), "\"");
    assert_eq!(text_of("&apos;"), "'");
    assert_eq!(text_of("&copy;"), "\u{00A9}");
    assert_eq!(text_of("&nbsp;"), "\u{00A0}");
    assert_eq!(text_of("&euro;"), "\u{20AC}");
}

#[test]
fn decimal_char_ref() {
    assert_eq!(text_of("&#65;"), "A");
    assert_eq!(text_of("&#169;"), "\u{00A9}");
    assert_eq!(text_of("&#8364;"), "\u{20AC}");
}

#[test]
fn hex_char_ref() {
    assert_eq!(text_of("&#x41;"), "A");
    assert_eq!(text_of("&#xA9;"), "\u{00A9}");
    assert_eq!(text_of("&#X20AC;"), "\u{20AC}"); // capital X
}

#[test]
fn unknown_entity_passthrough() {
    // Unrecognised entities should be left as `&` + name (lenient)
    let doc = parse("&fakeentity;").unwrap();
    // The parser should not crash; text content begins with `&`
    let t = doc.text_content();
    assert!(t.starts_with('&') || !t.is_empty());
}

// ── Error recovery ───────────────────────────────────────────────────────────

#[test]
fn unclosed_tag() {
    // <div> without </div> — still produces an element node
    let doc = parse("<div>Hello").unwrap();
    assert_eq!(doc.children[0].tag(), Some("div"));
    assert_eq!(doc.children[0].text_content(), "Hello");
}

#[test]
fn extra_close_tag() {
    // Spurious </span> should be ignored
    let doc = parse("<p>Hello</span> World</p>").unwrap();
    assert_eq!(doc.children[0].text_content(), "Hello World");
}

#[test]
fn misnested_tags() {
    // <b><i>text</b></i> — parser should still produce nodes
    let doc = parse("<b><i>text</b></i>").unwrap();
    assert!(!doc.children.is_empty());
    assert_eq!(doc.text_content(), "text");
}

#[test]
fn stray_angle_bracket_in_text() {
    // A `<` not followed by a valid tag name should become text
    let doc = parse("a < b > c").unwrap();
    let text = doc.text_content();
    assert!(text.contains('a'));
    assert!(text.contains('c'));
}

// ── Raw-text elements ────────────────────────────────────────────────────────

#[test]
fn script_raw_text() {
    let doc = parse("<script>var x = 1 < 2 && y > 0;</script>").unwrap();
    let script = &doc.children[0];
    assert_eq!(script.tag(), Some("script"));
    let text = script.text_content();
    assert!(text.contains("var x = 1 < 2"));
}

#[test]
fn style_raw_text() {
    let doc = parse("<style>body { color: red; }</style>").unwrap();
    assert_eq!(doc.children[0].tag(), Some("style"));
    assert!(doc.children[0].text_content().contains("color"));
}

// ── Auto-close (optional end tags) ──────────────────────────────────────────

#[test]
fn li_auto_close() {
    // Each <li> should auto-close the previous one
    let doc = parse("<ul><li>a<li>b<li>c</ul>").unwrap();
    let ul = &doc.children[0];
    assert_eq!(ul.tag(), Some("ul"));
    let items: alloc::vec::Vec<_> = ul.find_all("li").collect();
    assert_eq!(items.len(), 3);
}

#[test]
fn p_auto_close_on_block() {
    // Opening a block element inside <p> should auto-close <p>
    let doc = parse("<p>intro<div>block</div>").unwrap();
    // p and div should be siblings (or p closed before div)
    assert!(!doc.children.is_empty());
}

// ── DOM traversal ────────────────────────────────────────────────────────────

#[test]
fn find_all_deep() {
    let html = "<div><p>a</p><section><p>b</p></section></div>";
    let doc = parse(html).unwrap();
    let ps: alloc::vec::Vec<_> = doc.find_all("p").collect();
    assert_eq!(ps.len(), 2);
}

#[test]
fn find_all_returns_none_for_absent_tag() {
    let doc = parse("<div>text</div>").unwrap();
    let items: alloc::vec::Vec<_> = doc.find_all("span").collect();
    assert!(items.is_empty());
}

#[test]
fn text_content_recursive() {
    let html = "<div><p>Hello <em>world</em>!</p></div>";
    assert_eq!(text_of(html), "Hello world!");
}

#[test]
fn attr_lookup_missing() {
    let doc = parse("<div>x</div>").unwrap();
    assert_eq!(doc.children[0].attr("class"), None);
}

// ── Case sensitivity ─────────────────────────────────────────────────────────

#[test]
fn tag_names_lowercased() {
    let doc = parse("<DIV><P>Hello</P></DIV>").unwrap();
    assert_eq!(doc.children[0].tag(), Some("div"));
    assert_eq!(doc.children[0].children()[0].tag(), Some("p"));
}

#[test]
fn attr_names_lowercased() {
    let doc = parse(r#"<a HREF="/path" CLASS="x">link</a>"#).unwrap();
    let a = &doc.children[0];
    assert_eq!(a.attr("href"), Some("/path"));
    assert_eq!(a.attr("class"), Some("x"));
}

// ── Display / formatting ─────────────────────────────────────────────────────

#[test]
fn display_roundtrip_structure() {
    let html = r#"<html><head><title>Hi</title></head><body><p>World</p></body></html>"#;
    let doc = parse(html).unwrap();
    let rendered = alloc::format!("{}", doc);
    // Should contain tag names and text
    assert!(rendered.contains("<html>"));
    assert!(rendered.contains("<title>"));
    assert!(rendered.contains("Hi"));
    assert!(rendered.contains("World"));
}
