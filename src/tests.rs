#[test]
fn test0() {
    assert!(the_wash::html::parse_html("< os></os>").roots.is_empty());
}


#[test]
fn test1() {
    use the_wash::html::HtmlTree;

    let h = the_wash::html::parse_html("<os></os>");
    assert!(h.roots.len() == 1);
    match &h.roots[0] {
        HtmlTree::Tag {
            name,
            attrs,
            children,
        } => {
            assert!(*name == "os");
            assert!(attrs.is_empty());
            assert!(children.is_empty());
        }
        _ => panic!()
    }
}

#[test]
fn test2() {
    use the_wash::html::HtmlTree;

    let h = the_wash::html::parse_html("<os ></os>");
    assert!(h.roots.len() == 1);
    match &h.roots[0] {
        HtmlTree::Tag {
            name,
            attrs,
            children,
        } => {
            assert!(*name == "os");
            assert!(attrs.is_empty());
            assert!(children.is_empty());
        }
        _ => panic!()
    }
}