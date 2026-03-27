/// Check whether a character is an XML NameStartChar per the XML spec:
/// https://www.w3.org/TR/xml/#NT-NameStartChar
fn is_name_start_char(c: char) -> bool {
    matches!(c,
        ':' | 'A'..='Z' | '_' | 'a'..='z'
        | '\u{C0}'..='\u{D6}' | '\u{D8}'..='\u{F6}' | '\u{F8}'..='\u{2FF}'
        | '\u{370}'..='\u{37D}' | '\u{37F}'..='\u{1FFF}'
        | '\u{200C}'..='\u{200D}' | '\u{2070}'..='\u{218F}'
        | '\u{2C00}'..='\u{2FEF}' | '\u{3001}'..='\u{D7FF}'
        | '\u{F900}'..='\u{FDCF}' | '\u{FDF0}'..='\u{FFFD}'
        | '\u{10000}'..='\u{EFFFF}'
    )
}

/// Check whether a character is an XML NameChar per the XML spec:
/// https://www.w3.org/TR/xml/#NT-NameChar
fn is_name_char(c: char) -> bool {
    is_name_start_char(c)
        || matches!(c,
            '-' | '.' | '0'..='9' | '\u{B7}'
            | '\u{0300}'..='\u{036F}' | '\u{203F}'..='\u{2040}'
        )
}

/// Lenient start-character check matching actual browser behavior for DOM APIs.
/// Browsers accept any non-ASCII character (>= U+0080) as a valid name start
/// character, plus ASCII letters, `_`, and `:`. This is broader than the strict
/// XML NameStartChar production, which excludes certain Unicode ranges (e.g.
/// U+037E, U+0300, U+FFFF). Colon is included because local parts of QNames
/// can start with `:` (e.g. `f::oo`); the QName colon splitting is handled
/// separately in validate_and_extract.
fn is_lenient_name_start_char(c: char) -> bool {
    matches!(c, ':' | 'A'..='Z' | '_' | 'a'..='z') || c as u32 >= 0x80
}

/// Lenient name validation matching actual browser behavior for DOM APIs like
/// createElementNS and createDocument. Checks that the first character passes
/// the lenient NameStartChar check (ASCII letter, `_`, or any char >= U+0080)
/// and that the name contains no whitespace or '>' chars. Subsequent characters
/// are otherwise completely unchecked. This contradicts the spec (which says to
/// validate against the full QName production) but it's what every browser does,
/// and what WPT tests demand. C'est la vie.
///
/// NOTE: '>' is rejected in ALL positions (not just first). Browsers treat '>'
/// as invalid anywhere in a DOM name, unlike most other non-NameChar characters
/// which are leniently accepted in non-first positions.
pub fn is_valid_dom_name(name: &str) -> bool {
    name.chars().next().is_some_and(is_lenient_name_start_char)
        && !name.contains(char::is_whitespace)
        && !name.contains('>')
}

/// Validates whether a string is a valid element name per the HTML spec.
/// Rules match the WPT name-validation test regex:
///   /^(?:[A-Za-z][^\0\t\n\f\r\u0020/>]*|[:_\u0080-\u{10FFFF}][A-Za-z0-9-.:_\u0080-\u{10FFFF}]*)$/u
///
/// Two cases:
/// 1. ASCII alpha start -> subsequent chars must not be: \0, \t, \n, \x0C, \r, space, /, >
/// 2. :, _, or >= U+0080 start -> subsequent chars only: A-Za-z0-9, -, ., :, _, >= U+0080
///
/// Everything else (empty, digit start, other control chars) is invalid.
pub fn is_valid_element_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        None => false,
        Some(first) if first.is_ascii_alphabetic() => {
            // ASCII alpha start: reject \0, whitespace subset, /, >
            chars.all(|c| !matches!(c, '\0' | '\t' | '\n' | '\x0C' | '\r' | ' ' | '/' | '>'))
        }
        Some(first) if first == ':' || first == '_' || first as u32 >= 0x80 => {
            // :, _, or non-ASCII start: strict subsequent char set
            chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | ':' | '_') || c as u32 >= 0x80)
        }
        _ => false,
    }
}

/// Validates whether a string is a valid attribute name per the HTML spec.
/// Invalid chars: empty, \0, ASCII whitespace (\t, \n, \x0C, \r, space), /, >, =
pub fn is_valid_attribute_name(name: &str) -> bool {
    !name.is_empty() && !name.contains(['\0', '\t', '\n', '\x0C', '\r', ' ', '/', '>', '='])
}

/// Validates whether a string is a valid doctype name.
/// Invalid chars: empty allowed, \0, ASCII whitespace (\t, \n, \x0C, \r, space), >
pub fn is_valid_doctype_name(name: &str) -> bool {
    !name.contains(['\0', '\t', '\n', '\x0C', '\r', ' ', '>'])
}

/// Validates whether a string is a valid XML Name per the XML spec.
/// Used by createProcessingInstruction and other DOM APIs that require valid XML names.
pub fn is_valid_xml_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        None => false,
        Some(first) => is_name_start_char(first) && chars.all(is_name_char),
    }
}
