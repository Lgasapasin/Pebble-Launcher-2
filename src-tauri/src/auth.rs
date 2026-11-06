use crate::models::Account;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;
use tokio::sync::Mutex;

const AUTH_BASE: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0";
const KEYRING_SERVICE: &str = "io.pebble.launcher";

pub struct AuthState {
    pub pending: Mutex<Option<PendingAuth>>,
    pub session: Mutex<Option<Session>>,
}
impl Default for AuthState {
    fn default() -> Self {
        Self { pending: Mutex::new(None), session: Mutex::new(None) }
    }
}

pub struct PendingAuth {
    client_id: String,
    device_code: String,
    interval: u64,
    expires: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub profile: Account,
    client_id: String,
    microsoft_refresh_token: String,
    minecraft_access_token: String,
    minecraft_expires_at: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DevicePrompt {
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub message: String,
}

fn epoch() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}
fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, "minecraft-account").map_err(|e| e.to_string())
}
fn save_session(session: &Session) -> Result<(), String> {
    let contents = serde_json::to_string(session).map_err(|e| e.to_string())?;
    entry()?.set_password(&contents).map_err(|e| e.to_string())
}
fn load_session() -> Option<Session> {
    entry().ok()?.get_password().ok().and_then(|s| serde_json::from_str(&s).ok())
}

pub async fn get_session(state: &State<'_, AuthState>) -> Option<Session> {
    if let Some(session) = state.session.lock().await.clone() { return Some(session); }
    let saved = load_session()?;
    *state.session.lock().await = Some(saved.clone());
    Some(saved)
}

#[tauri::command]
pub async fn account_info(state: State<'_, AuthState>) -> Result<Option<Account>, String> {
    Ok(get_session(&state).await.map(|s| s.profile))
}

#[tauri::command]
pub async fn logout(state: State<'_, AuthState>) -> Result<(), String> {
    *state.session.lock().await = None;
    *state.pending.lock().await = None;
    if let Ok(e) = entry() {
        match e.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {},
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn start_login(client_id: String, state: State<'_, AuthState>) -> Result<DevicePrompt, String> {
    if client_id.len() > 100 || client_id.trim().is_empty() || !client_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err("Enter your Azure public app's Application (client) ID".into());
    }
    let client = crate::net::client()?;
    let response = client.post(format!("{AUTH_BASE}/devicecode"))
        .form(&[("client_id", client_id.as_str()), ("scope", "XboxLive.signin offline_access")])
        .send().await.map_err(|e| e.to_string())?
        .error_for_status().map_err(|e| format!("Microsoft device login: {e}"))?
        .json::<serde_json::Value>().await.map_err(|e| e.to_string())?;
    let string = |k: &str| response[k].as_str().unwrap_or_default().to_owned();
    let device_code = string("device_code");
    let prompt = DevicePrompt {
        user_code: string("user_code"),
        verification_uri: string("verification_uri"),
        expires_in: response["expires_in"].as_u64().unwrap_or(900),
        message: string("message"),
    };
    if device_code.is_empty() || prompt.user_code.is_empty() { return Err("Unexpected Microsoft device login response".into()); }
    *state.pending.lock().await = Some(PendingAuth {
        client_id, device_code, interval: response["interval"].as_u64().unwrap_or(5).max(2),
        expires: epoch() + prompt.expires_in,
    });
    Ok(prompt)
}

#[tauri::command]
pub async fn complete_login(state: State<'_, AuthState>) -> Result<Account, String> {
    let pending = state.pending.lock().await.take().ok_or("Start Microsoft sign-in first")?;
    let client = crate::net::client()?;
    let mut interval = pending.interval;
    let microsoft_tokens = loop {
        if epoch() >= pending.expires { return Err("Device login timed out; please start again".into()); }
        tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
        let r: serde_json::Value = client.post(format!("{AUTH_BASE}/token"))
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", &pending.client_id),
                ("device_code", &pending.device_code),
            ])
            .send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
        if r.get("access_token").is_some() { break r; }
        match r["error"].as_str().unwrap_or("unknown_error") {
            "authorization_pending" => continue,
            "slow_down" => { interval += 5; continue; },
            "authorization_declined" | "access_denied" => return Err("Microsoft sign-in was declined".into()),
            "expired_token" => return Err("Device login expired".into()),
            other => return Err(format!("Microsoft authentication failed: {other}")),
        }
    };
    let access_token = microsoft_tokens["access_token"].as_str().ok_or("Missing Microsoft token")?;
    let refresh_token = microsoft_tokens["refresh_token"].as_str().ok_or("Missing Microsoft refresh token")?;
    let (profile, minecraft_access_token, expires_in) = minecraft_tokens(&client, access_token).await?;
    let session = Session {
        profile: profile.clone(), client_id: pending.client_id,
        microsoft_refresh_token: refresh_token.to_owned(),
        minecraft_access_token,
        minecraft_expires_at: epoch() + expires_in.saturating_sub(120),
    };
    *state.session.lock().await = Some(session.clone());
    // On systems without an OS keyring, login remains usable for this running session.
    // We intentionally never fall back to unencrypted disk token storage.
    let _ = save_session(&session);
    Ok(profile)
}

pub async fn access_for_launch(state: &State<'_, AuthState>) -> Result<(Account, String), String> {
    let mut session = get_session(state).await.ok_or("Sign into your Microsoft account first")?;
    if epoch() >= session.minecraft_expires_at {
        let client = crate::net::client()?;
        let r: serde_json::Value = client.post(format!("{AUTH_BASE}/token"))
            .form(&[
                ("grant_type", "refresh_token"),
                ("client_id", &session.client_id),
                ("refresh_token", &session.microsoft_refresh_token),
                ("scope", "XboxLive.signin offline_access"),
            ])
            .send().await.map_err(|e| e.to_string())?
            .error_for_status().map_err(|_| "Microsoft session expired; sign in again".to_string())?
            .json().await.map_err(|e| e.to_string())?;
        let ms_access = r["access_token"].as_str().ok_or("Microsoft token refresh failed")?;
        let (profile, mc_access, lifetime) = minecraft_tokens(&client, ms_access).await?;
        session.profile = profile;
        session.minecraft_access_token = mc_access;
        session.minecraft_expires_at = epoch() + lifetime.saturating_sub(120);
        if let Some(rt) = r["refresh_token"].as_str() { session.microsoft_refresh_token = rt.to_owned(); }
        *state.session.lock().await = Some(session.clone());
        let _ = save_session(&session);
    }
    Ok((session.profile, session.minecraft_access_token))
}

async fn minecraft_tokens(client: &reqwest::Client, ms_access: &str) -> Result<(Account, String, u64), String> {
    let xbox: serde_json::Value = client.post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&serde_json::json!({
            "Properties": {"AuthMethod":"RPS", "SiteName":"user.auth.xboxlive.com", "RpsTicket":format!("d={ms_access}")},
            "RelyingParty":"http://auth.xboxlive.com", "TokenType":"JWT"
        })).send().await.map_err(|e| e.to_string())?
        .error_for_status().map_err(|_| "Xbox authentication failed; verify Microsoft permissions".to_string())?
        .json().await.map_err(|e| e.to_string())?;
    let xbox_token = xbox["Token"].as_str().ok_or("Xbox auth token missing")?;
    let xsts: serde_json::Value = client.post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&serde_json::json!({
            "Properties": {"SandboxId":"RETAIL", "UserTokens":[xbox_token]},
            "RelyingParty":"rp://api.minecraftservices.com/", "TokenType":"JWT"
        })).send().await.map_err(|e| e.to_string())?
        .error_for_status().map_err(|_| "Xbox XSTS authorization failed (Xbox account or permissions)".to_string())?
        .json().await.map_err(|e| e.to_string())?;
    let xsts_token = xsts["Token"].as_str().ok_or("XSTS token missing")?;
    let uhs = xsts["DisplayClaims"]["xui"][0]["uhs"].as_str().ok_or("Xbox user hash missing")?;
    let minecraft: serde_json::Value = client.post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&serde_json::json!({"identityToken":format!("XBL3.0 x={uhs};{xsts_token}")}))
        .send().await.map_err(|e| e.to_string())?
        .error_for_status().map_err(|_| "Minecraft API access denied; your Azure app might require Minecraft API approval".to_string())?
        .json().await.map_err(|e| e.to_string())?;
    let token = minecraft["access_token"].as_str().ok_or("Minecraft token missing")?;
    let lifetime = minecraft["expires_in"].as_u64().unwrap_or(86_400);
    let response = client.get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(token).send().await.map_err(|e| e.to_string())?
        .error_for_status().map_err(|_| "No Minecraft Java Edition profile is available for this account".to_string())?;
    let profile: Account = response.json().await.map_err(|e| e.to_string())?;
    Ok((profile, token.to_owned(), lifetime))
}
