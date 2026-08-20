//! Offline license token storage + ES256 JWT verification (Rust-side).

use crate::error::AppError;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const LICENSE_PUBLIC_KEY_PEM: &str = include_str!("../license-public.pem");
const TOKEN_FILE: &str = "license.jwt";
const EMAIL_FILE: &str = "license.email";
/// Must match the issuer written by verify-license (Supabase Edge Function).
const LICENSE_ISS: &str = "https://meratech.co";
/// Audience bound to this desktop app.
const LICENSE_AUD: &str = "meraraw";

#[derive(Debug, Deserialize)]
struct LicenseClaims {
    sub: String,
    status: String,
    #[serde(default, rename = "earlySupporter")]
    early_supporter: bool,
    #[allow(dead_code)]
    exp: i64,
    #[allow(dead_code)]
    iat: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupporterStatusResult {
    pub licensed: bool,
    pub is_early_supporter: bool,
    pub user_id: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LicenseCheckResult {
    pub licensed: bool,
    pub user_id: Option<String>,
    pub email: Option<String>,
    pub reason: Option<String>,
}

fn token_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Internal(format!("app data dir: {e}")))?;
    Ok(dir.join(TOKEN_FILE))
}

fn email_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Internal(format!("app data dir: {e}")))?;
    Ok(dir.join(EMAIL_FILE))
}

fn read_saved_email(app: &AppHandle) -> Option<String> {
    let path = email_path(app).ok()?;
    let raw = std::fs::read_to_string(path).ok()?;
    let email = raw.trim().to_lowercase();
    if email.is_empty() || !email.contains('@') {
        None
    } else {
        Some(email)
    }
}

fn save_email(app: &AppHandle, email: &str) -> Result<(), AppError> {
    let path = email_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::Internal(format!("create app data dir: {e}")))?;
    }
    std::fs::write(&path, email.trim().to_lowercase())
        .map_err(|e| AppError::Internal(format!("write license email: {e}")))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub(crate) fn normalize_email(raw: &str) -> Result<String, AppError> {
    let email = raw.trim().to_lowercase();
    let (user, host) = email
        .split_once('@')
        .ok_or_else(|| AppError::Internal("Enter a valid email address.".into()))?;
    if user.is_empty()
        || host.is_empty()
        || !host.contains('.')
        || email.chars().any(char::is_whitespace)
        || email.len() > 254
    {
        return Err(AppError::Internal("Enter a valid email address.".into()));
    }
    Ok(email)
}

pub(crate) fn parse_otp(raw: &str) -> Result<String, AppError> {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() != 6 {
        return Err(AppError::Internal(
            "Enter the 6-digit code from your email.".into(),
        ));
    }
    Ok(digits)
}

fn verify_token(token: &str) -> Result<LicenseClaims, AppError> {
    let key = DecodingKey::from_ec_pem(LICENSE_PUBLIC_KEY_PEM.as_bytes())
        .map_err(|e| AppError::Internal(format!("license public key: {e}")))?;
    let mut validation = Validation::new(Algorithm::ES256);
    validation.validate_exp = true;
    // Default: require iss/aud (must match verify-license Edge Function).
    // MERARAW_LICENSE_RELAX_ISS_AUD=1 keeps older tokens working during migration;
    // debug-only, like MERARAW_SKIP_LICENSE, so a release build always binds
    // tokens to this issuer+audience.
    let relax = license_relax_iss_aud_enabled();
    if relax {
        validation.validate_aud = false;
    } else {
        validation.set_issuer(&[LICENSE_ISS]);
        validation.set_audience(&[LICENSE_AUD]);
    }
    let data = decode::<LicenseClaims>(token, &key, &validation)
        .map_err(|e| AppError::Internal(format!("invalid license token: {e}")))?;
    if data.claims.status != "active" {
        return Err(AppError::Internal("license not active".into()));
    }
    Ok(data.claims)
}

#[cfg(debug_assertions)]
fn license_skip_enabled() -> bool {
    std::env::var("MERARAW_SKIP_LICENSE").ok().as_deref() == Some("1")
}

#[cfg(not(debug_assertions))]
fn license_skip_enabled() -> bool {
    false
}

#[cfg(debug_assertions)]
fn license_relax_iss_aud_enabled() -> bool {
    std::env::var("MERARAW_LICENSE_RELAX_ISS_AUD")
        .ok()
        .as_deref()
        == Some("1")
}

#[cfg(not(debug_assertions))]
fn license_relax_iss_aud_enabled() -> bool {
    false
}

#[tauri::command]
pub fn license_check_local(app: AppHandle) -> LicenseCheckResult {
    if license_skip_enabled() {
        return LicenseCheckResult {
            licensed: true,
            user_id: Some("beta-skip".into()),
            email: None,
            reason: None,
        };
    }

    let path = match token_path(&app) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = %e, "license token path");
            return LicenseCheckResult {
                licensed: false,
                user_id: None,
                email: read_saved_email(&app),
                reason: Some("license unavailable".into()),
            };
        }
    };

    let token = match std::fs::read_to_string(&path) {
        Ok(t) => t.trim().to_string(),
        Err(_) => {
            return LicenseCheckResult {
                licensed: false,
                user_id: None,
                email: read_saved_email(&app),
                reason: Some("no saved license".into()),
            };
        }
    };

    match verify_token(&token) {
        Ok(claims) => LicenseCheckResult {
            licensed: true,
            user_id: Some(claims.sub),
            email: read_saved_email(&app),
            reason: None,
        },
        Err(e) => {
            tracing::warn!(error = %e, "license verify failed");
            LicenseCheckResult {
                licensed: false,
                user_id: None,
                email: read_saved_email(&app),
                reason: Some("invalid license".into()),
            }
        }
    }
}

/// Persist a verified license token. Not exposed over IPC — only called from
/// the native sign-in / activate path.
fn save_token(app: &AppHandle, token: &str) -> Result<(), AppError> {
    verify_token(token.trim())?;
    let path = token_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::Internal(format!("create app data dir: {e}")))?;
    }
    std::fs::write(&path, token.trim())
        .map_err(|e| AppError::Internal(format!("write license token: {e}")))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

#[tauri::command]
pub fn license_clear_token(app: AppHandle) -> Result<(), AppError> {
    let path = token_path(&app)?;
    if path.exists() {
        std::fs::remove_file(&path)
            .map_err(|e| AppError::Internal(format!("remove license token: {e}")))?;
    }
    if let Ok(email_file) = email_path(&app) {
        if email_file.exists() {
            let _ = std::fs::remove_file(&email_file);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn license_verify_token_locally(token: String) -> Result<bool, AppError> {
    verify_token(token.trim()).map(|_| true)
}

const DEFAULT_SUPABASE_URL: &str = "https://kaanlfnxoyrjrgqrxcuz.supabase.co";
const DEFAULT_SUPABASE_ANON_KEY: &str = "sb_publishable_7HICr8pQlJLALuYJzMrhjQ_mzaQt_4U";

fn supabase_host_allowed(host: &str) -> bool {
    host.eq_ignore_ascii_case("kaanlfnxoyrjrgqrxcuz.supabase.co")
}

fn supabase_url() -> Result<String, AppError> {
    let raw = std::env::var("VITE_SUPABASE_URL").unwrap_or_else(|_| DEFAULT_SUPABASE_URL.into());
    let parsed = url::Url::parse(&raw)
        .map_err(|_| AppError::Internal("Invalid VITE_SUPABASE_URL.".into()))?;
    if parsed.scheme() != "https" {
        return Err(AppError::Internal(
            "VITE_SUPABASE_URL must be https.".into(),
        ));
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| AppError::Internal("Invalid VITE_SUPABASE_URL host.".into()))?;
    if !supabase_host_allowed(host) {
        return Err(AppError::Internal(
            "VITE_SUPABASE_URL host not allowed.".into(),
        ));
    }
    // Normalize to origin (no credentials / fragment).
    Ok(format!("https://{host}"))
}

fn supabase_anon_key() -> String {
    std::env::var("VITE_SUPABASE_ANON_KEY").unwrap_or_else(|_| DEFAULT_SUPABASE_ANON_KEY.into())
}

async fn http_text(resp: reqwest::Response) -> Result<String, AppError> {
    let status = resp.status();
    let body = resp.text().await.map_err(|e| {
        tracing::warn!(error = %e, "license http read failed");
        AppError::Internal("Could not read server response.".into())
    })?;
    if status.is_success() {
        Ok(body)
    } else {
        // Never log full error bodies (may contain tokens / PII).
        tracing::warn!(%status, body_len = body.len(), "license http error");
        let hint = if body.contains("No active license") {
            "No beta license yet. Sign up and log in on meratech.co first, then try again."
        } else if body.contains("STRIPE_PRICE_ID") {
            "Stripe price not configured on server."
        } else if body.contains("LICENSE_SIGNING_PRIVATE_KEY") {
            "Server missing LICENSE_SIGNING_PRIVATE_KEY in Supabase secrets."
        } else if status.as_u16() == 404 {
            "Server endpoint not found. Deploy the required Supabase Edge Function."
        } else if status.as_u16() == 429 {
            "Wait a moment before requesting another code."
        } else if body.to_lowercase().contains("expired") || body.contains("otp_expired") {
            "That code expired. Request a new one."
        } else if body.to_lowercase().contains("invalid")
            && (body.to_lowercase().contains("token") || body.to_lowercase().contains("otp"))
        {
            "That code didn't match. Check the email and try again."
        } else if status.as_u16() == 401 || status.as_u16() == 403 {
            "Authentication failed."
        } else {
            "Server request failed."
        };
        Err(AppError::Internal(hint.into()))
    }
}

async fn activate_with_access_token(
    app: &AppHandle,
    access_token: &str,
    email: Option<&str>,
) -> Result<String, AppError> {
    let base = supabase_url()?;
    let anon = supabase_anon_key();
    let client = reqwest::Client::new();
    let auth_header = format!("Bearer {access_token}");

    let _ = client
        .post(format!("{base}/functions/v1/grant-beta-license"))
        .header("apikey", &anon)
        .header("Authorization", &auth_header)
        .send()
        .await;

    let verify_resp = client
        .post(format!("{base}/functions/v1/verify-license"))
        .header("apikey", &anon)
        .header("Authorization", &auth_header)
        .send()
        .await
        .map_err(|e| {
            AppError::Internal(format!(
                "Activation request failed ({e}). Deploy verify-license in Supabase."
            ))
        })?;

    let verify_body = http_text(verify_resp).await?;
    let verify: serde_json::Value = serde_json::from_str(&verify_body)
        .map_err(|e| AppError::Internal(format!("Activation response: {e}")))?;
    let token = verify["token"]
        .as_str()
        .ok_or_else(|| AppError::Internal("Activation did not return a license token.".into()))?;

    save_token(app, token)?;
    if let Some(email) = email {
        let _ = save_email(app, email);
    }
    Ok(verify["user_id"].as_str().unwrap_or("").to_string())
}

/// Email OTP via Supabase Auth. Delivery is the project's Auth SMTP (Resend).
#[tauri::command]
pub async fn license_request_otp(email: String) -> Result<(), AppError> {
    let email = normalize_email(&email)?;
    let base = supabase_url()?;
    let anon = supabase_anon_key();
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{base}/auth/v1/otp"))
        .header("apikey", &anon)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "email": email,
            "create_user": true,
        }))
        .send()
        .await
        .map_err(|e| {
            AppError::Internal(format!(
                "Could not reach the sign-in service ({e}). Check your internet connection."
            ))
        })?;

    http_text(resp).await.map(|_| ())
}

/// Confirm the 6-digit email code, grant beta, persist the offline license JWT.
#[tauri::command]
pub async fn license_verify_otp(
    app: AppHandle,
    email: String,
    code: String,
) -> Result<LicenseCheckResult, AppError> {
    let email = normalize_email(&email)?;
    let code = parse_otp(&code)?;
    let base = supabase_url()?;
    let anon = supabase_anon_key();
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{base}/auth/v1/verify"))
        .header("apikey", &anon)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "type": "email",
            "email": email,
            "token": code,
        }))
        .send()
        .await
        .map_err(|e| {
            AppError::Internal(format!(
                "Could not reach the sign-in service ({e}). Check your internet connection."
            ))
        })?;

    let body = http_text(resp).await?;
    let auth: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| AppError::Internal(format!("Sign-in response: {e}")))?;
    let access_token = auth["access_token"]
        .as_str()
        .ok_or_else(|| AppError::Internal("Sign-in did not return a session.".into()))?;
    let user_id = auth["user"]["id"].as_str().unwrap_or("").to_string();

    let _ = activate_with_access_token(&app, access_token, Some(&email)).await?;

    Ok(LicenseCheckResult {
        licensed: true,
        user_id: if user_id.is_empty() {
            None
        } else {
            Some(user_id)
        },
        email: Some(email),
        reason: None,
    })
}

/// Sign in + grant beta + verify license over native HTTPS (avoids WKWebView "Load failed").
#[tauri::command]
pub async fn license_sign_in_and_activate(
    app: AppHandle,
    email: String,
    password: String,
) -> Result<String, AppError> {
    let email = normalize_email(&email)?;
    if password.is_empty() {
        return Err(AppError::Internal("Enter email and password.".into()));
    }

    let base = supabase_url()?;
    let anon = supabase_anon_key();
    let client = reqwest::Client::new();

    let auth_resp = client
        .post(format!("{base}/auth/v1/token?grant_type=password"))
        .header("apikey", &anon)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "email": email, "password": password }))
        .send()
        .await
        .map_err(|e| {
            AppError::Internal(format!(
                "Could not reach Supabase ({e}). Check your internet connection."
            ))
        })?;

    let auth_body = http_text(auth_resp).await.map_err(|e| {
        if e.to_string().contains("invalid_grant") || e.to_string().contains("Invalid login") {
            AppError::Internal(
                "Invalid email or password. Use the same account as meratech.co.".into(),
            )
        } else if e.to_string().contains("email_not_confirmed") {
            AppError::Internal("Confirm your email from meratech.co, then try again.".into())
        } else {
            e
        }
    })?;

    let auth: serde_json::Value = serde_json::from_str(&auth_body)
        .map_err(|e| AppError::Internal(format!("Sign-in response: {e}")))?;
    let access_token = auth["access_token"]
        .as_str()
        .ok_or_else(|| AppError::Internal("Sign-in did not return a session.".into()))?;
    let user_id = auth["user"]["id"].as_str().unwrap_or("").to_string();

    let _ = activate_with_access_token(&app, access_token, Some(&email)).await?;
    Ok(user_id)
}

#[tauri::command]
pub fn license_supporter_status(app: AppHandle) -> SupporterStatusResult {
    if license_skip_enabled() {
        return SupporterStatusResult {
            licensed: true,
            is_early_supporter: false,
            user_id: Some("beta-skip".into()),
            reason: None,
        };
    }

    let path = match token_path(&app) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = %e, "license token path");
            return SupporterStatusResult {
                licensed: false,
                is_early_supporter: false,
                user_id: None,
                reason: Some("license unavailable".into()),
            };
        }
    };

    let token = match std::fs::read_to_string(&path) {
        Ok(t) => t.trim().to_string(),
        Err(_) => {
            return SupporterStatusResult {
                licensed: false,
                is_early_supporter: false,
                user_id: None,
                reason: Some("no saved license".into()),
            };
        }
    };

    match verify_token(&token) {
        Ok(claims) => SupporterStatusResult {
            licensed: true,
            is_early_supporter: claims.early_supporter,
            user_id: Some(claims.sub),
            reason: None,
        },
        Err(e) => {
            tracing::warn!(error = %e, "license verify failed");
            SupporterStatusResult {
                licensed: false,
                is_early_supporter: false,
                user_id: None,
                reason: Some("invalid license".into()),
            }
        }
    }
}

/// Sign in and create a Stripe checkout session for Early Supporter ($20 lifetime).
#[tauri::command]
pub async fn license_start_checkout(email: String, password: String) -> Result<String, AppError> {
    let email = email.trim();
    if email.is_empty() || password.is_empty() {
        return Err(AppError::Internal("Enter email and password.".into()));
    }

    let base = supabase_url()?;
    let anon = supabase_anon_key();
    let client = reqwest::Client::new();

    let auth_resp = client
        .post(format!("{base}/auth/v1/token?grant_type=password"))
        .header("apikey", &anon)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "email": email, "password": password }))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("Could not reach Supabase ({e}).")))?;

    let auth_body = http_text(auth_resp).await?;
    let auth: serde_json::Value = serde_json::from_str(&auth_body)
        .map_err(|e| AppError::Internal(format!("Sign-in response: {e}")))?;
    let access_token = auth["access_token"]
        .as_str()
        .ok_or_else(|| AppError::Internal("Sign-in did not return a session.".into()))?;

    let checkout_resp = client
        .post(format!("{base}/functions/v1/create-checkout-session"))
        .header("apikey", &anon)
        .header("Authorization", format!("Bearer {access_token}"))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "product": "early_supporter" }))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("Checkout request failed ({e}).")))?;

    let checkout_body = http_text(checkout_resp).await?;

    let checkout: serde_json::Value = serde_json::from_str(&checkout_body)
        .map_err(|e| AppError::Internal(format!("Checkout response: {e}")))?;
    checkout["url"]
        .as_str()
        .map(|u| u.to_string())
        .ok_or_else(|| AppError::Internal("Checkout did not return a URL.".into()))
}

#[tauri::command]
pub fn open_external_url(url: String) -> Result<(), AppError> {
    crate::open_url::open_https_url(&url)
}

#[cfg(test)]
mod tests {
    use super::{normalize_email, parse_otp};

    #[test]
    fn email_normalizes_and_rejects() {
        assert_eq!(
            normalize_email("  Kai@Meratech.CO ").unwrap(),
            "kai@meratech.co"
        );
        assert!(normalize_email("not-an-email").is_err());
        assert!(normalize_email("a@b").is_err());
        assert!(normalize_email("a @b.com").is_err());
    }

    #[test]
    fn otp_accepts_six_digits_and_strips_noise() {
        assert_eq!(parse_otp("123456").unwrap(), "123456");
        assert_eq!(parse_otp("123 456").unwrap(), "123456");
        assert!(parse_otp("12345").is_err());
        assert!(parse_otp("abcdef").is_err());
    }
}
