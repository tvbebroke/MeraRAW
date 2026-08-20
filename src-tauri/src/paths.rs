//! Path validation for IPC commands that accept filesystem paths.
//! Canonicalize when possible, reject traversal tricks and sensitive locations.

use crate::error::AppError;
use std::io;
use std::path::{Component, Path, PathBuf};

/// True when macOS TCC / iCloud / ACL denied the open (EPERM / EACCES).
pub fn is_permission_denied(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::PermissionDenied
        || matches!(err.raw_os_error(), Some(1) | Some(13))
}

/// Human-readable guidance when the OS blocks reading a photo path.
pub fn permission_denied_message(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    format!(
        "macOS blocked reading “{name}”. \
         Open it with Browse… / the file picker (that grants access), \
         or System Settings → Privacy & Security → Files and Folders \
         (or Full Disk Access) → allow MeraRAW. \
         If the file lives in iCloud/Photos, download it first in Finder."
    )
}

/// Ensure we can open the file for read before handing it to the decoder.
/// Surfaces TCC / cloud-placeholder failures as a clear Decode error.
pub fn ensure_readable(path: &Path) -> Result<(), AppError> {
    match std::fs::File::open(path) {
        Ok(_) => Ok(()),
        Err(e) if is_permission_denied(&e) => {
            Err(AppError::Decode(permission_denied_message(path)))
        }
        Err(e) => {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "file".into());
            Err(AppError::Io(format!("cannot open {name}: {e}")))
        }
    }
}

/// Sensitive prefixes we never open/list via IPC (defense in depth).
fn is_denied(path: &Path) -> bool {
    let s = path.to_string_lossy();
    let lower = s.to_lowercase();

    // Home-relative secrets (any OS)
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

    // System dirs
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
        let deny = [
            "\\windows\\system32",
            "\\windows\\syswow64",
            "\\$recycle.bin",
        ];
        if deny.iter().any(|d| lower.contains(d)) {
            return true;
        }
    }

    false
}

/// Normalize without requiring the path to exist (resolve `.` / `..`).
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

/// Validate a user-supplied path from the webview.
/// - Rejects empty / null-byte paths
/// - Lexically normalizes `..`
/// - Canonicalizes when the path (or a parent) exists
/// - Rejects known-sensitive locations
pub fn validate_user_path(path: &str) -> Result<PathBuf, AppError> {
    let path = path.trim();
    if path.is_empty() {
        return Err(AppError::InvalidOp("empty path".into()));
    }
    if path.contains('\0') {
        return Err(AppError::InvalidOp("invalid path".into()));
    }

    let raw = PathBuf::from(path);
    let normalized = normalize_lexical(&raw);

    let resolved = if normalized.exists() {
        normalized
            .canonicalize()
            .map_err(|e| AppError::Io(format!("resolve path: {e}")))?
    } else if let Some(parent) = normalized.parent().filter(|p| !p.as_os_str().is_empty()) {
        if parent.exists() {
            let parent = parent
                .canonicalize()
                .map_err(|e| AppError::Io(format!("resolve parent: {e}")))?;
            let name = normalized
                .file_name()
                .ok_or_else(|| AppError::InvalidOp("invalid path".into()))?;
            parent.join(name)
        } else {
            normalized
        }
    } else {
        normalized
    };

    if is_denied(&resolved) {
        return Err(AppError::InvalidOp("path not allowed".into()));
    }

    // Windows canonicalize() yields `\\?\C:\…`; the picker returns `C:\…`.
    // Strip so catalog keys match what the UI queries with.
    Ok(meratech_core::path_safety::simplify_path(resolved))
}

/// Same as [`validate_user_path`], but the path must already exist.
pub fn validate_existing_path(path: &str) -> Result<PathBuf, AppError> {
    let p = validate_user_path(path)?;
    if !p.exists() {
        return Err(AppError::NotFound(path.to_string()));
    }
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty() {
        assert!(validate_user_path("").is_err());
        assert!(validate_user_path("   ").is_err());
    }

    #[test]
    fn rejects_null() {
        assert!(validate_user_path("foo\0bar").is_err());
    }

    #[test]
    fn normalizes_dotdot() {
        let p = validate_user_path("/tmp/a/../b").unwrap();
        assert!(!p.to_string_lossy().contains(".."));
    }

    #[test]
    fn rejects_ssh_denylist() {
        assert!(validate_user_path("/Users/test/.ssh/id_rsa").is_err());
        assert!(validate_user_path("/home/x/.ssh/config").is_err());
        assert!(validate_user_path("/Users/test/.gnupg/secring.gpg").is_err());
        assert!(validate_user_path("/etc/passwd").is_err());
    }

    /// `/var` and `/etc` are symlinks on macOS; the denylist must match the
    /// canonicalized `/private/...` form, not just the pre-resolution path.
    #[test]
    fn rejects_symlinked_system_dirs() {
        assert!(validate_user_path("/var/root/anything").is_err());
        assert!(validate_user_path("/private/var/root/anything").is_err());
        assert!(validate_user_path("/private/etc/passwd").is_err());
    }
}
