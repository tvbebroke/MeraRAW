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
    exp: i64,
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
    #[cfg(debug_assertions)]
    if std::env::var("MERARAW_SKIP_LICENSE").ok().as_deref() == Some("1") {
        return LicenseCheckResult {
            licensed: true,
            user_id: Some("dev-skip".into()),
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
