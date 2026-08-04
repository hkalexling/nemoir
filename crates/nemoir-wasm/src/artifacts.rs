//! Artifact file representation and path normalisation.
//!
//! Backends emit in-memory [`GeneratedFile`] values whose paths are relative
//! to a CLI output parent. This module normalises those paths so the
//! JavaScript ZIP layer can prefix them with a single `archiveRoot` without
//! doubling directory prefixes or exposing unsafe paths.

use serde::{Deserialize, Serialize};

use crate::api::Target;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single UTF-8 text file within a generated artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactFile {
    /// Safe UTF-8 path relative to [`Artifact::archive_root`].
    pub path: String,

    /// UTF-8 file content.
    pub content: String,
}

/// A complete generated artifact ready for ZIP packaging.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    /// The backend target that produced this artifact.
    pub target: Target,

    /// Package / project name (e.g. `"my-workflow"`).
    pub package_name: String,

    /// Top-level directory name inside the ZIP archive.
    pub archive_root: String,

    /// Files comprising the artifact, with paths relative to `archive_root`.
    pub files: Vec<ArtifactFile>,
}

// ---------------------------------------------------------------------------
// Path safety validation
// ---------------------------------------------------------------------------

/// Reject file paths that contain `..`, are absolute, use backslashes, or
/// contain dot / empty components. Current backends only produce safe paths,
/// but this guard is cheap and prevents any future slip.
pub fn is_safe_path(path: &str) -> bool {
    if path.is_empty() {
        return false;
    }
    // Reject backslash anywhere — only forward slash is allowed.
    if path.contains('\\') {
        return false;
    }
    // Reject absolute paths, drive-like paths, and control characters.
    if path.starts_with('/') || path.contains(':') || path.chars().any(char::is_control) {
        return false;
    }
    // Reject each component: must not be empty, ".", or "..".
    for component in path.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            return false;
        }
    }
    true
}

/// Validate that a value used as [`Artifact::archive_root`] is a single safe
/// directory-name component: non-empty, no path separators, and not `.` or
/// `..`.
pub fn is_safe_archive_root(root: &str) -> bool {
    if root.is_empty() {
        return false;
    }
    if root.contains('/')
        || root.contains('\\')
        || root.contains(':')
        || root.chars().any(char::is_control)
    {
        return false;
    }
    if root == "." || root == ".." {
        return false;
    }
    true
}

// ---------------------------------------------------------------------------
// PathBuf → String conversion
// ---------------------------------------------------------------------------

/// Convert a backend [`std::path::Path`] to a safe UTF-8 [`String`], then
/// validate it with [`is_safe_path`]. Returns `Err(msg)` when the path is
/// non-UTF-8 or fails the safety check.
pub fn normalize_path(path: &std::path::Path) -> Result<String, String> {
    let s = path
        .to_str()
        .ok_or_else(|| format!("non-UTF-8 path in generated file: {:?}", path))?;
    if !is_safe_path(s) {
        return Err(format!("unsafe path in generated file: {}", s));
    }
    Ok(s.to_string())
}

// ---------------------------------------------------------------------------
// Artifact normalization
// ---------------------------------------------------------------------------

/// Build an [`Artifact`] from a Python backend [`nemoir_backend_python::GeneratedPackage`].
///
/// - `archive_root` is the distribution name (e.g. `"policy-gated-edit"`).
/// - Package files retain their `{package_name}/` prefix so the import
///   package directory lives beneath the distribution archive root:
///   `pyproject.toml` plus `hello_workflow/_agent.py`, etc.
/// - The prefix is *not* flattened — Python packaging needs the nested
///   package directory for `pip install` compatibility.
pub fn normalize_python_artifact(
    pkg: &nemoir_backend_python::GeneratedPackage,
) -> Result<Artifact, String> {
    let mut files: Vec<ArtifactFile> = Vec::with_capacity(pkg.files.len());

    for f in &pkg.files {
        let rel = normalize_path(&f.relative_path)?;
        files.push(ArtifactFile {
            path: rel,
            content: f.content.clone(),
        });
    }

    if !is_safe_archive_root(&pkg.distribution_name) {
        return Err(format!(
            "unsafe archive_root for Python artifact: {}",
            pkg.distribution_name
        ));
    }

    Ok(Artifact {
        target: Target::Python,
        package_name: pkg.distribution_name.clone(),
        archive_root: pkg.distribution_name.clone(),
        files,
    })
}

/// Build an [`Artifact`] from a web backend [`nemoir_backend_web::GeneratedPackage`].
///
/// - `archive_root` is the kebab-case package directory name (e.g.
///   `"hint-tutor"`).
/// - All file paths from the backend are prefixed with `{package_name}/`;
///   this function strips that prefix so ZIP assembly does not double it.
/// - Web artifacts are intentionally flat beneath the archive root (the
///   web backend's own directory layout is self-contained).
pub fn normalize_web_artifact(
    pkg: &nemoir_backend_web::GeneratedPackage,
) -> Result<Artifact, String> {
    let prefix = format!("{}/", pkg.package_name);
    let mut files: Vec<ArtifactFile> = Vec::with_capacity(pkg.files.len());
    let mut bare: Vec<String> = Vec::new();

    for f in &pkg.files {
        let raw = normalize_path(&f.relative_path)?;
        let rel = raw
            .strip_prefix(&prefix)
            .ok_or_else(|| {
                format!(
                    "web backend file '{}' does not start with expected prefix '{}'",
                    raw, prefix
                )
            })?
            .to_string();
        bare.push(rel.clone());
        files.push(ArtifactFile {
            path: rel,
            content: f.content.clone(),
        });
    }

    // Belt-and-braces: verify no file escaped the prefix and ended up with
    // a path that, when joined with archive_root, would look like a prefixed
    // path. This catches bugs in the backend or the strip logic.
    for b in &bare {
        if b.starts_with(&prefix) || b.starts_with(&format!("{}/", pkg.package_name)) {
            return Err(format!(
                "web artifact path still has prefix after normalisation: {}",
                b
            ));
        }
    }

    if !is_safe_archive_root(&pkg.package_name) {
        return Err(format!(
            "unsafe archive_root for web artifact: {}",
            pkg.package_name
        ));
    }

    Ok(Artifact {
        target: Target::Web,
        package_name: pkg.package_name.clone(),
        archive_root: pkg.package_name.clone(),
        files,
    })
}

/// Build an [`Artifact`] from a visualizer HTML string.
///
/// - `archive_root` is derived from the workflow id, lowercased with
///   non-alphanumeric characters replaced by hyphens.
/// - Contains a single `index.html` file.
/// - Falls back to `"workflow"` when sanitisation yields an empty string.
pub fn normalize_visualizer_artifact(html: &str, workflow_id: &str) -> Artifact {
    let root = workflow_id.to_lowercase();
    // Replace any run of non-alphanumeric chars with a single hyphen.
    let root: String = root
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    // Collapse multiple adjacent hyphens.
    let mut clean = String::new();
    let mut last_was_hyphen = false;
    for ch in root.chars() {
        if ch == '-' {
            if !last_was_hyphen {
                clean.push('-');
                last_was_hyphen = true;
            }
        } else {
            clean.push(ch);
            last_was_hyphen = false;
        }
    }
    let root = clean.trim_matches('-').to_string();
    let root = if root.is_empty() {
        "workflow".to_string()
    } else {
        root
    };
    // Safety: the sanitisation above guarantees a safe single component.
    debug_assert!(
        is_safe_archive_root(&root),
        "visualizer archive_root should always be safe after sanitisation"
    );

    Artifact {
        target: Target::Visualizer,
        package_name: workflow_id.to_string(),
        archive_root: root,
        files: vec![ArtifactFile {
            path: "index.html".to_string(),
            content: html.to_string(),
        }],
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn safe_relative_paths() {
        assert!(is_safe_path("src/main.ts"));
        assert!(is_safe_path("pyproject.toml"));
        assert!(is_safe_path("my_package/__init__.py"));
        assert!(is_safe_path("my_wf/_agent.py"));
    }

    #[test]
    fn rejects_parent_traversal() {
        assert!(!is_safe_path("../etc/passwd"));
        assert!(!is_safe_path("foo/../../bar"));
    }

    #[test]
    fn rejects_absolute() {
        assert!(!is_safe_path("/etc/passwd"));
    }

    #[test]
    fn rejects_empty() {
        assert!(!is_safe_path(""));
    }

    #[test]
    fn rejects_backslash_drive_and_control_chars() {
        assert!(!is_safe_path("\\windows\\system32"));
        assert!(!is_safe_path("foo\\bar"));
        assert!(!is_safe_path("foo/bar\\baz"));
        assert!(!is_safe_path("C:/windows/system32"));
        assert!(!is_safe_path("foo\nbar"));
    }

    #[test]
    fn rejects_dot_component() {
        assert!(!is_safe_path("."));
        assert!(!is_safe_path("./foo"));
        assert!(!is_safe_path("foo/./bar"));
        assert!(!is_safe_path("foo/."));
    }

    #[test]
    fn rejects_empty_component() {
        assert!(!is_safe_path("foo//bar"));
        assert!(!is_safe_path("foo/"));
        assert!(!is_safe_path("/foo"));
    }

    #[test]
    fn safe_archive_root_accepts_single_component() {
        assert!(is_safe_archive_root("my-wf"));
        assert!(is_safe_archive_root("hello_workflow"));
        assert!(is_safe_archive_root("coding-agent"));
        assert!(is_safe_archive_root("a"));
        assert!(is_safe_archive_root("package123"));
    }

    #[test]
    fn safe_archive_root_rejects_problematic() {
        assert!(!is_safe_archive_root(""));
        assert!(!is_safe_archive_root("."));
        assert!(!is_safe_archive_root(".."));
        assert!(!is_safe_archive_root("foo/bar"));
        assert!(!is_safe_archive_root("foo\\bar"));
        assert!(!is_safe_archive_root("/root"));
        assert!(!is_safe_archive_root("../escape"));
        assert!(!is_safe_archive_root("C:root"));
        assert!(!is_safe_archive_root("root\nname"));
    }

    #[test]
    fn normalize_path_validates() {
        let p = PathBuf::from("src/main.ts");
        assert_eq!(normalize_path(&p).unwrap(), "src/main.ts");
    }

    #[test]
    fn normalize_path_rejects_traversal() {
        let p = PathBuf::from("../etc/passwd");
        assert!(normalize_path(&p).is_err());
    }

    #[test]
    fn normalize_path_rejects_backslash() {
        let p = PathBuf::from("foo\\bar");
        assert!(normalize_path(&p).is_err());
    }

    #[test]
    fn normalize_python_artifact_preserves_package_directory() {
        let pkg = nemoir_backend_python::GeneratedPackage {
            package_name: "my_wf".into(),
            distribution_name: "my-wf".into(),
            files: vec![
                nemoir_backend_python::GeneratedFile {
                    relative_path: PathBuf::from("my_wf/_agent.py"),
                    content: "# agent".into(),
                },
                nemoir_backend_python::GeneratedFile {
                    relative_path: PathBuf::from("my_wf/__init__.py"),
                    content: "# init".into(),
                },
                nemoir_backend_python::GeneratedFile {
                    relative_path: PathBuf::from("pyproject.toml"),
                    content: "[project]".into(),
                },
            ],
        };
        let artifact = normalize_python_artifact(&pkg).unwrap();
        assert_eq!(artifact.target, Target::Python);
        assert_eq!(artifact.archive_root, "my-wf");
        assert_eq!(artifact.package_name, "my-wf");

        let paths: Vec<&str> = artifact.files.iter().map(|f| f.path.as_str()).collect();
        assert!(
            paths.contains(&"my_wf/_agent.py"),
            "expected my_wf/_agent.py (preserved package dir), got {paths:?}"
        );
        assert!(
            paths.contains(&"my_wf/__init__.py"),
            "expected my_wf/__init__.py, got {paths:?}"
        );
        assert!(
            paths.contains(&"pyproject.toml"),
            "expected pyproject.toml, got {paths:?}"
        );
        // All paths must be safe
        for f in &artifact.files {
            assert!(is_safe_path(&f.path), "unsafe path: {}", f.path);
        }
    }

    #[test]
    fn normalize_python_artifact_rejects_unsafe_archive_root() {
        let pkg = nemoir_backend_python::GeneratedPackage {
            package_name: "pkg".into(),
            distribution_name: "../escape".into(),
            files: vec![nemoir_backend_python::GeneratedFile {
                relative_path: PathBuf::from("pyproject.toml"),
                content: "[project]".into(),
            }],
        };
        assert!(normalize_python_artifact(&pkg).is_err());
    }

    #[test]
    fn normalize_web_artifact_strips_package_prefix() {
        let pkg = nemoir_backend_web::GeneratedPackage {
            package_name: "hint-tutor".into(),
            files: vec![
                nemoir_backend_web::GeneratedFile {
                    relative_path: PathBuf::from("hint-tutor/package.json"),
                    content: "{}".into(),
                },
                nemoir_backend_web::GeneratedFile {
                    relative_path: PathBuf::from("hint-tutor/src/agent.ts"),
                    content: "// agent".into(),
                },
            ],
        };
        let artifact = normalize_web_artifact(&pkg).unwrap();
        assert_eq!(artifact.target, Target::Web);
        assert_eq!(artifact.archive_root, "hint-tutor");

        let paths: Vec<&str> = artifact.files.iter().map(|f| f.path.as_str()).collect();
        assert!(paths.contains(&"package.json"), "got {paths:?}");
        assert!(paths.contains(&"src/agent.ts"), "got {paths:?}");
        // No path should still contain the prefix
        for p in &paths {
            assert!(
                !p.starts_with("hint-tutor/"),
                "path still has prefix: {}",
                p
            );
        }
        // archive_root must be safe
        assert!(is_safe_archive_root(&artifact.archive_root));
    }

    #[test]
    fn normalize_web_artifact_rejects_missing_prefix() {
        let pkg = nemoir_backend_web::GeneratedPackage {
            package_name: "hint-tutor".into(),
            files: vec![nemoir_backend_web::GeneratedFile {
                relative_path: PathBuf::from("other/package.json"),
                content: "{}".into(),
            }],
        };
        assert!(normalize_web_artifact(&pkg).is_err());
    }

    #[test]
    fn normalize_web_artifact_rejects_unsafe_archive_root() {
        let pkg = nemoir_backend_web::GeneratedPackage {
            package_name: "../escape".into(),
            files: vec![nemoir_backend_web::GeneratedFile {
                relative_path: PathBuf::from("../escape/package.json"),
                content: "{}".into(),
            }],
        };
        assert!(normalize_web_artifact(&pkg).is_err());
    }

    #[test]
    fn visualizer_artifact_single_file() {
        let html = "<html></html>";
        let artifact = normalize_visualizer_artifact(html, "MyWorkflow");
        assert_eq!(artifact.target, Target::Visualizer);
        assert_eq!(artifact.package_name, "MyWorkflow");
        assert_eq!(artifact.files.len(), 1);
        assert_eq!(artifact.files[0].path, "index.html");
        assert_eq!(artifact.files[0].content, html);
        // archive_root should be sanitised
        assert!(!artifact.archive_root.contains(' '));
        assert!(is_safe_archive_root(&artifact.archive_root));
    }

    #[test]
    fn visualizer_artifact_fallback_for_all_special_chars() {
        let artifact = normalize_visualizer_artifact("<html></html>", "!@#$%");
        assert_eq!(artifact.archive_root, "workflow");
        assert!(is_safe_archive_root(&artifact.archive_root));
    }
}
