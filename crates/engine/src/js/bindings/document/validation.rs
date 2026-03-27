use boa_engine::{Context, JsError, JsResult};

// ---------------------------------------------------------------------------
// DOM "validate and extract" algorithm (namespace validation)
// https://dom.spec.whatwg.org/#validate-and-extract
// ---------------------------------------------------------------------------

/// Implements the DOM spec's "validate and extract a namespace and qualifiedName" algorithm.
/// Returns (namespace, prefix, local_name) or throws InvalidCharacterError / NamespaceError.
pub(crate) fn validate_and_extract(
    namespace: &str,
    qualified_name: &str,
    ctx: &mut Context,
) -> JsResult<(String, String, String)> {
    // Step 1: Validate the qualifiedName
    // For colon-containing names, split into prefix:localName and validate each.
    // For no-colon names, validate the whole name as an element name.
    if let Some(colon_pos) = qualified_name.find(':') {
        let prefix_part = &qualified_name[..colon_pos];
        let local_part = &qualified_name[colon_pos + 1..];
        let invalid_prefix =
            prefix_part.is_empty() || prefix_part.contains(['\0', '\t', '\n', '\x0C', '\r', ' ', '/', '>']);
        if invalid_prefix || !crate::dom::is_valid_element_name(local_part) {
            let exc =
                super::super::create_dom_exception(ctx, "InvalidCharacterError", "String contains an invalid character", 5)?;
            return Err(JsError::from_opaque(exc.into()));
        }
    } else {
        // No colon — validate the whole name as an element name
        if !qualified_name.is_empty() && !crate::dom::is_valid_element_name(qualified_name) {
            let exc =
                super::super::create_dom_exception(ctx, "InvalidCharacterError", "String contains an invalid character", 5)?;
            return Err(JsError::from_opaque(exc.into()));
        }
    }

    // Step 2: Extract prefix and localName
    let (prefix, local_name) = if let Some(colon_pos) = qualified_name.find(':') {
        (
            qualified_name[..colon_pos].to_string(),
            qualified_name[colon_pos + 1..].to_string(),
        )
    } else {
        (String::new(), qualified_name.to_string())
    };

    let ns = namespace.to_string();

    // Step 3: Namespace validation
    // 3a: prefix present but namespace is empty
    if !prefix.is_empty() && ns.is_empty() {
        let exc = super::super::create_dom_exception(ctx, "NamespaceError", "Namespace error", 14)?;
        return Err(JsError::from_opaque(exc.into()));
    }

    // 3b: prefix is "xml" but namespace is not the XML namespace
    if prefix == "xml" && ns != "http://www.w3.org/XML/1998/namespace" {
        let exc = super::super::create_dom_exception(ctx, "NamespaceError", "Namespace error", 14)?;
        return Err(JsError::from_opaque(exc.into()));
    }

    // 3c: prefix or qualifiedName is "xmlns" but namespace is not the XMLNS namespace
    if (prefix == "xmlns" || qualified_name == "xmlns") && ns != "http://www.w3.org/2000/xmlns/" {
        let exc = super::super::create_dom_exception(ctx, "NamespaceError", "Namespace error", 14)?;
        return Err(JsError::from_opaque(exc.into()));
    }

    // 3d: namespace is XMLNS but neither prefix nor qualifiedName is "xmlns"
    if ns == "http://www.w3.org/2000/xmlns/"
        && !qualified_name.is_empty()
        && prefix != "xmlns"
        && qualified_name != "xmlns"
    {
        let exc = super::super::create_dom_exception(ctx, "NamespaceError", "Namespace error", 14)?;
        return Err(JsError::from_opaque(exc.into()));
    }

    Ok((ns, prefix, local_name))
}
