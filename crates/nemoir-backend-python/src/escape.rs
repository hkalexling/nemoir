//! Python string-literal escaping for emitted constructor arguments.
//!
//! Produces a double-quoted Python string literal (without surrounding context),
//! escaping backslash, double-quote, all control and non-printable characters,
//! and unicode codepoints outside the printable ASCII range. The output is safe
//! to embed inside generated Python source even for adversarial inputs (e.g.
//! prompts containing `</script>` or backslash sequences).

/// Render `s` as a double-quoted Python string literal (including surrounding `"`).
pub fn python_string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => {
                let u = c as u32;
                if c.is_control() {
                    if u <= 0xFF {
                        out.push_str(&format!("\\x{:02x}", u));
                    } else if u <= 0xFFFF {
                        out.push_str(&format!("\\u{:04x}", u));
                    } else {
                        out.push_str(&format!("\\U{:08x}", u));
                    }
                } else if u <= 0x7E {
                    // Printable ASCII (excluding the special cases handled above).
                    out.push(c);
                } else if u <= 0xFFFF {
                    out.push_str(&format!("\\u{:04x}", u));
                } else {
                    out.push_str(&format!("\\U{:08x}", u));
                }
            }
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_string_keeps_quotes() {
        assert_eq!(python_string_literal("task"), "\"task\"");
    }

    #[test]
    fn empty_string() {
        assert_eq!(python_string_literal(""), "\"\"");
    }

    #[test]
    fn escapes_double_quote() {
        assert_eq!(python_string_literal("a\"b"), "\"a\\\"b\"");
    }

    #[test]
    fn escapes_backslash() {
        assert_eq!(python_string_literal("a\\b"), "\"a\\\\b\"");
    }

    #[test]
    fn escapes_newline_tab_cr() {
        assert_eq!(python_string_literal("a\nb\tc\rd"), "\"a\\nb\\tc\\rd\"");
    }

    #[test]
    fn escapes_control_chars() {
        // BEL (0x07) and NUL (0x00) are control characters.
        let lit = python_string_literal("\u{0007}\u{0000}");
        assert_eq!(lit, "\"\\x07\\x00\"");
    }

    #[test]
    fn escapes_non_ascii_printable() {
        let lit = python_string_literal("\u{00e7}"); // 'ç'
        assert_eq!(lit, "\"\\u00e7\"");
    }

    #[test]
    fn escapes_supplementary_plane() {
        let lit = python_string_literal("\u{1F600}"); // '😀'
        assert_eq!(lit, "\"\\U0001f600\"");
    }

    #[test]
    fn script_close_tag_does_not_leak_unchanged() {
        let lit = python_string_literal("</script>");
        // The slash and angle brackets are printable ASCII, so they pass through,
        // but the result must still be a balanced quoted Python string.
        assert_eq!(lit, "\"</script>\"");
        // No raw unescaped backslashes were introduced.
        assert_eq!(lit.matches('\\').count(), 0);
    }

    #[test]
    fn cranks_stay_balanced() {
        let lit = python_string_literal("prompt with </script> and \"quotes\" and \\backslash\\");
        assert!(lit.starts_with('"'));
        assert!(lit.ends_with('"'));
        assert!(lit.contains("\\\""));
        assert!(lit.contains("\\\\"));
        assert!(lit.contains("</script>"));
    }

    #[test]
    fn backslash_before_quote_does_not_collide() {
        // A trailing backslash must be escaped as \\, even if it precedes the closing quote.
        assert_eq!(python_string_literal("a\\"), "\"a\\\\\"");
    }
}
