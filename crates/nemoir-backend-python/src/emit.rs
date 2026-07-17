//! Static template emitters for the generated package files that do not depend
//! on IR variant translation: `pyproject.toml` in particular.

/// Render `pyproject.toml` for a generated workflow package.
///
/// - `distribution_name`: PEP 503 hyphenated name (e.g. `coding-agent`).
/// - `package_name`: snake_case import name (e.g. `coding_agent`).
/// - `version`: package version string; defaults to `0.1.0` when not provided
///   by the caller.
pub fn emit_pyproject(distribution_name: &str, package_name: &str, version: &str) -> String {
    format!(
        "[project]\n\
name = \"{dist}\"\n\
version = \"{ver}\"\n\
requires-python = \">=3.11\"\n\
dependencies = [\n\
    \"nemoir-runtime>=0.9.2\",\n\
]\n\
\n\
[build-system]\n\
requires = [\"setuptools>=61.0\"]\n\
build-backend = \"setuptools.build_meta\"\n\
\n\
[tool.setuptools]\n\
packages = [\"{pkg}\"]\n",
        dist = distribution_name,
        ver = version,
        pkg = package_name,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pyproject_for_coding_agent() {
        let s = emit_pyproject("coding-agent", "coding_agent", "0.1.0");
        assert!(s.contains("name = \"coding-agent\""));
        assert!(s.contains("version = \"0.1.0\""));
        assert!(s.contains("requires-python = \">=3.11\""));
        assert!(s.contains("\"nemoir-runtime>=0.9.2\""));
        assert!(s.contains("packages = [\"coding_agent\"]"));
        assert!(s.contains("[build-system]"));
    }

    #[test]
    fn pyproject_version_override() {
        let s = emit_pyproject("foo", "foo", "1.2.3");
        assert!(s.contains("version = \"1.2.3\""));
    }
}
