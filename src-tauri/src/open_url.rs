//! Safe OS URL opening — never shells through `cmd /C`.

use crate::error::AppError;
use std::net::IpAddr;
use url::Url;

/// Hosts the app is allowed to open in the system browser (checkout / site).
fn host_allowed(host: &str) -> bool {
    let h = host.to_ascii_lowercase();
    h == "meratech.co"
        || h == "www.meratech.co"
        || h.ends_with(".meratech.co")
        || h.ends_with(".supabase.co")
        || h == "stripe.com"
        || h.ends_with(".stripe.com")
}

fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.octets()[0] == 0
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
        }
    }
}

/// Validate and open an HTTPS URL in the user's default browser.
pub fn open_https_url(raw: &str) -> Result<(), AppError> {
    let raw = raw.trim();
    let parsed = Url::parse(raw).map_err(|_| AppError::InvalidOp("Invalid URL.".into()))?;
    if parsed.scheme() != "https" {
        return Err(AppError::InvalidOp("Invalid URL (https only).".into()));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(AppError::InvalidOp("Invalid URL.".into()));
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| AppError::InvalidOp("Invalid URL.".into()))?;
    let host_l = host.to_ascii_lowercase();
    if host_l == "localhost" || host_l.ends_with(".localhost") || host_l.ends_with(".local") {
        return Err(AppError::InvalidOp("Invalid URL.".into()));
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_blocked_ip(ip) {
            return Err(AppError::InvalidOp("Invalid URL.".into()));
        }
    }
    if !host_allowed(host) {
        return Err(AppError::InvalidOp("URL host not allowed.".into()));
    }
    // Re-serialize so we open a normalized form (no cmd metachar injection surface).
    let safe = parsed.as_str().to_string();
    open::that(&safe).map_err(|e| {
        tracing::warn!(error = %e, "open https url failed");
        AppError::Internal("Could not open browser.".into())
    })?;
    Ok(())
}

/// Open a mailto: URL (problem reports). Caps length; no shell.
pub fn open_mailto_url(raw: &str) -> Result<(), AppError> {
    let raw = raw.trim();
    if raw.len() > 32_768 {
        return Err(AppError::InvalidOp("Message too long.".into()));
    }
    let parsed = Url::parse(raw).map_err(|_| AppError::InvalidOp("Invalid mailto URL.".into()))?;
    if parsed.scheme() != "mailto" {
        return Err(AppError::InvalidOp("Invalid mailto URL.".into()));
    }
    open::that(parsed.as_str()).map_err(|e| {
        tracing::warn!(error = %e, "open mailto failed");
        AppError::Internal("Could not open mail client.".into())
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_http_and_loopback() {
        assert!(open_https_url("http://www.meratech.co").is_err());
        assert!(open_https_url("https://127.0.0.1/").is_err());
        assert!(open_https_url("https://localhost/").is_err());
        assert!(open_https_url("https://192.168.1.1/").is_err());
        assert!(open_https_url("https://user:pass@www.meratech.co/").is_err());
    }

    #[test]
    fn allows_known_hosts() {
        // Don't actually open — just validate the allow path by checking Err kind isn't "not allowed"
        // We call the validation pieces via rejected unknown host.
        assert!(open_https_url("https://evil.example/").is_err());
        assert!(host_allowed("www.meratech.co"));
        assert!(host_allowed("checkout.stripe.com"));
        assert!(host_allowed("kaanlfnxoyrjrgqrxcuz.supabase.co"));
        assert!(!host_allowed("evil.example"));
    }
}
