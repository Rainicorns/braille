use libtest_mimic::{Arguments, Failed, Trial};
use serde_json::Value;
use std::path::Path;

// ---------------------------------------------------------------------------
// Token types parsed from JSON test input
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct Attr {
    name: String,
    value: String,
}

#[derive(Debug)]
enum Token {
    StartTag {
        tag: String,
        attrs: Vec<Attr>,
    },
    EndTag {
        tag: String,
    },
    EmptyTag {
        tag: String,
        attrs: Vec<Attr>,
    },
    Characters {
        text: String,
    },
    Comment {
        text: String,
    },
    Doctype {
        name: String,
        public_id: Option<String>,
        system_id: Option<String>,
    },
}

// ---------------------------------------------------------------------------
// JSON test parser
// ---------------------------------------------------------------------------

struct TestCase {
    description: String,
    tokens: Vec<Token>,
    expected: Vec<String>,
    file_name: String,
    should_skip: bool,
}

fn parse_attrs(val: &Value) -> Vec<Attr> {
    match val {
        Value::Array(arr) => arr
            .iter()
            .map(|a| Attr {
                name: a["name"].as_str().unwrap().to_string(),
                value: a["value"].as_str().unwrap().to_string(),
            })
            .collect(),
        Value::Object(map) => map
            .iter()
            .map(|(k, v)| Attr {
                name: k.clone(),
                value: v.as_str().unwrap_or("").to_string(),
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn parse_token(arr: &Value) -> Token {
    let token_type = arr[0].as_str().unwrap();
    match token_type {
        "StartTag" => {
            let tag = arr[2].as_str().unwrap().to_string();
            let attrs = parse_attrs(&arr[3]);
            Token::StartTag { tag, attrs }
        }
        "EndTag" => {
            let tag = arr[2].as_str().unwrap().to_string();
            Token::EndTag { tag }
        }
        "EmptyTag" => {
            let tag = arr[1].as_str().unwrap().to_string();
            let attrs = parse_attrs(&arr[2]);
            Token::EmptyTag { tag, attrs }
        }
        "Characters" => {
            let text = arr[1].as_str().unwrap().to_string();
            Token::Characters { text }
        }
        "Comment" => {
            let text = arr[1].as_str().unwrap().to_string();
            Token::Comment { text }
        }
        "Doctype" => {
            let name = arr[1].as_str().unwrap().to_string();
            let public_id = arr.get(2).and_then(|v| v.as_str()).map(|s| s.to_string());
            let system_id = arr.get(3).and_then(|v| v.as_str()).map(|s| s.to_string());
            Token::Doctype {
                name,
                public_id,
                system_id,
            }
        }
        other => panic!("Unknown token type: {other}"),
    }
}

fn load_test_file(path: &Path) -> Vec<TestCase> {
    let contents = std::fs::read_to_string(path).unwrap();
    let json: Value = serde_json::from_str(&contents).unwrap();
    let file_name = path.file_name().unwrap().to_string_lossy().to_string();

    let skip_files = ["options.test", "injectmeta.test", "whitespace.test"];
    let should_skip = skip_files.contains(&file_name.as_str());

    let tests = json["tests"].as_array().unwrap();
    tests
        .iter()
        .map(|t| {
            let description = t["description"].as_str().unwrap().to_string();
            let tokens: Vec<Token> = t["input"]
                .as_array()
                .unwrap()
                .iter()
                .map(parse_token)
                .collect();
            let expected: Vec<String> = t["expected"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect();
            // Skip tests with non-default options that change serialization behavior
            // "encoding" is informational and doesn't affect output
            let has_behavioral_options = if let Some(opts) = t.get("options") {
                if let Some(obj) = opts.as_object() {
                    obj.keys().any(|k| k != "encoding")
                } else {
                    false
                }
            } else {
                false
            };
            TestCase {
                description,
                tokens,
                expected,
                file_name: file_name.clone(),
                should_skip: should_skip || has_behavioral_options,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Void elements
// ---------------------------------------------------------------------------

fn is_void_element(tag: &str) -> bool {
    matches!(
        tag,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

// ---------------------------------------------------------------------------
// Raw text elements (no escaping inside these)
// ---------------------------------------------------------------------------

fn is_raw_text_element(tag: &str) -> bool {
    matches!(
        tag,
        "script" | "style" | "xmp" | "iframe" | "noembed" | "noframes" | "plaintext"
    )
}

// ---------------------------------------------------------------------------
// Attribute serialization
// ---------------------------------------------------------------------------

fn serialize_attr(attr: &Attr) -> String {
    let name = &attr.name;
    let value = &attr.value;

    // Boolean attribute: name == value -> emit just name
    if name == value {
        return name.clone();
    }

    // Empty value: must quote
    if value.is_empty() {
        return format!("{name}=\"\"");
    }

    // Check if value needs quoting
    let needs_quoting = value.chars().any(|c| {
        matches!(
            c,
            ' ' | '\t' | '\n' | '\r' | '\x0C' | '"' | '\'' | '=' | '>' | '`'
        )
    });

    if !needs_quoting {
        // Unquoted, but still escape &
        let escaped = value.replace('&', "&amp;");
        return format!("{name}={escaped}");
    }

    let has_double = value.contains('"');
    let has_single = value.contains('\'');

    if has_double && !has_single {
        // Single-quote, escape &
        let escaped = value.replace('&', "&amp;");
        return format!("{name}='{escaped}'");
    }

    // Double-quote, escape & and "
    let escaped = value.replace('&', "&amp;").replace('"', "&quot;");
    format!("{name}=\"{escaped}\"")
}

fn serialize_attrs(attrs: &[Attr]) -> String {
    if attrs.is_empty() {
        return String::new();
    }
    let mut parts = Vec::new();
    for attr in attrs {
        parts.push(serialize_attr(attr));
    }
    format!(" {}", parts.join(" "))
}

// ---------------------------------------------------------------------------
// Text escaping
// ---------------------------------------------------------------------------

fn escape_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\u{00A0}' => out.push_str("&nbsp;"),
            _ => out.push(c),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Optional tag omission
// ---------------------------------------------------------------------------

/// Elements whose start tag causes `</p>` to be omittable
fn is_p_closing_element(tag: &str) -> bool {
    matches!(
        tag,
        "address"
            | "article"
            | "aside"
            | "blockquote"
            | "datagrid"
            | "dialog"
            | "dir"
            | "div"
            | "dl"
            | "fieldset"
            | "footer"
            | "form"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "header"
            | "hr"
            | "menu"
            | "nav"
            | "ol"
            | "p"
            | "pre"
            | "section"
            | "table"
            | "ul"
    )
}

/// What the next token looks like (for peeking)
enum NextToken<'a> {
    StartTag(&'a str),
    EndTag(&'a str),
    EmptyTag(&'a str),
    Comment,
    Characters(&'a str),
    Eof,
}

fn peek_next<'a>(tokens: &'a [Token], i: usize) -> NextToken<'a> {
    if i + 1 >= tokens.len() {
        return NextToken::Eof;
    }
    match &tokens[i + 1] {
        Token::StartTag { tag, .. } => NextToken::StartTag(tag),
        Token::EndTag { tag } => NextToken::EndTag(tag),
        Token::EmptyTag { tag, .. } => NextToken::EmptyTag(tag),
        Token::Comment { .. } => NextToken::Comment,
        Token::Characters { text } => NextToken::Characters(text),
        Token::Doctype { .. } => NextToken::Eof, // treat doctype as non-matching
    }
}

fn can_omit_start_tag(tag: &str, attrs: &[Attr], tokens: &[Token], i: usize) -> bool {
    // Can never omit a start tag that has attributes (we'd lose them)
    if !attrs.is_empty() {
        return false;
    }
    let next = peek_next(tokens, i);
    match tag {
        "html" => {
            // Omit <html> unless next is a comment or space character
            match next {
                NextToken::Comment => false,
                NextToken::Characters(text) => !text.starts_with(|c: char| {
                    c == ' ' || c == '\t' || c == '\n' || c == '\r' || c == '\x0C'
                }),
                _ => true,
            }
        }
        "head" => {
            // Omit <head> if next is an element (start tag or empty tag) or its own end tag or EOF
            matches!(
                next,
                NextToken::StartTag(_) | NextToken::EmptyTag(_) | NextToken::EndTag(_) | NextToken::Eof
            )
        }
        "body" => {
            // Omit <body> unless next is space char or comment
            match next {
                NextToken::Comment => false,
                NextToken::Characters(text) => !text.starts_with(|c: char| {
                    c == ' ' || c == '\t' || c == '\n' || c == '\r' || c == '\x0C'
                }),
                _ => true,
            }
        }
        "colgroup" => {
            // Omit if next is <col> (either StartTag or EmptyTag with tag "col")
            // Only if not preceded by another omitted colgroup end tag
            let col_next = matches!(
                next,
                NextToken::StartTag(t) if t == "col"
            ) || matches!(
                next,
                NextToken::EmptyTag(t) if t == "col"
            );
            if !col_next {
                return false;
            }
            // Can only omit if preceded by nothing problematic
            // If the previous token was a </colgroup> that might itself be omitted,
            // we can't omit both. Check if prev was an end tag for colgroup/thead/tbody/tfoot.
            if i > 0 {
                if let Token::EndTag { tag: prev_tag } = &tokens[i - 1] {
                    if prev_tag == "colgroup" {
                        return false;
                    }
                }
            }
            true
        }
        "tbody" => {
            // Omit <tbody> if first thing inside is <tr>
            let tr_next = matches!(next, NextToken::StartTag(t) if t == "tr");
            if !tr_next {
                return false;
            }
            // Can't omit if preceded by </tbody>, </thead>, or </tfoot> that would itself be omitted
            if i > 0 {
                if let Token::EndTag { tag: prev_tag } = &tokens[i - 1] {
                    if prev_tag == "tbody" || prev_tag == "thead" || prev_tag == "tfoot" {
                        return false;
                    }
                }
            }
            true
        }
        _ => false,
    }
}

fn can_omit_end_tag(tag: &str, tokens: &[Token], i: usize) -> bool {
    let next = peek_next(tokens, i);
    match tag {
        "html" => {
            // Omit </html> unless next is a comment or space character
            match next {
                NextToken::Comment => false,
                NextToken::Characters(text) => !text.starts_with(|c: char| {
                    c == ' ' || c == '\t' || c == '\n' || c == '\r' || c == '\x0C'
                }),
                _ => true,
            }
        }
        "head" => {
            // Omit </head> unless next is space char or comment
            match next {
                NextToken::Comment => false,
                NextToken::Characters(text) => !text.starts_with(|c: char| {
                    c == ' ' || c == '\t' || c == '\n' || c == '\r' || c == '\x0C'
                }),
                _ => true,
            }
        }
        "body" => {
            // Omit </body> unless next is a comment or space character
            match next {
                NextToken::Comment => false,
                NextToken::Characters(text) => !text.starts_with(|c: char| {
                    c == ' ' || c == '\t' || c == '\n' || c == '\r' || c == '\x0C'
                }),
                _ => true,
            }
        }
        "li" => {
            // Omit </li> if followed by <li>, </anything> (parent end), or EOF
            matches!(
                next,
                NextToken::StartTag("li") | NextToken::EndTag(_) | NextToken::Eof
            )
        }
        "dt" => {
            // Omit </dt> if followed by <dt> or <dd>
            matches!(next, NextToken::StartTag("dt") | NextToken::StartTag("dd"))
        }
        "dd" => {
            // Omit </dd> if followed by <dd>, <dt>, </anything> (parent end), or EOF
            matches!(
                next,
                NextToken::StartTag("dd")
                    | NextToken::StartTag("dt")
                    | NextToken::EndTag(_)
                    | NextToken::Eof
            )
        }
        "p" => {
            // Omit </p> if followed by specific block elements, </anything>, EmptyTag("hr"), or EOF
            match next {
                NextToken::StartTag(t) => is_p_closing_element(t),
                NextToken::EmptyTag(t) => is_p_closing_element(t),
                NextToken::EndTag(_) | NextToken::Eof => true,
                _ => false,
            }
        }
        "optgroup" => {
            // Omit </optgroup> if followed by <optgroup>, </anything>, or EOF
            matches!(
                next,
                NextToken::StartTag("optgroup") | NextToken::EndTag(_) | NextToken::Eof
            )
        }
        "option" => {
            // Omit </option> if followed by <option>, <optgroup>, </anything>, or EOF
            matches!(
                next,
                NextToken::StartTag("option")
                    | NextToken::StartTag("optgroup")
                    | NextToken::EndTag(_)
                    | NextToken::Eof
            )
        }
        "colgroup" => {
            // Omit </colgroup> unless followed by comment or space
            match next {
                NextToken::Comment => false,
                NextToken::Characters(text) => !text.starts_with(|c: char| {
                    c == ' ' || c == '\t' || c == '\n' || c == '\r' || c == '\x0C'
                }),
                _ => true,
            }
        }
        "thead" => {
            // Omit </thead> if followed by <tbody> or <tfoot>
            matches!(
                next,
                NextToken::StartTag("tbody") | NextToken::StartTag("tfoot")
            )
        }
        "tbody" => {
            // Omit </tbody> if followed by <tbody>, <tfoot>, </anything>, or EOF
            matches!(
                next,
                NextToken::StartTag("tbody")
                    | NextToken::StartTag("tfoot")
                    | NextToken::EndTag(_)
                    | NextToken::Eof
            )
        }
        "tfoot" => {
            // Omit </tfoot> if followed by <tbody>, </anything>, or EOF
            matches!(
                next,
                NextToken::StartTag("tbody") | NextToken::EndTag(_) | NextToken::Eof
            )
        }
        "tr" => {
            // Omit </tr> if followed by <tr>, </anything>, or EOF
            matches!(
                next,
                NextToken::StartTag("tr") | NextToken::EndTag(_) | NextToken::Eof
            )
        }
        "td" => {
            // Omit </td> if followed by <td>, <th>, </anything>, or EOF
            matches!(
                next,
                NextToken::StartTag("td")
                    | NextToken::StartTag("th")
                    | NextToken::EndTag(_)
                    | NextToken::Eof
            )
        }
        "th" => {
            // Omit </th> if followed by <th>, <td>, </anything>, or EOF
            matches!(
                next,
                NextToken::StartTag("th")
                    | NextToken::StartTag("td")
                    | NextToken::EndTag(_)
                    | NextToken::Eof
            )
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Token-stream serializer
// ---------------------------------------------------------------------------

fn serialize_tokens(tokens: &[Token]) -> String {
    let mut out = String::new();
    let mut raw_text_context: Option<String> = None;

    for (i, token) in tokens.iter().enumerate() {
        match token {
            Token::StartTag { tag, attrs } => {
                if is_void_element(tag) {
                    // Void elements: emit start tag, no end tag
                    if !can_omit_start_tag(tag, attrs, tokens, i) {
                        out.push_str(&format!("<{tag}{}>", serialize_attrs(attrs)));
                    }
                } else if can_omit_start_tag(tag, attrs, tokens, i) {
                    // Omittable start tag: skip
                    // But still track raw text context
                    if is_raw_text_element(tag) {
                        raw_text_context = Some(tag.clone());
                    }
                } else {
                    out.push_str(&format!("<{tag}{}>", serialize_attrs(attrs)));
                    if is_raw_text_element(tag) {
                        raw_text_context = Some(tag.clone());
                    }
                }
            }
            Token::EndTag { tag } => {
                if is_raw_text_element(tag) {
                    raw_text_context = None;
                }
                if !is_void_element(tag) && !can_omit_end_tag(tag, tokens, i) {
                    out.push_str(&format!("</{tag}>"));
                }
            }
            Token::EmptyTag { tag, attrs } => {
                // EmptyTag is always emitted (it's a void element token)
                // But check if p end-tag omission should apply if this is hr
                out.push_str(&format!("<{tag}{}>", serialize_attrs(attrs)));
            }
            Token::Characters { text } => {
                if raw_text_context.is_some() {
                    // Inside raw text element: no escaping
                    out.push_str(text);
                } else {
                    out.push_str(&escape_text(text));
                }
            }
            Token::Comment { text } => {
                out.push_str(&format!("<!--{text}-->"));
            }
            Token::Doctype {
                name,
                public_id,
                system_id,
            } => {
                match (public_id, system_id) {
                    (Some(pub_id), Some(sys_id)) => {
                        if pub_id.is_empty() {
                            // Empty public ID means use SYSTEM keyword
                            out.push_str(&format!("<!DOCTYPE {name} SYSTEM \"{sys_id}\">"));
                        } else {
                            out.push_str(&format!(
                                "<!DOCTYPE {name} PUBLIC \"{pub_id}\" \"{sys_id}\">"
                            ));
                        }
                    }
                    (Some(pub_id), None) => {
                        out.push_str(&format!("<!DOCTYPE {name} PUBLIC \"{pub_id}\">"));
                    }
                    _ => {
                        out.push_str(&format!("<!DOCTYPE {name}>"));
                    }
                }
            }
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Test runner
// ---------------------------------------------------------------------------

fn run_test(test: &TestCase) -> Result<(), Failed> {
    let actual = serialize_tokens(&test.tokens);

    // The expected field is an array — any match is a pass
    if test.expected.iter().any(|e| *e == actual) {
        Ok(())
    } else {
        Err(format!(
            "Description: {}\n\nTokens: {:?}\n\nExpected (any of): {:?}\n\nActual: {:?}\n",
            test.description, test.tokens, test.expected, actual
        )
        .into())
    }
}

fn main() {
    let args = Arguments::from_args();

    let test_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/html5lib-tests/serializer");

    let mut trials = Vec::new();

    let mut test_files: Vec<_> = std::fs::read_dir(&test_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "test")
                .unwrap_or(false)
        })
        .collect();
    test_files.sort_by_key(|e| e.file_name());

    for entry in test_files {
        let path = entry.path();
        let tests = load_test_file(&path);

        for (idx, test) in tests.into_iter().enumerate() {
            let name = format!(
                "{}::{}::{}",
                test.file_name.trim_end_matches(".test"),
                idx,
                test.description
            );

            let ignored = test.should_skip;

            trials.push(
                Trial::test(name, move || run_test(&test)).with_ignored_flag(ignored),
            );
        }
    }

    libtest_mimic::run(&args, trials).exit();
}
