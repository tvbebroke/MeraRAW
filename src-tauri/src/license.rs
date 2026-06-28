//! Offline license token storage + ES256 JWT verification (Rust-side).

use crate::error::AppError;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const LICENSE_PUBLIC_KEY_PEM: &str = include_str!("../license-public.pem");
const TOKEN_FILE: &str = "license.jwt";

#[derive(Debug, Deserialize)]
struct LicenseClaims {
    sub: String,
    status: String,
    #[allow(dead_code)]
    exp: i64,
    #[allow(dead_code)]
    iat: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LicenseCheckResult {
    pub licensed: bool,
    pub user_id: Option<String>,
    pub reason: Option<String>,
}

fn token_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Internal(format!("app data dir: {e}")))?;
    Ok(dir.join(TOKEN_FILE))
}

fn verify_token(token: &str) -> Result<LicenseClaims, AppError> {
    let key = DecodingKey::from_ec_pem(LICENSE_PUBLIC_KEY_PEM.as_bytes())
        .map_err(|e| AppError::Internal(format!("license public key: {e}")))?;
    let mut validation = Validation::new(Algorithm::ES256);
    validation.validate_exp = true;
    let data = decode::<LicenseClaims>(token, &key, &validation)
        .map_err(|e| AppError::Internal(format!("invalid license token: {e}")))?;
    if data.claims.status != "active" {
        return Err(AppError::Internal("license not active".into()));
    }
    Ok(data.claims)
}

#[tauri::command]
pub fn license_check_local(app: AppHandle) -> LicenseCheckResult {
    if std::env::var("MERARAW_SKIP_LICENSE").ok().as_deref() == Some("1") {
        return LicenseCheckResult {
            licensed: true,
            user_id: Some("beta-skip".into()),
            reason: None,
        };
    }

    let path = match token_path(&app) {
        Ok(p) => p,
        Err(e) => {
            return LicenseCheckResult {
                licensed: false,
                user_id: None,
                reason: Some(e.to_string()),
            };
        }
    };

    let token = match std::fs::read_to_string(&path) {
        Ok(t) => t.trim().to_string(),
        Err(_) => {
            return LicenseCheckResult {
                licensed: false,
                user_id: None,
                reason: Some("no saved license".into()),
            };
        }
    };

    match verify_token(&token) {
        Ok(claims) => LicenseCheckResult {
            licensed: true,
            user_id: Some(claims.sub),
            reason: None,
        },
        Err(e) => LicenseCheckResult {
            licensed: false,
            user_id: None,
            reason: Some(e.to_string()),
        },
    }
}

#[tauri::command]
pub fn license_save_token(app: AppHandle, token: String) -> Result<(), AppError> {
    verify_token(token.trim())?;
    let path = token_path(&app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::Internal(format!("create app data dir: {e}")))?;
    }
    std::fs::write(&path, token.trim())
        .map_err(|e| AppError::Internal(format!("write license token: {e}")))?;
    Ok(())
}

#[tauri::command]
pub fn license_clear_token(app: AppHandle) -> Result<(), AppError> {
    let path = token_path(&app)?;
    if path.exists() {
        std::fs::remove_file(&path)
            .map_err(|e| AppError::Internal(format!("remove license token: {e}")))?;
    }
    Ok(())
}

#[tauri::command]
pub fn license_verify_token_locally(token: String) -> Result<bool, AppError> {
    verify_token(token.trim()).map(|_| true)
}

const DEFAULT_SUPABASE_URL: &str = "https://kaanlfnxoyrjrgqrxcuz.supabase.co";
const DEFAULT_SUPABASE_ANON_KEY: &str = "sb_publishable_7HICr8pQlJLALuYJzMrhjQ_mzaQt_4U";

fn supabase_url() -> String {
    std::env::var("VITE_SUPABASE_URL").unwrap_or_else(|_| DEFAULT_SUPABASE_URL.into())
}

fn supabase_anon_key() -> String {
    std::env::var("VITE_SUPABASE_ANON_KEY").unwrap_or_else(|_| DEFAULT_SUPABASE_ANON_KEY.into())
}

async fn http_text(resp: reqwest::Response) -> Result<String, AppError> {
    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| AppError::Internal(format!("read response: {e}")))?;
    if status.is_success() {
        Ok(body)
    } else {
        Err(AppError::Internal(format!("HTTP {status}: {body}")))
    }
}

/// Sign in + grant beta + verify license over native HTTPS (avoids WKWebView "Load failed").
#[tauri::command]
pub async fn license_sign_in_and_activate(
    app: AppHandle,
    email: String,
    password: String,
) -> Result<String, AppError> {
    let email = email.trim();
    if email.is_empty() || password.is_empty() {
        return Err(AppError::Internal("Enter email and password.".into()));
    }

    let base = supabase_url();
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
    let user_id = auth["user"]["id"]
        .as_str()
        .unwrap_or("")
        .to_string();

    let auth_header = format!("Bearer {access_token}");

    // Best-effort beta row (same as website).
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

    let verify_body = http_text(verify_resp).await.map_err(|e| {
        if e.to_string().contains("No active license") {
            AppError::Internal(
                "No beta license yet. Sign up and log in on meratech.co first, then try again."
                    .into(),
            )
        } else if e.to_string().contains("LICENSE_SIGNING_PRIVATE_KEY") {
            AppError::Internal(
                "Server missing LICENSE_SIGNING_PRIVATE_KEY in Supabase secrets.".into(),
            )
        } else if e.to_string().contains("404") || e.to_string().contains("Not Found") {
            AppError::Internal(
                "verify-license function not deployed. Add it in Supabase Edge Functions.".into(),
            )
        } else {
            e
        }
    })?;

    let verify: serde_json::Value = serde_json::from_str(&verify_body)
        .map_err(|e| AppError::Internal(format!("Activation response: {e}")))?;
    let token = verify["token"]
        .as_str()
        .ok_or_else(|| AppError::Internal("Activation did not return a license token.".into()))?;

    license_save_token(app, token.to_string())?;
    Ok(user_id)
}
