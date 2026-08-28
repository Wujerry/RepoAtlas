use crate::error::{Error, Result};
use std::path::{Path, PathBuf};

pub fn canonicalize(path: &Path) -> Result<PathBuf> {
    let resolved = if path.exists() {
        dunce::canonicalize(path).map_err(|source| Error::Io {
            path: Some(path.to_path_buf()),
            source,
        })?
    } else {
        path.to_path_buf()
    };
    Ok(normalize(&resolved))
}

pub fn normalize(path: &Path) -> PathBuf {
    // `Path` already knows which separators and prefixes are valid for the
    // host platform.  Rewriting `/` to `\\` unconditionally makes a Unix
    // absolute path (for example, `/tmp/repo`) a relative path whose first
    // component is `\\tmp\\repo`, so parse and trim in the path domain first.
    let normalized = path.components().as_path();

    #[cfg(windows)]
    {
        // Windows accepts both separators, but keep the serialized form
        // native so database keys and API responses are stable regardless of
        // how the caller spelled the input.
        let value = normalized.to_string_lossy().replace('/', "\\");
        // canonicalize may return an extended-length path while UI/database paths use DOS syntax.
        let value = if let Some(rest) = value.strip_prefix(r"\\?\UNC\") {
            format!(r"\\{}", rest)
        } else if let Some(rest) = value.strip_prefix(r"\\?\") {
            rest.to_owned()
        } else {
            value
        };
        PathBuf::from(value)
    }

    #[cfg(not(windows))]
    {
        normalized.to_path_buf()
    }
}

pub fn path_to_string(path: &Path) -> String {
    normalize(path).to_string_lossy().into_owned()
}

pub fn is_within(child: &Path, parent: &Path) -> bool {
    let child = normalize(child);
    let parent = normalize(parent);

    // An empty parent is not a useful authorization boundary.  In
    // particular, `Path::starts_with("")` is true for every path.
    if parent.as_os_str().is_empty() {
        return child.as_os_str().is_empty();
    }

    #[cfg(windows)]
    {
        // Windows path comparison is case-insensitive and `components`
        // treats both `/` and `\\` as separators.  Comparing components
        // avoids false positives such as `repo-other` being inside `repo`.
        let child = windows_comparison_components(&child);
        let parent = windows_comparison_components(&parent);
        child.starts_with(&parent)
    }

    #[cfg(not(windows))]
    {
        // Unix filesystems are case-sensitive.  Path::starts_with compares
        // complete native components, so a sibling with a shared textual
        // prefix is not considered a descendant.
        child == parent || child.starts_with(&parent)
    }
}

#[cfg(windows)]
fn windows_comparison_components(path: &Path) -> Vec<String> {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy().to_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{is_within, normalize, path_to_string};
    use std::path::{Path, PathBuf};

    #[cfg(unix)]
    #[test]
    fn unix_paths_keep_native_separators_and_case() {
        let path = Path::new("/tmp/RepoAtlas/project/");

        assert_eq!(normalize(path), PathBuf::from("/tmp/RepoAtlas/project"));
        assert_eq!(path_to_string(path), "/tmp/RepoAtlas/project");
        assert!(is_within(
            Path::new("/tmp/RepoAtlas/project/src/main.rs"),
            Path::new("/tmp/RepoAtlas/project"),
        ));
        assert!(is_within(
            Path::new("/tmp/RepoAtlas/project"),
            Path::new("/tmp/RepoAtlas/project/"),
        ));
        assert!(!is_within(
            Path::new("/tmp/repoatlas/project/src/main.rs"),
            Path::new("/tmp/RepoAtlas/project"),
        ));
        assert!(!is_within(
            Path::new("/tmp/RepoAtlas/project-other/src/main.rs"),
            Path::new("/tmp/RepoAtlas/project"),
        ));
    }

    #[cfg(windows)]
    #[test]
    fn windows_paths_compare_native_separators_case_insensitively() {
        let path = Path::new("C:/Users/RepoAtlas/project/");

        assert_eq!(
            normalize(path),
            PathBuf::from(r"C:\Users\RepoAtlas\project")
        );
        assert_eq!(path_to_string(path), r"C:\Users\RepoAtlas\project");
        assert!(is_within(
            Path::new(r"c:\users\repoatlas\PROJECT\src\main.rs"),
            Path::new(r"C:/Users/RepoAtlas/project"),
        ));
        assert!(is_within(
            Path::new(r"c:\USERS\repoatlas\PROJECT\"),
            Path::new(r"C:/Users/RepoAtlas/project/"),
        ));
        assert!(!is_within(
            Path::new(r"C:\Users\RepoAtlas\project-other\src\main.rs"),
            Path::new(r"C:\Users\RepoAtlas\project"),
        ));
    }

    #[test]
    fn equivalent_paths_are_within_each_other() {
        let path = if cfg!(windows) {
            Path::new(r"C:\RepoAtlas\project")
        } else {
            Path::new("/tmp/RepoAtlas/project")
        };
        let equivalent = if cfg!(windows) {
            Path::new(r"c:/repoatlas/project/")
        } else {
            Path::new("/tmp/RepoAtlas/project/")
        };

        assert!(is_within(path, equivalent));
        assert!(is_within(equivalent, path));
    }

    #[cfg(windows)]
    #[test]
    fn extended_length_windows_paths_match_dos_paths() {
        assert!(is_within(
            Path::new(r"C:\RepoAtlas\project\scripts"),
            Path::new(r"\\?\C:\RepoAtlas\project"),
        ));
        assert!(is_within(
            Path::new(r"\\?\C:\RepoAtlas\project\scripts"),
            Path::new(r"C:\RepoAtlas\project"),
        ));
    }
}
