//! HTML Parser using hand-written byte-by-byte tokenizing and parsing.

extern crate alloc;

use alloc::{string::String, vec::Vec};
use core::{fmt::Debug, str};

#[derive(Clone, Debug)]
pub struct HtmlDoc<'a> {
    pub doc_type: Option<&'a str>,
    pub root: HtmlTree<'a>,
}
impl HtmlDoc<'_> {
    pub fn fixed_format(&self) -> String {
        fn recurse(tree: &HtmlTree<'_>, s: &mut String) {
            match tree {
                HtmlTree::Tag {
                    name,
                    attrs,
                    children,
                } => {
                    *s += &alloc::format!("<{}", name);
                    for attr in attrs {
                        *s += &alloc::format!(" {}", attr.name);
                        if let Some(v) = attr.value {
                            *s += &alloc::format!("={}", v);
                        }
                    }
                    *s += ">";

                    for child in children {
                        recurse(child, s);
                    }

                    *s += &alloc::format!("</{}>", name);
                }
                HtmlTree::Text(t) => {
                    *s += t;
                }
                _ => {}
            }
        }
        let mut sb = String::new();
        if let Some(v) = self.doc_type {
            sb += &alloc::format!("<!DOCTYPE {v}>");
        }

        recurse(&self.root, &mut sb);

        sb
    }
}
//impl Debug for HtmlDoc<'_> {
//    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
//        fn recurse(tree: &HtmlTree<'_>, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
//            match tree {
//                HtmlTree::Tag {
//                    name,
//                    attrs,
//                    children,
//                } => {
//                    write!(f, "<{name}")?;
//                    for attr in attrs {
//                        write!(f, " {}", attr.name)?;
//                        if let Some(v) = attr.value {
//                            write!(f, "={v}")?;
//                        }
//                    }
//                    write!(f, ">")?;
//
//                    for child in children {
//                        recurse(child, f)?;
//                    }
//
//                    write!(f, "</{name}>")
//                }
//                HtmlTree::Text(t) => {
//                    write!(f, "{t}")
//                }
//                _ => Ok(()),
//            }
//        }
//
//        if let Some(v) = self.doc_type {
//            write!(f, "<!DOCTYPE {v}>")?;
//        }
//
//        recurse(&self.root, f)?;
//
//        Ok(())
//    }
//}

#[derive(Clone, Debug)]
pub struct Attribute<'a> {
    pub name: &'a str,
    pub value: Option<&'a str>,
}

#[derive(Clone, Debug)]
pub enum HtmlTree<'a> {
    Tag {
        name: &'a str,
        attrs: Vec<Attribute<'a>>,
        children: Vec<HtmlTree<'a>>,
    },
    Script(&'a str),
    Style(&'a str),
    Text(&'a str),
}

struct HtmlFlags(u8);
impl HtmlFlags {
    const fn get_bit(&self, bit: u8) -> bool {
        (self.0 & (1 << bit)) > 0
    }
    const fn set_bit(&mut self, bit: u8, v: bool) {
        if v {
            self.0 |= 1 << bit
        } else {
            self.0 &= !(1 << bit)
        }
    }
    const fn reset(&mut self) {
        self.0 = 0
    }

    const fn get_open_tag(&self) -> bool {
        self.get_bit(0)
    }
    const fn get_name_issued(&self) -> bool {
        self.get_bit(1)
    }
    const fn get_end_tag(&self) -> bool {
        self.get_bit(2)
    }

    const fn get_inside_quote(&self) -> bool {
        self.get_bit(7)
    }

    const fn set_open_tag(&mut self, v: bool) {
        self.set_bit(0, v)
    }
    const fn set_name_issued(&mut self, v: bool) {
        self.set_bit(1, v)
    }
    const fn set_end_tag(&mut self, v: bool) {
        self.set_bit(2, v)
    }

    const fn set_inside_quote(&mut self, v: bool) {
        self.set_bit(7, v)
    }
}

pub fn parse_html<'a>(input: &'a str) -> HtmlDoc<'a> {
    let b = input.as_bytes();
    let sz = b.len();

    let mut from = 0;
    let mut to = 0;

    let mut from_n = 0;
    let mut to_n = 0;

    let mut from_q = 0;
    let mut to_q = 0;

    // Only relative from the body tag.
    //let mut tree_pos = (0, 0);

    let mut tag_name = "";

    //let mut temp_attrs = alloc::vec![];

    let mut flags = HtmlFlags(0);

    let mut text_builder = String::new();

    let mut doc = HtmlDoc {
        doc_type: None,
        root: HtmlTree::Tag {
            name: "html",
            attrs: alloc::vec![],
            children: alloc::vec![
                HtmlTree::Tag {
                    name: "head",
                    attrs: alloc::vec![],
                    children: alloc::vec![]
                },
                HtmlTree::Tag {
                    name: "body",
                    attrs: alloc::vec![],
                    children: alloc::vec![]
                }
            ],
        },
    };

    for p in 0..sz {
        debug_assert!(from_n <= to_n, "from_n <= to_n  ->  {from_n} <= {to_n}");
        debug_assert!(from <= to, "from <= to  ->  {from} <= {to}");
        let byte = b[p];
        if !flags.get_inside_quote() {
            match byte {
                b'\n' | b'\t' | b' ' => {
                    if flags.get_open_tag() {
                        if flags.get_name_issued() {
                            to_n = p;
                            tag_name = str::from_utf8(&b[from_n..to_n]).unwrap();
                        } else {
                            flags.reset();
                        }
                    }
                }
                b'<' => {
                    flags.set_open_tag(true);
                    to = p;
                }
                b'>' => {
                    if flags.get_open_tag() {
                        if tag_name.is_empty() {
                            to_n = p;
                            tag_name = str::from_utf8(&b[from_n..to_n]).unwrap();
                        }

                        if from != to {
                            if let HtmlTree::Tag {
                                ref mut children, ..
                            } = doc.root
                            {
                                if let HtmlTree::Tag {
                                    ref mut children, ..
                                } = children[1]
                                {
                                    children.push(HtmlTree::Text(
                                        str::from_utf8(&b[from..to]).unwrap(),
                                    ));
                                    children.push(HtmlTree::Tag {
                                        name: tag_name,
                                        attrs: alloc::vec![],
                                        children: alloc::vec![],
                                    })
                                } else {
                                    unreachable!();
                                }
                            } else {
                                unreachable!();
                            }
                        }
                        from = p;
                        to = p;

                        flags.reset();
                    }
                }
                b'/' => {}

                _ => {
                    if flags.get_open_tag() && !flags.get_name_issued() {
                        flags.set_name_issued(true);
                        from_n = p;
                        to_n = p;
                    }
                }
            }
        }
    }

    if from != to {
        if let HtmlTree::Tag {
            ref mut children, ..
        } = doc.root
        {
            if let HtmlTree::Tag {
                ref mut children, ..
            } = children[1]
            {
                children.push(HtmlTree::Text(str::from_utf8(&b[from..to]).unwrap()));
            } else {
                unreachable!();
            }
        } else {
            unreachable!();
        }
    }

    doc
}
