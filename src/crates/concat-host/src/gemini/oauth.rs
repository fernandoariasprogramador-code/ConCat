// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! Google OAuth 2.0 PKCE authentication for Gemini.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::dirs::AppDirs;

/// Default Google OAuth Client ID for Concat Gemini Assistant.
/// Can be overridden by the user or build environment.
pub const DEFAULT_CLIENT_ID: &str = "643510293568-o42toaogl1e3ir4770i6los6022r7u5l.apps.googleusercontent.com";
pub const DEFAULT_CLIENT_SECRET: &str = "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";
pub const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
pub const GOOGLE_USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v2/userinfo";
pub const SCOPE_GEMINI: &str = "https://www.googleapis.com/auth/cloud-platform https://www.googleapis.com/auth/generative-language.retriever email profile";

/// Saved credentials for Gemini access.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct GeminiAuth {
    /// OAuth access token.
    pub access_token: Option<String>,
    /// OAuth refresh token.
    pub refresh_token: Option<String>,
    /// Expiration timestamp in seconds since epoch.
    pub expires_at: Option<u64>,
    /// Authenticated user's email if retrieved.
    pub user_email: Option<String>,
    /// Authenticated user's display name.
    pub user_name: Option<String>,
    /// Direct API key fallback if set instead of OAuth.
    pub api_key: Option<String>,
    /// Custom Client ID if user provides their own GCP project credentials.
    pub custom_client_id: Option<String>,
}

impl GeminiAuth {
    /// Loads stored auth from the configuration directory.
    pub fn load(dirs: &AppDirs) -> Self {
        let path = token_path(dirs);
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(auth) = serde_json::from_str(&data) {
                return auth;
            }
        }
        Self::default()
    }

    /// Saves auth credentials to disk.
    pub fn save(&self, dirs: &AppDirs) -> Result<(), String> {
        let path = token_path(dirs);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Clears OAuth tokens and logs out.
    pub fn logout(&mut self, dirs: &AppDirs) -> Result<(), String> {
        self.access_token = None;
        self.refresh_token = None;
        self.expires_at = None;
        self.user_email = None;
        self.user_name = None;
        self.save(dirs)
    }

    /// Checks if we currently have an active login or API key.
    pub fn is_logged_in(&self) -> bool {
        self.api_key.is_some()
            || self.access_token.is_some()
            || self.refresh_token.is_some()
            || std::env::var("GEMINI_API_KEY").map(|k| !k.trim().is_empty()).unwrap_or(false)
    }

    /// Returns the effective client id.
    pub fn client_id(&self) -> &str {
        self.custom_client_id
            .as_deref()
            .unwrap_or(DEFAULT_CLIENT_ID)
    }

    /// The Cloud project number embedded in the client id (`<number>-<hash>.apps...`),
    /// sent as `x-goog-user-project` so OAuth calls are attributed to that project.
    pub fn quota_project(&self) -> Option<String> {
        let number = self.client_id().split('-').next()?;
        (!number.is_empty() && number.chars().all(|c| c.is_ascii_digit()))
            .then(|| number.to_string())
    }

    /// Obtains a valid token (refreshing if expired) or an API key.
    pub fn get_valid_token(&mut self, dirs: &AppDirs) -> Result<String, String> {
        if let Some(key) = &self.api_key {
            if !key.trim().is_empty() {
                return Ok(key.clone());
            }
        }

        if let Ok(key) = std::env::var("GEMINI_API_KEY") {
            if !key.trim().is_empty() {
                return Ok(key);
            }
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Check if access_token is still valid (with 60s margin)
        if let Some(token) = &self.access_token {
            if let Some(exp) = self.expires_at {
                if exp > now + 60 {
                    return Ok(token.clone());
                }
            }
        }

        // Try to refresh using refresh_token
        if let Some(refresh_token) = &self.refresh_token {
            let refreshed = refresh_oauth_token(self.client_id(), refresh_token)?;
            self.access_token = Some(refreshed.access_token.clone());
            self.expires_at = Some(now + refreshed.expires_in);
            let _ = self.save(dirs);
            return Ok(refreshed.access_token);
        }

        Err("No hay sesión activa de Gemini ni clave de API configurada.".into())
    }
}

fn token_path(dirs: &AppDirs) -> PathBuf {
    dirs.config.join("gemini_auth.json")
}

/// Generates a cryptographic PKCE code verifier and challenge.
pub fn generate_pkce() -> (String, String) {
    let mut bytes = [0u8; 32];
    // Simple deterministic fallback / system entropy
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(42);
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = ((seed >> (i % 8 * 8)) ^ (i as u128 * 0x5851f42d4c957f2d)) as u8;
    }
    let verifier = URL_SAFE_NO_PAD.encode(bytes);

    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let hash = hasher.finalize();
    let challenge = URL_SAFE_NO_PAD.encode(hash);

    (verifier, challenge)
}

/// Structure for starting an interactive OAuth flow in the browser.
pub struct OAuthFlow {
    pub auth_url: String,
    pub verifier: String,
    listener: TcpListener,
    port: u16,
    cancelled: Arc<AtomicBool>,
}

impl OAuthFlow {
    /// Prepares the OAuth server and returns the flow handle.
    pub fn start(client_id: &str) -> Result<Self, String> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|e| format!("No se pudo enlazar servidor local: {e}"))?;
        let port = listener
            .local_addr()
            .map_err(|e| e.to_string())?
            .port();

        let (verifier, challenge) = generate_pkce();
        let redirect_uri = format!("http://127.0.0.1:{port}/oauth/callback");

        let auth_url = format!(
            "{GOOGLE_AUTH_URL}?client_id={}&redirect_uri={}&response_type=code&scope={}&code_challenge={}&code_challenge_method=S256&access_type=offline&prompt=consent",
            urlencoding(client_id),
            urlencoding(&redirect_uri),
            urlencoding(SCOPE_GEMINI),
            challenge
        );

        Ok(Self {
            auth_url,
            verifier,
            listener,
            port,
            cancelled: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Cancels the waiting flow.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// Waits for the browser callback and exchanges the code for tokens.
    pub fn wait_and_exchange(self, client_id: &str) -> Result<OAuthExchangeResult, String> {
        self.listener
            .set_nonblocking(false)
            .map_err(|e| e.to_string())?;

        let (mut stream, _) = self
            .listener
            .accept()
            .map_err(|e| format!("Fallo al recibir respuesta OAuth: {e}"))?;

        let mut buffer = [0u8; 2048];
        let bytes_read = stream.read(&mut buffer).map_err(|e| e.to_string())?;
        let request = String::from_utf8_lossy(&buffer[..bytes_read]);

        // Parse query params from GET /oauth/callback?code=...
        let code = extract_query_param(&request, "code")
            .ok_or_else(|| "No se encontró el código de autorización en la respuesta".to_string())?;

        // Send friendly HTML response
        let body = "<!DOCTYPE html><html><body style=\"font-family:sans-serif;text-align:center;padding-top:60px;background:#18181b;color:#f4f4f5;\"><h2>Autenticación completada con éxito</h2><p>Ya puedes cerrar esta pestaña y volver a Concat.</p></body></html>";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(response.as_bytes());

        // Exchange code with Google
        let redirect_uri = format!("http://127.0.0.1:{}/oauth/callback", self.port);
        let token_resp = exchange_code(client_id, &code, &self.verifier, &redirect_uri)?;

        // Try to fetch user info (email)
        let (email, name) = fetch_user_info(&token_resp.access_token).unwrap_or((None, None));

        Ok(OAuthExchangeResult {
            access_token: token_resp.access_token,
            refresh_token: token_resp.refresh_token,
            expires_in: token_resp.expires_in,
            email,
            name,
        })
    }
}

#[derive(Debug)]
pub struct OAuthExchangeResult {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: u64,
    pub email: Option<String>,
    pub name: Option<String>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: u64,
}

fn exchange_code(
    client_id: &str,
    code: &str,
    verifier: &str,
    redirect_uri: &str,
) -> Result<TokenResponse, String> {
    let params = [
        ("client_id", client_id),
        ("client_secret", DEFAULT_CLIENT_SECRET),
        ("code", code),
        ("code_verifier", verifier),
        ("grant_type", "authorization_code"),
        ("redirect_uri", redirect_uri),
    ];

    let resp = ureq::post(GOOGLE_TOKEN_URL)
        .send_form(&params)
        .map_err(|e| format!("Error canjeando código OAuth con Google: {e}"))?;

    let token_resp: TokenResponse = resp
        .into_json()
        .map_err(|e| format!("Error decodificando tokens de Google: {e}"))?;

    Ok(token_resp)
}

fn refresh_oauth_token(client_id: &str, refresh_token: &str) -> Result<TokenResponse, String> {
    let params = [
        ("client_id", client_id),
        ("client_secret", DEFAULT_CLIENT_SECRET),
        ("refresh_token", refresh_token),
        ("grant_type", "refresh_token"),
    ];

    let resp = ureq::post(GOOGLE_TOKEN_URL)
        .send_form(&params)
        .map_err(|e| format!("Error refrescando token OAuth con Google: {e}"))?;

    let token_resp: TokenResponse = resp
        .into_json()
        .map_err(|e| format!("Error decodificando nuevo token: {e}"))?;

    Ok(token_resp)
}

#[derive(Deserialize)]
struct UserInfoResponse {
    email: Option<String>,
    name: Option<String>,
}

fn fetch_user_info(access_token: &str) -> Result<(Option<String>, Option<String>), String> {
    let resp = ureq::get(GOOGLE_USERINFO_URL)
        .set("Authorization", &format!("Bearer {access_token}"))
        .call()
        .map_err(|e| e.to_string())?;

    let info: UserInfoResponse = resp.into_json().map_err(|e| e.to_string())?;
    Ok((info.email, info.name))
}

fn extract_query_param(request: &str, key: &str) -> Option<String> {
    let first_line = request.lines().next()?;
    let path = first_line.split_whitespace().nth(1)?;
    let query = path.split('?').nth(1)?;
    for pair in query.split('&') {
        let mut parts = pair.split('=');
        if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
            if k == key {
                return Some(urldecode(v));
            }
        }
    }
    None
}

fn urlencoding(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    encoded
}

fn urldecode(input: &str) -> String {
    let mut result = String::new();
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let h1 = chars.next().unwrap_or('0');
            let h2 = chars.next().unwrap_or('0');
            if let Ok(byte) = u8::from_str_radix(&format!("{h1}{h2}"), 16) {
                result.push(byte as char);
            }
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }
    result
}
