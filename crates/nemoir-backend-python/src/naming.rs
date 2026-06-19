//! Workflow id -> Python package/distribution name conversion.

/// Convert a workflow id (typically CamelCase) to a snake_case Python package name.
///
/// Returns `None` if the result is not a valid Python identifier (`^[a-z_][a-z0-9_]*$`),
/// is empty, or contains characters outside the supported set.
///
/// Examples:
///   "CodingAgent" -> "coding_agent"
///   "MyAPI"       -> "my_api"
///   "Foo"         -> "foo"
///   "coding_agent" -> "coding_agent"  (lowercase ids pass through unchanged)
pub fn package_name(workflow_id: &str) -> Option<String> {
    let mut out = String::with_capacity(workflow_id.len() + 4);
    let chars: Vec<char> = workflow_id.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() {
            // Insert an underscore before an uppercase letter that follows a lower letter or digit.
            if i > 0 {
                let prev = chars[i - 1];
                if prev.is_ascii_lowercase() || prev.is_ascii_digit() {
                    out.push('_');
                }
            }
            out.push(c.to_ascii_lowercase());
        } else if c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_' {
            out.push(*c);
        } else {
            // Unsupported character: not a valid Python identifier component.
            return None;
        }
    }

    if is_valid_python_module_name(&out) {
        Some(out)
    } else {
        None
    }
}

/// Convert a workflow id to a PEP 503 distribution name (hyphenated).
///
/// Returns the hyphenated form of the package name conversion. If the workflow id
/// cannot be converted to a valid package name, this returns a best-effort string
/// (caller is expected to call `package_name` first for validation).
///
/// Examples:
///   "CodingAgent" -> "coding-agent"
///   "MyAPI"       -> "my-api"
pub fn distribution_name(workflow_id: &str) -> String {
    match package_name(workflow_id) {
        Some(name) => name.replace('_', "-"),
        None => workflow_id.to_lowercase(),
    }
}

fn is_valid_python_module_name(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    if !(first.is_ascii_lowercase() || first == '_') {
        return false;
    }
    if !chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
        return false;
    }
    // F6: reject Python reserved keywords. A workflow id like `Class` converts
    // to package name `class`, which would produce invalid generated Python
    // such as `from class._agent import Agent`. Soft keywords (`match`,
    // `case`) are allowed because Python permits them as module names.
    if PYTHON_KEYWORDS.contains(&s) {
        return false;
    }
    true
}

/// Python reserved keywords (CPython 3.11+). Soft keywords like `match` and
/// `case` are intentionally excluded: Python allows them as identifiers, so
/// they are safe to emit as dataclass field names.
const PYTHON_KEYWORDS: &[&str] = &[
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class", "continue",
    "def", "del", "elif", "else", "except", "finally", "for", "from", "global", "if", "import",
    "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while",
    "with", "yield",
];

/// Returns true iff `s` is a valid Python identifier that is not a reserved
/// keyword. Used to validate IR names that the Python backend emits directly
/// as dataclass field names (`AgentInput.x`, `AgentOutput.y`) or as attribute
/// accesses (`inputs.x`).
pub fn is_valid_python_field_name(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    // The first character must be a letter or underscore (NOT a digit, hyphen,
    // dot, space, or any other non-identifier character). Checking only
    // `is_ascii_digit` here was the F5 bug: it let `-bad`, `.bad`, ` bad`
    // through because the first char is not a digit and the `.all()` tail
    // check skipped the first character entirely.
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return false;
    }
    if PYTHON_KEYWORDS.contains(&s) {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coding_agent_to_coding_agent() {
        assert_eq!(package_name("CodingAgent").as_deref(), Some("coding_agent"));
        assert_eq!(distribution_name("CodingAgent"), "coding-agent");
    }

    #[test]
    fn my_api() {
        assert_eq!(package_name("MyAPI").as_deref(), Some("my_api"));
        assert_eq!(distribution_name("MyAPI"), "my-api");
    }

    #[test]
    fn foo() {
        assert_eq!(package_name("Foo").as_deref(), Some("foo"));
        assert_eq!(distribution_name("Foo"), "foo");
    }

    #[test]
    fn lowercase_passes_through() {
        assert_eq!(
            package_name("coding_agent").as_deref(),
            Some("coding_agent")
        );
        assert_eq!(distribution_name("coding_agent"), "coding-agent");
    }

    #[test]
    fn single_char() {
        assert_eq!(package_name("A").as_deref(), Some("a"));
        assert_eq!(package_name("a").as_deref(), Some("a"));
    }

    #[test]
    fn with_digit_boundary() {
        assert_eq!(package_name("Foo2Bar").as_deref(), Some("foo2_bar"));
    }

    #[test]
    fn leading_underscore_preserved() {
        assert_eq!(package_name("_Foo").as_deref(), Some("_foo"));
    }

    #[test]
    fn reject_leading_digit() {
        assert_eq!(package_name("123abc"), None);
    }

    #[test]
    fn reject_python_keywords_as_package_name() {
        // F6: workflow ids that convert to Python keywords must be rejected.
        // `Class` camelCase-converts to `class`, which is a Python keyword
        // and would produce invalid generated Python like
        // `from class._agent import Agent`.
        assert_eq!(package_name("Class"), None);
        // Already-lowercase keywords are also rejected.
        assert_eq!(package_name("class"), None);
        assert_eq!(package_name("for"), None);
        assert_eq!(package_name("return"), None);
        assert_eq!(package_name("import"), None);
        // `True`/`False`/`None` convert to `true`/`false`/`none`, which are
        // NOT Python keywords (Python is case-sensitive). These should pass.
        assert_eq!(package_name("True").as_deref(), Some("true"));
        assert_eq!(package_name("False").as_deref(), Some("false"));
        assert_eq!(package_name("None").as_deref(), Some("none"));
        // Soft keywords `match`/`case` are permitted as module names.
        assert_eq!(package_name("Match").as_deref(), Some("match"));
        assert_eq!(package_name("Case").as_deref(), Some("case"));
    }

    #[test]
    fn reject_empty() {
        assert_eq!(package_name(""), None);
    }

    #[test]
    fn reject_non_identifier_chars() {
        assert_eq!(package_name("Foo Bar"), None);
        assert_eq!(package_name("Foo-Bar"), None);
        assert_eq!(package_name("Foo.Bar"), None);
        assert_eq!(package_name("Fran\u{00e7}ais"), None);
    }

    #[test]
    fn distribution_falls_back_for_lowercase() {
        assert_eq!(distribution_name("coding_agent"), "coding-agent");
    }

    #[test]
    fn field_name_accepts_simple_identifiers() {
        assert!(is_valid_python_field_name("task"));
        assert!(is_valid_python_field_name("cwd"));
        assert!(is_valid_python_field_name("summary"));
        assert!(is_valid_python_field_name("_private"));
        assert!(is_valid_python_field_name("CamelCase"));
        assert!(is_valid_python_field_name("snake_case1"));
    }

    #[test]
    fn field_name_rejects_empty() {
        assert!(!is_valid_python_field_name(""));
    }

    #[test]
    fn field_name_rejects_leading_digit() {
        assert!(!is_valid_python_field_name("1task"));
        assert!(!is_valid_python_field_name("123"));
    }

    #[test]
    fn field_name_rejects_hyphen_and_dot() {
        assert!(!is_valid_python_field_name("task-class"));
        assert!(!is_valid_python_field_name("task.class"));
        assert!(!is_valid_python_field_name("two words"));
    }

    #[test]
    fn field_name_rejects_leading_non_identifier_chars() {
        // F5 regression: leading hyphen/dot/space were not rejected because
        // the old check only rejected leading DIGITS, and `.all()` on the tail
        // skipped the first character.
        assert!(!is_valid_python_field_name("-bad"));
        assert!(!is_valid_python_field_name(".bad"));
        assert!(!is_valid_python_field_name(" bad"));
        assert!(!is_valid_python_field_name("+bad"));
        assert!(!is_valid_python_field_name("@bad"));
    }

    #[test]
    fn field_name_rejects_python_keywords() {
        for kw in &[
            "class", "return", "if", "else", "for", "while", "import", "from", "as", "with",
            "True", "False", "None", "lambda", "yield", "async", "await",
        ] {
            assert!(
                !is_valid_python_field_name(kw),
                "expected {} to be rejected as a Python keyword",
                kw
            );
        }
    }

    #[test]
    fn field_name_allows_soft_keywords_match_and_case() {
        // `match` and `case` are soft keywords: Python permits them as identifiers.
        assert!(is_valid_python_field_name("match"));
        assert!(is_valid_python_field_name("case"));
    }

    #[test]
    fn field_name_allows_leading_underscore() {
        // `_manifest` is the conventional pattern for internal modules; field names
        // like `_x` are also valid Python identifiers.
        assert!(is_valid_python_field_name("_x"));
    }
}
