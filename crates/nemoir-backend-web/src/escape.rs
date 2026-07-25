//! String escaping for embedding in generated TypeScript / JSON.
//!
//! For `workflow.json` we rely on `serde_json` which handles JSON escaping
//! safely. For raw string interpolation into generated `.ts` / `.tsx`
//! files we emit TypeScript template-literal-safe strings (backtick
//! strings) so prompts containing arbitrary JavaScript — including
//! `</script>`, quotes, and newlines — cannot escape the literal.

/// Render `s` as the body of a TypeScript template literal (between
/// backticks). Escapes backticks, `${`, and backslash so the literal
/// is unbreakable even for adversarial prompts.
///
/// The caller is responsible for adding the surrounding backticks:
///   `format!("`{}`", ts_template_body(&prompt))`
pub fn ts_template_body(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for c in s.chars() {
        match c {
            '`' => out.push_str("\\`"),
            '\\' => out.push_str("\\\\"),
            '$' => {
                // Only escape `$` when followed by `{`, which would start
                // a template interpolation. A bare `$` is safe.
                out.push('$');
            }
            _ => out.push(c),
        }
    }
    // Defensively neutralize any `${` sequences: we only pushed `$`
    // verbatim above, so a `{` following it in the input is already
    // included. Prevent interpolation by rewriting `${` to `\${`.
    out.replace("${", "\\${")
}

/// Render `s` as a complete backtick-delimited TypeScript template literal.
pub fn ts_template_literal(s: &str) -> String {
    format!("`{}`", ts_template_body(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_string_keeps_backticks() {
        assert_eq!(ts_template_literal("hello"), "`hello`");
    }

    #[test]
    fn empty_string() {
        assert_eq!(ts_template_literal(""), "``");
    }

    #[test]
    fn escapes_backtick() {
        assert_eq!(ts_template_literal("a`b"), "`a\\`b`");
    }

    #[test]
    fn escapes_backslash() {
        assert_eq!(ts_template_literal("a\\b"), "`a\\\\b`");
    }

    #[test]
    fn escapes_dollar_brace_to_prevent_interpolation() {
        assert_eq!(ts_template_literal("${evil}"), "`\\${evil}`");
        assert_eq!(ts_template_literal("price: $5"), "`price: $5`");
    }

    #[test]
    fn preserves_newlines_and_quotes() {
        let lit = ts_template_literal("line1\nline2 \"quoted\" 'single'");
        assert!(lit.starts_with('`'));
        assert!(lit.ends_with('`'));
        assert!(lit.contains("line1\nline2"));
        assert!(lit.contains("\"quoted\""));
    }

    #[test]
    fn script_close_tag_does_not_break_out() {
        let lit = ts_template_literal("</script>");
        // The literal should be balanced and contain the tag verbatim.
        assert_eq!(lit, "`</script>`");
        assert!(!lit.contains("\\`"));
    }
}
