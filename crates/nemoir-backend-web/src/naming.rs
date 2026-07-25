//! Workflow id → web package directory name conversion and TS identifier
//! validation.
//!
//! Mirrors `nemoir-backend-python::naming` but with kebab-case conventions
//! for npm package directories and TS-safe identifier checks for generated
//! interface fields.

/// Convert a workflow id to a kebab-case npm-style package directory name.
///
/// PascalCase / camelCase ids get a `-` inserted before each uppercase
/// letter that follows a lowercase letter or digit, then lowercased.
/// Existing `-` / `_` separators in the input are normalized to `-`.
///
/// Returns `None` when the result is empty, starts with `-`, or contains
/// characters outside the supported set.
///
/// Examples:
///   "CodingAgent"    -> "coding-agent"
///   "JudgeCandidate" -> "judge-candidate"
///   "MyAPI"          -> "my-api"   (consecutive capitals group)
///   "FileProcessor"  -> "file-processor"
///   "coding_agent"    -> "coding-agent"  (underscores normalized)
pub fn package_dir(workflow_id: &str) -> Option<String> {
    let chars: Vec<char> = workflow_id.chars().collect();
    let mut out = String::with_capacity(workflow_id.len() + 4);

    for (i, c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() {
            // Insert a separator before an uppercase letter that follows a
            // lowercase letter or digit (so "CodingAgent" -> "coding-agent",
            // but "MyAPI" -> "my-api" without "my-a-p-i").
            if i > 0 {
                let prev = chars[i - 1];
                if prev.is_ascii_lowercase() || prev.is_ascii_digit() {
                    out.push('-');
                }
            }
            out.push(c.to_ascii_lowercase());
        } else if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(*c);
        } else if *c == '_' || *c == '-' {
            // Normalize existing separators to '-', collapsing runs.
            if !out.is_empty() && !out.ends_with('-') {
                out.push('-');
            }
        } else {
            return None;
        }
    }

    finalize_package_dir(&out)
}

/// Validate the final kebab-case directory name.
fn finalize_package_dir(s: &str) -> Option<String> {
    if s.is_empty() || s.starts_with('-') || s.ends_with('-') {
        return None;
    }
    // The first character must be a lowercase letter (npm package names
    // and directory names should not start with a digit or dash).
    let first = s.chars().next().unwrap();
    if !first.is_ascii_lowercase() {
        return None;
    }
    // Must match [a-z0-9-]+ with no consecutive dashes beyond what we already
    // collapsed. package_dir guarantees this, but double-check defensively.
    if !s
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return None;
    }
    Some(s.to_string())
}

/// Returns true iff `s` is a valid TypeScript identifier usable as an
/// interface property name.
///
/// First character: ASCII letter, underscore, or `$`.
/// Rest: ASCII alphanumeric, underscore, or `$`.
///
/// TypeScript permits reserved words as property names (e.g.
/// `interface Foo { delete: string }` compiles fine), so we intentionally
/// do NOT reject JS/TS keywords here — only structural validity.
pub fn is_valid_ts_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    if !(first.is_ascii_alphabetic() || first == '_' || first == '$') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pascal_case_to_kebab() {
        assert_eq!(package_dir("CodingAgent").as_deref(), Some("coding-agent"));
        assert_eq!(
            package_dir("JudgeCandidate").as_deref(),
            Some("judge-candidate")
        );
        assert_eq!(
            package_dir("FileProcessor").as_deref(),
            Some("file-processor")
        );
    }

    #[test]
    fn camel_case_to_kebab() {
        assert_eq!(package_dir("codingAgent").as_deref(), Some("coding-agent"));
    }

    #[test]
    fn consecutive_capitals_group() {
        // MyAPI -> M,y,A,P,I
        // M (i=0) -> "m"
        // y (i=1, lower) -> "y"
        // A (i=2, prev=y lower) -> "-a"
        // P (i=3, prev=A upper) -> "p"   (no separator)
        // I (i=4, prev=P upper) -> "i"  (no separator)
        // => "my-api"
        assert_eq!(package_dir("MyAPI").as_deref(), Some("my-api"));
    }

    #[test]
    fn with_digit_boundary() {
        assert_eq!(package_dir("Foo2Bar").as_deref(), Some("foo2-bar"));
    }

    #[test]
    fn underscore_normalized_to_hyphen() {
        assert_eq!(package_dir("coding_agent").as_deref(), Some("coding-agent"));
    }

    #[test]
    fn already_kebab_passes() {
        assert_eq!(package_dir("coding-agent").as_deref(), Some("coding-agent"));
    }

    #[test]
    fn single_word() {
        assert_eq!(package_dir("Foo").as_deref(), Some("foo"));
    }

    #[test]
    fn rejects_empty() {
        assert_eq!(package_dir(""), None);
    }

    #[test]
    fn rejects_leading_digit() {
        assert_eq!(package_dir("123abc"), None);
        assert_eq!(package_dir("1foo"), None);
    }

    #[test]
    fn rejects_non_identifier_chars() {
        assert_eq!(package_dir("Foo Bar"), None);
        assert_eq!(package_dir("Foo.Bar"), None);
        assert_eq!(package_dir("Fran\u{00e7}ais"), None);
    }

    #[test]
    fn rejects_trailing_separator() {
        // Trailing underscore becomes a trailing dash, which is invalid.
        assert_eq!(package_dir("Foo_"), None);
    }

    #[test]
    fn ts_identifier_accepts_simple_names() {
        assert!(is_valid_ts_identifier("task"));
        assert!(is_valid_ts_identifier("cwd"));
        assert!(is_valid_ts_identifier("summary"));
        assert!(is_valid_ts_identifier("learnerCode"));
        assert!(is_valid_ts_identifier("_private"));
        assert!(is_valid_ts_identifier("score2"));
    }

    #[test]
    fn ts_identifier_rejects_empty() {
        assert!(!is_valid_ts_identifier(""));
    }

    #[test]
    fn ts_identifier_rejects_leading_digit() {
        assert!(!is_valid_ts_identifier("1task"));
        assert!(!is_valid_ts_identifier("123"));
    }

    #[test]
    fn ts_identifier_rejects_hyphen_and_dot() {
        assert!(!is_valid_ts_identifier("task-class"));
        assert!(!is_valid_ts_identifier("task.class"));
        assert!(!is_valid_ts_identifier("two words"));
    }

    #[test]
    fn ts_identifier_allows_reserved_words() {
        // TS permits reserved words as property names in interfaces.
        assert!(is_valid_ts_identifier("delete"));
        assert!(is_valid_ts_identifier("class"));
        assert!(is_valid_ts_identifier("return"));
        assert!(is_valid_ts_identifier("await"));
    }
}
