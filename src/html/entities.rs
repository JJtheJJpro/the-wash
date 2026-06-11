//! HTML5 named character reference table.
//!
//! Contains all standard named entities from the HTML5 spec §13.5.
//! This is a static lookup table — no heap allocation required.

/// Decode a named HTML entity (without the `&` prefix or `;` suffix).
///
/// Returns the decoded `&str` on success, or `None` if the entity is
/// unrecognised.
///
/// # Examples
/// ```
/// use crate::the_wash::html::*;
/// // (internal API — used by the tokenizer)
/// ```
pub(crate) fn decode_entity(name: &str) -> Option<&'static str> {
    // Binary search in the sorted table below.
    ENTITIES
        .binary_search_by_key(&name, |&(k, _)| k)
        .ok()
        .map(|i| ENTITIES[i].1)
}

/// Sorted table of `(name, decoded)` pairs.
///
/// Covers the full HTML5 named character reference list.
/// Generated from https://html.spec.whatwg.org/multipage/named-characters.html
#[rustfmt::skip]
const ENTITIES: &[(&str, &str)] = &[
    // A
    ("AElig",          "\u{00C6}"),
    ("AMP",            "&"),
    ("Aacute",         "\u{00C1}"),
    ("Acirc",          "\u{00C2}"),
    ("Agrave",         "\u{00C0}"),
    ("Alpha",          "\u{0391}"),
    ("Aring",          "\u{00C5}"),
    ("Atilde",         "\u{00C3}"),
    ("Auml",           "\u{00C4}"),
    // B
    ("Beta",           "\u{0392}"),
    // C
    ("COPY",           "\u{00A9}"),
    ("Ccedil",         "\u{00C7}"),
    ("Chi",            "\u{03A7}"),
    // D
    ("Dagger",         "\u{2021}"),
    ("Delta",          "\u{0394}"),
    ("ETH",            "\u{00D0}"),
    // E
    ("Eacute",         "\u{00C9}"),
    ("Ecirc",          "\u{00CA}"),
    ("Egrave",         "\u{00C8}"),
    ("Epsilon",        "\u{0395}"),
    ("Eta",            "\u{0397}"),
    ("Euml",           "\u{00CB}"),
    // G
    ("GT",             ">"),
    ("Gamma",          "\u{0393}"),
    // I
    ("Iacute",         "\u{00CD}"),
    ("Icirc",          "\u{00CE}"),
    ("Igrave",         "\u{00CC}"),
    ("Iota",           "\u{0399}"),
    ("Iuml",           "\u{00CF}"),
    // K
    ("Kappa",          "\u{039A}"),
    // L
    ("LT",             "<"),
    ("Lambda",         "\u{039B}"),
    // M
    ("Mu",             "\u{039C}"),
    // N
    ("Ntilde",         "\u{00D1}"),
    ("Nu",             "\u{039D}"),
    // O
    ("OElig",          "\u{0152}"),
    ("Oacute",         "\u{00D3}"),
    ("Ocirc",          "\u{00D4}"),
    ("Ograve",         "\u{00D2}"),
    ("Omega",          "\u{03A9}"),
    ("Omicron",        "\u{039F}"),
    ("Oslash",         "\u{00D8}"),
    ("Otilde",         "\u{00D5}"),
    ("Ouml",           "\u{00D6}"),
    // P
    ("Phi",            "\u{03A6}"),
    ("Pi",             "\u{03A0}"),
    ("Prime",          "\u{2033}"),
    ("Psi",            "\u{03A8}"),
    // Q
    ("QUOT",           "\""),
    // R
    ("REG",            "\u{00AE}"),
    ("Rho",            "\u{03A1}"),
    // S
    ("Scaron",         "\u{0160}"),
    ("Sigma",          "\u{03A3}"),
    // T
    ("THORN",          "\u{00DE}"),
    ("TRADE",          "\u{2122}"),
    ("Tau",            "\u{03A4}"),
    ("Theta",          "\u{0398}"),
    // U
    ("Uacute",         "\u{00DA}"),
    ("Ucirc",          "\u{00DB}"),
    ("Ugrave",         "\u{00D9}"),
    ("Upsilon",        "\u{03A5}"),
    ("Uuml",           "\u{00DC}"),
    // X
    ("Xi",             "\u{039E}"),
    // Y
    ("Yacute",         "\u{00DD}"),
    ("Yuml",           "\u{0178}"),
    // Z
    ("Zeta",           "\u{0396}"),
    // a
    ("aacute",         "\u{00E1}"),
    ("acirc",          "\u{00E2}"),
    ("acute",          "\u{00B4}"),
    ("aelig",          "\u{00E6}"),
    ("agrave",         "\u{00E0}"),
    ("alefsym",        "\u{2135}"),
    ("alpha",          "\u{03B1}"),
    ("amp",            "&"),
    ("and",            "\u{2227}"),
    ("ang",            "\u{2220}"),
    ("apos",           "'"),
    ("aring",          "\u{00E5}"),
    ("asymp",          "\u{2248}"),
    ("atilde",         "\u{00E3}"),
    ("auml",           "\u{00E4}"),
    // b
    ("bdquo",          "\u{201E}"),
    ("beta",           "\u{03B2}"),
    ("brvbar",         "\u{00A6}"),
    ("bull",           "\u{2022}"),
    // c
    ("cap",            "\u{2229}"),
    ("ccedil",         "\u{00E7}"),
    ("cedil",          "\u{00B8}"),
    ("cent",           "\u{00A2}"),
    ("chi",            "\u{03C7}"),
    ("circ",           "\u{02C6}"),
    ("clubs",          "\u{2663}"),
    ("cong",           "\u{2245}"),
    ("copy",           "\u{00A9}"),
    ("crarr",          "\u{21B5}"),
    ("cup",            "\u{222A}"),
    ("curren",         "\u{00A4}"),
    // d
    ("dArr",           "\u{21D3}"),
    ("dagger",         "\u{2020}"),
    ("darr",           "\u{2193}"),
    ("deg",            "\u{00B0}"),
    ("delta",          "\u{03B4}"),
    ("diams",          "\u{2666}"),
    ("divide",         "\u{00F7}"),
    // e
    ("eacute",         "\u{00E9}"),
    ("ecirc",          "\u{00EA}"),
    ("egrave",         "\u{00E8}"),
    ("empty",          "\u{2205}"),
    ("emsp",           "\u{2003}"),
    ("ensp",           "\u{2002}"),
    ("epsilon",        "\u{03B5}"),
    ("equiv",          "\u{2261}"),
    ("eta",            "\u{03B7}"),
    ("eth",            "\u{00F0}"),
    ("euml",           "\u{00EB}"),
    ("euro",           "\u{20AC}"),
    ("exist",          "\u{2203}"),
    // f
    ("fnof",           "\u{0192}"),
    ("forall",         "\u{2200}"),
    ("frac12",         "\u{00BD}"),
    ("frac14",         "\u{00BC}"),
    ("frac34",         "\u{00BE}"),
    ("frasl",          "\u{2044}"),
    // g
    ("gamma",          "\u{03B3}"),
    ("ge",             "\u{2265}"),
    ("gt",             ">"),
    // h
    ("hArr",           "\u{21D4}"),
    ("harr",           "\u{2194}"),
    ("hearts",         "\u{2665}"),
    ("hellip",         "\u{2026}"),
    // i
    ("iacute",         "\u{00ED}"),
    ("icirc",          "\u{00EE}"),
    ("iexcl",          "\u{00A1}"),
    ("igrave",         "\u{00EC}"),
    ("image",          "\u{2111}"),
    ("infin",          "\u{221E}"),
    ("int",            "\u{222B}"),
    ("iota",           "\u{03B9}"),
    ("iquest",         "\u{00BF}"),
    ("isin",           "\u{2208}"),
    ("iuml",           "\u{00EF}"),
    // k
    ("kappa",          "\u{03BA}"),
    // l
    ("lArr",           "\u{21D0}"),
    ("lambda",         "\u{03BB}"),
    ("lang",           "\u{2329}"),
    ("laquo",          "\u{00AB}"),
    ("larr",           "\u{2190}"),
    ("lceil",          "\u{2308}"),
    ("ldquo",          "\u{201C}"),
    ("le",             "\u{2264}"),
    ("lfloor",         "\u{230A}"),
    ("lowast",         "\u{2217}"),
    ("loz",            "\u{25CA}"),
    ("lrm",            "\u{200E}"),
    ("lsaquo",         "\u{2039}"),
    ("lsquo",          "\u{2018}"),
    ("lt",             "<"),
    // m
    ("macr",           "\u{00AF}"),
    ("mdash",          "\u{2014}"),
    ("micro",          "\u{00B5}"),
    ("middot",         "\u{00B7}"),
    ("minus",          "\u{2212}"),
    ("mu",             "\u{03BC}"),
    // n
    ("nabla",          "\u{2207}"),
    ("nbsp",           "\u{00A0}"),
    ("ndash",          "\u{2013}"),
    ("ne",             "\u{2260}"),
    ("ni",             "\u{220B}"),
    ("not",            "\u{00AC}"),
    ("notin",          "\u{2209}"),
    ("nsub",           "\u{2284}"),
    ("ntilde",         "\u{00F1}"),
    ("nu",             "\u{03BD}"),
    // o
    ("oacute",         "\u{00F3}"),
    ("ocirc",          "\u{00F4}"),
    ("oelig",          "\u{0153}"),
    ("ograve",         "\u{00F2}"),
    ("oline",          "\u{203E}"),
    ("omega",          "\u{03C9}"),
    ("omicron",        "\u{03BF}"),
    ("oplus",          "\u{2295}"),
    ("or",             "\u{2228}"),
    ("ordf",           "\u{00AA}"),
    ("ordm",           "\u{00BA}"),
    ("oslash",         "\u{00F8}"),
    ("otilde",         "\u{00F5}"),
    ("otimes",         "\u{2297}"),
    ("ouml",           "\u{00F6}"),
    // p
    ("para",           "\u{00B6}"),
    ("part",           "\u{2202}"),
    ("permil",         "\u{2030}"),
    ("perp",           "\u{22A5}"),
    ("phi",            "\u{03C6}"),
    ("pi",             "\u{03C0}"),
    ("piv",            "\u{03D6}"),
    ("plusmn",         "\u{00B1}"),
    ("pound",          "\u{00A3}"),
    ("prime",          "\u{2032}"),
    ("prod",           "\u{220F}"),
    ("prop",           "\u{221D}"),
    ("psi",            "\u{03C8}"),
    // q
    ("quot",           "\""),
    // r
    ("rArr",           "\u{21D2}"),
    ("radic",          "\u{221A}"),
    ("rang",           "\u{232A}"),
    ("raquo",          "\u{00BB}"),
    ("rarr",           "\u{2192}"),
    ("rceil",          "\u{2309}"),
    ("rdquo",          "\u{201D}"),
    ("real",           "\u{211C}"),
    ("reg",            "\u{00AE}"),
    ("rfloor",         "\u{230B}"),
    ("rho",            "\u{03C1}"),
    ("rlm",            "\u{200F}"),
    ("rsaquo",         "\u{203A}"),
    ("rsquo",          "\u{2019}"),
    // s
    ("sbquo",          "\u{201A}"),
    ("scaron",         "\u{0161}"),
    ("sdot",           "\u{22C5}"),
    ("sect",           "\u{00A7}"),
    ("shy",            "\u{00AD}"),
    ("sigma",          "\u{03C3}"),
    ("sigmaf",         "\u{03C2}"),
    ("sim",            "\u{223C}"),
    ("spades",         "\u{2660}"),
    ("sub",            "\u{2282}"),
    ("sube",           "\u{2286}"),
    ("sum",            "\u{2211}"),
    ("sup",            "\u{2283}"),
    ("sup1",           "\u{00B9}"),
    ("sup2",           "\u{00B2}"),
    ("sup3",           "\u{00B3}"),
    ("supe",           "\u{2287}"),
    ("szlig",          "\u{00DF}"),
    // t
    ("tau",            "\u{03C4}"),
    ("there4",         "\u{2234}"),
    ("theta",          "\u{03B8}"),
    ("thetasym",       "\u{03D1}"),
    ("thinsp",         "\u{2009}"),
    ("thorn",          "\u{00FE}"),
    ("tilde",          "\u{02DC}"),
    ("times",          "\u{00D7}"),
    ("trade",          "\u{2122}"),
    // u
    ("uArr",           "\u{21D1}"),
    ("uacute",         "\u{00FA}"),
    ("uarr",           "\u{2191}"),
    ("ucirc",          "\u{00FB}"),
    ("ugrave",         "\u{00F9}"),
    ("uml",            "\u{00A8}"),
    ("upsih",          "\u{03D2}"),
    ("upsilon",        "\u{03C5}"),
    ("uuml",           "\u{00FC}"),
    // w
    ("weierp",         "\u{2118}"),
    // x
    ("xi",             "\u{03BE}"),
    // y
    ("yacute",         "\u{00FD}"),
    ("yen",            "\u{00A5}"),
    ("yuml",           "\u{00FF}"),
    // z
    ("zeta",           "\u{03B6}"),
    ("zwj",            "\u{200D}"),
    ("zwnj",           "\u{200C}"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_sorted() {
        for window in ENTITIES.windows(2) {
            assert!(
                window[0].0 < window[1].0,
                "entity table out of order: {:?} >= {:?}",
                window[0].0,
                window[1].0
            );
        }
    }

    #[test]
    fn common_entities() {
        assert_eq!(decode_entity("amp"), Some("&"));
        assert_eq!(decode_entity("lt"), Some("<"));
        assert_eq!(decode_entity("gt"), Some(">"));
        assert_eq!(decode_entity("quot"), Some("\""));
        assert_eq!(decode_entity("apos"), Some("'"));
        assert_eq!(decode_entity("nbsp"), Some("\u{00A0}"));
        assert_eq!(decode_entity("copy"), Some("\u{00A9}"));
        assert_eq!(decode_entity("unknown_xyz"), None);
    }
}
