use crate::{model::now, service::Runtime};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct DeviceFlow {
    code: String,
    next_poll: u64,
    interval: u64,
    expires_at: u64,
}
#[derive(Default)]
pub struct AuthState(pub Mutex<Option<DeviceFlow>>);
fn client_id() -> Result<&'static str, String> {
    option_env!("CRICKET_GITHUB_CLIENT_ID").ok_or("This build has no GitHub OAuth client configured. Use your GitHub CLI sign-in or an access token.").map(str::trim).map_err(str::to_string)
}
fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn begin_github_login(state: tauri::State<'_, AuthState>) -> Result<Value, String> {
    let response: Value = client()?
        .post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .form(&[("client_id", client_id()?), ("scope", "repo")])
        .send()
        .await
        .map_err(|_| "Couldn't reach GitHub.")?
        .json()
        .await
        .map_err(|_| "Invalid GitHub sign-in response.")?;
    let code = response["device_code"]
        .as_str()
        .ok_or("GitHub device login isn't enabled for this OAuth app.")?
        .to_string();
    let user_code = response["user_code"]
        .as_str()
        .ok_or("Missing GitHub verification code.")?;
    let interval = response["interval"].as_u64().unwrap_or(5).max(5);
    let expires = response["expires_in"].as_u64().unwrap_or(900);
    *state.0.lock().await = Some(DeviceFlow {
        code,
        next_poll: now() + interval,
        interval,
        expires_at: now() + expires,
    });
    Ok(
        json!({"user_code":user_code,"verification_uri":"https://github.com/login/device","interval":interval}),
    )
}
#[tauri::command]
pub async fn poll_github_login(
    state: tauri::State<'_, AuthState>,
    runtime: tauri::State<'_, Arc<Runtime>>,
) -> Result<bool, String> {
    let mut flow = state.0.lock().await;
    let current = flow.as_mut().ok_or("Start GitHub sign-in first.")?;
    if now() >= current.expires_at {
        *flow = None;
        return Err("This sign-in code expired. Start again.".into());
    }
    if now() < current.next_poll {
        return Ok(false);
    }
    current.next_poll = now() + current.interval;
    let value: Value = client()?
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", client_id()?),
            ("device_code", &current.code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])
        .send()
        .await
        .map_err(|_| "Couldn't reach GitHub.")?
        .json()
        .await
        .map_err(|_| "Invalid GitHub sign-in response.")?;
    if let Some(token) = value["access_token"].as_str() {
        runtime.connect(token.to_string()).await?;
        *flow = None;
        return Ok(true);
    }
    match value["error"].as_str() {
        Some("authorization_pending") => Ok(false),
        Some("slow_down") => {
            current.interval += 5;
            current.next_poll = now() + current.interval;
            Ok(false)
        }
        _ => {
            *flow = None;
            Err("GitHub sign-in was declined or expired. Please try again.".into())
        }
    }
}
