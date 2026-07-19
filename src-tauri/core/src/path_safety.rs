//! Path denylist for sidecar-sourced paths (LUT, etc.) that never pass
//! through the Tauri `validate_user_path` IPC gate.
//! Also: Windows verbatim-prefix stripping so catalog keys match picker paths.

use std::path::{Component, Path, PathBuf};

/// Strip Windows `\\?\` / `\\?\UNC\` prefixes from a path string.
/// Rust `canonicalize()` on Windows yields these; the file picker does not.
pub fn simplify_path_str(path: &str) -> String {
    let path = path.trim().trim_end_matches(['/', '\\']);
    if let Some(rest) = path.strip_prefix(r"\\?\") {
        if let Some(unc) = rest.strip_prefix("UNC\\") {
            return format!(r"\\{unc}");
        }
        if let Some(unc) = rest.strip_prefix("UNC/") {
            return format!(r"\\{unc}");
        }
        return rest.to_string();
    }
    path.to_string()
}

/// Strip Windows verbatim prefixes from a [`PathBuf`].
pub fn simplify_path(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    let simple = simplify_path_str(&s);
    if simple.as_str() == s.as_ref() {
        path
    } else {
        PathBuf::from(simple)
    }
}

/// Sensitive prefixes we never open from sidecar / edit-doc paths.
fn is_denied(path: &Path) -> bool {
    let s = path.to_string_lossy();
    let lower = s.to_lowercase();

    let home_denied = [
        "/.ssh",
        "/.gnupg",
        "/.aws",
        "/.config/gcloud",
        "/.kube",
        "/.docker",
        "/library/keychains",
        "\\.ssh",
        "\\.gnupg",
        "\\.aws",
    ];
    if home_denied.iter().any(|d| lower.contains(d)) {
        return true;
    }

    #[cfg(unix)]
    {
        const DENY: &[&str] = &[
            "/etc",
            "/private/etc",
            "/var/root",
            // macOS resolves /var -> /private/var, so the canonicalized form
            // must be denied too (same reason /private/etc is listed).
            "/private/var/root",
            "/root",
            "/System",
            "/usr/bin",
            "/usr/sbin",
            "/bin",
            "/sbin",
            "/dev",
            "/proc",
            "/sys",
        ];
        for d in DENY {
            if path.starts_with(d) {
                return true;
            }
        }
    }

    #[cfg(windows)]
    {
        let deny = ["\\windows\\system32", "\\windows\\syswow64", "\\$recycle.bin"];
        if deny.iter().any(|d| lower.contains(d)) {
            return true;
        }
    }

    false
}

fn normalize_lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::Prefix(p) => out.push(p.as_os_str()),
            Component::RootDir => out.push(Component::RootDir.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(s) => out.push(s),
        }
    }
    out
}

/// Validate a filesystem path that may come from a sidecar (not IPC).
/// Returns `None` when empty, null-byte, denied, or unresolvable.
pub fn sanitize_user_path(path: &str) -> Option<PathBuf> {
    let path = path.trim();
    if path.is_empty() || path.contains('\0') {
        return None;
    }

    let raw = PathBuf::from(path);
    let normalized = normalize_lexical(&raw);

    let resolved = if normalized.exists() {
        normalized.canonicalize().ok()?
    } else if let Some(parent) = normalized.parent().filter(|p| !p.as_os_str().is_empty()) {
        if parent.exists() {
            let parent = parent.canonicalize().ok()?;
            let name = normalized.file_name()?;
            parent.join(name)
        } else {
            normalized
        }
    } else {
        normalized
    };

    if is_denied(&resolved) {
        return None;
    }
    Some(simplify_path(resolved))
}

/// Clear or rewrite `lut_file` when the sidecar path is unsafe.
pub fn sanitize_lut_path(path: Option<String>) -> Option<String> {
    let p = path?;
    sanitize_user_path(&p).map(|pb| pb.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_ssh_dir() {
        assert!(sanitize_user_path("/Users/test/.ssh/id_rsa").is_none());
        assert!(sanitize_user_path("/home/x/.ssh/authorized_keys").is_none());
    }

    #[test]
    fn rejects_etc() {
        assert!(sanitize_user_path("/etc/passwd").is_none());
    }

    /// `/var` and `/etc` are symlinks on macOS; the denylist must match the
    /// canonicalized `/private/...` form, not just the pre-resolution path.
    #[test]
    fn rejects_symlinked_system_dirs() {
        assert!(sanitize_user_path("/var/root/anything").is_none());
        assert!(sanitize_user_path("/private/var/root/anything").is_none());
        assert!(sanitize_user_path("/private/etc/passwd").is_none());
    }

    #[test]
    fn rejects_null_and_empty() {
        assert!(sanitize_user_path("").is_none());
        assert!(sanitize_user_path("a\0b").is_none());
    }

    #[test]
    fn accepts_tmp_style() {
        let p = sanitize_user_path("/tmp/looks/photo.cube");
        assert!(p.is_some());
        assert!(!p.unwrap().to_string_lossy().contains(".."));
    }

    #[test]
    fn strips_windows_verbatim_prefix() {
        assert_eq!(
            simplify_path_str(r"\\?\C:\Users\test\photos"),
            r"C:\Users\test\photos"
        );
        assert_eq!(
            simplify_path_str(r"\\?\UNC\server\share\folder"),
            r"\\server\share\folder"
        );
        assert_eq!(simplify_path_str("/Users/mac/photos"), "/Users/mac/photos");
    }
}
