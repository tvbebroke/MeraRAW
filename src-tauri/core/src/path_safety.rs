//! Path denylist for sidecar-sourced paths (LUT, etc.) that never pass
//! through the Tauri `validate_user_path` IPC gate.

use std::path::{Component, Path, PathBuf};

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
    Some(resolved)
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
}
