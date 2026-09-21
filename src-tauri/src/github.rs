use crate::model::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::{Client, Method, StatusCode};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct GitHub {
    client: Client,
    token: String,
    cache: Arc<Mutex<HashMap<String, (String, Value)>>>,
    transfers: Arc<Mutex<HashMap<String, (String, Transfer)>>>,
}
#[derive(Deserialize)]
pub struct Repo {
    pub full_name: String,
    pub private: bool,
    pub description: Option<String>,
}
#[derive(Clone, serde::Serialize, Deserialize)]
pub struct Invitation {
    pub id: u64,
    pub repository: InvitationRepo,
}
#[derive(Clone, serde::Serialize, Deserialize)]
pub struct InvitationRepo {
    pub full_name: String,
    pub description: Option<String>,
}
#[derive(Deserialize)]
struct Author {
    login: String,
}
#[derive(Deserialize)]
struct Issue {
    number: u64,
    body: Option<String>,
    user: Author,
    pull_request: Option<Value>,
    updated_at: String,
    comments: u64,
}
#[derive(Deserialize)]
struct Comment {
    body: String,
    user: Author,
}

impl GitHub {
    pub fn new(token: String) -> Result<Self, String> {
        let client = Client::builder()
            .user_agent("Cricket/0.1.0 (https://github.com/eddieyzhan/Cricket)")
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            client,
            token,
            cache: Arc::new(Mutex::new(HashMap::new())),
            transfers: Arc::new(Mutex::new(HashMap::new())),
        })
    }
    pub async fn request(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<Value, String> {
        let cached = if method == Method::GET {
            self.cache.lock().await.get(path).cloned()
        } else {
            None
        };
        let mut request = self
            .client
            .request(method.clone(), format!("https://api.github.com{path}"))
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");
        if let Some((etag, _)) = &cached {
            request = request.header("If-None-Match", etag);
        }
        if let Some(value) = body {
            request = request.json(&value);
        }
        let response = request.send().await.map_err(|_| {
            "Can't reach GitHub. Check your connection; your local history is safe.".to_string()
        })?;
        if response.status() == StatusCode::NOT_MODIFIED {
            return Ok(cached.ok_or("Missing GitHub cache entry")?.1);
        }
        let status = response.status();
        if !status.is_success() {
            return Err(match status.as_u16() {
                401 => "GitHub sign-in has expired. Reconnect your account.",
                403 | 429 => "GitHub denied this request or is rate limiting it. Check repository access, then try again later.",
                404 => "This repository or invitation isn't available to your account.",
                422 => "GitHub could not accept this request. Check the name and GitHub usernames.",
                _ => "GitHub couldn't complete the request. Please try again later.",
            }.to_string());
        }
        if status == StatusCode::NO_CONTENT {
            return Ok(Value::Null);
        }
        let etag = response
            .headers()
            .get("etag")
            .and_then(|h| h.to_str().ok())
            .map(str::to_string);
        let value = response
            .json::<Value>()
            .await
            .map_err(|_| "GitHub returned an unreadable response.".to_string())?;
        if method == Method::GET {
            if let Some(etag) = etag {
                self.cache
                    .lock()
                    .await
                    .insert(path.into(), (etag, value.clone()));
            }
        } else {
            self.cache.lock().await.clear();
        }
        Ok(value)
    }
    pub async fn get(&self, path: &str) -> Result<Value, String> {
        self.request(Method::GET, path, None).await
    }
    async fn pages(&self, path: &str) -> Result<Vec<Value>, String> {
        let mut values = Vec::new();
        let separator = if path.contains('?') { '&' } else { '?' };
        for page in 1..=20 {
            let value = self
                .get(&format!("{path}{separator}per_page=100&page={page}"))
                .await?;
            let items = value.as_array().ok_or("Invalid GitHub list response")?;
            values.extend(items.iter().cloned());
            if items.len() < 100 {
                return Ok(values);
            }
        }
        Err("This GitHub collection exceeds Cricket's current 2,000-item limit.".into())
    }
    pub async fn user(&self) -> Result<User, String> {
        serde_json::from_value(self.get("/user").await?).map_err(|e| e.to_string())
    }
    pub async fn private_repo(&self, repo: &str) -> Result<Repo, String> {
        if !valid_repo(repo) {
            return Err("Use a repository in owner/name format.".into());
        }
        let result: Repo = serde_json::from_value(self.get(&format!("/repos/{repo}")).await?)
            .map_err(|e| e.to_string())?;
        if !result.private {
            return Err("Cricket only shares transfer information in private repositories. Make this chat private again before continuing.".into());
        }
        Ok(result)
    }
    pub async fn chat(&self, repo: &str, login: &str) -> Result<Chat, String> {
        self.private_repo(repo).await?;
        let file = self
            .get(&format!("/repos/{repo}/contents/.cricket/chat.json"))
            .await?;
        let encoded = file["content"]
            .as_str()
            .ok_or("Missing Cricket chat manifest")?
            .replace('\n', "");
        let bytes = STANDARD
            .decode(encoded)
            .map_err(|_| "Invalid chat manifest encoding")?;
        if bytes.len() > 16_384 {
            return Err("Chat manifest is too large.".into());
        }
        let mut chat: Chat =
            serde_json::from_slice(&bytes).map_err(|_| "Invalid Cricket chat manifest")?;
        chat.repo = repo.to_string();
        chat.validate(login)?;
        Ok(chat)
    }
    pub async fn discover(&self, login: &str) -> Result<Vec<Chat>, String> {
        let repos = self
            .pages("/user/repos?affiliation=owner,collaborator,organization_member&sort=updated")
            .await?;
        let mut chats = Vec::new();
        for value in repos {
            let repo: Repo = serde_json::from_value(value).map_err(|e| e.to_string())?;
            if repo.private
                && repo
                    .description
                    .as_deref()
                    .is_some_and(|d| d.starts_with(CHAT_MARKER))
            {
                chats.push(self.chat(&repo.full_name, login).await?);
            }
        }
        Ok(chats)
    }
    pub async fn create_chat(
        &self,
        login: &str,
        name: String,
        members: Vec<String>,
    ) -> Result<(Chat, Vec<String>), String> {
        let repo_name = format!("cricket-{}", &id()[..12]);
        let chat = Chat {
            repo: format!("{login}/{repo_name}"),
            name,
            members,
        };
        chat.validate(login)?;
        for member in &chat.members {
            self.get(&format!("/users/{member}")).await?;
        }
        self.request(Method::POST, "/user/repos", Some(json!({"name":repo_name,"private":true,"description":CHAT_MARKER,"has_issues":true,"has_projects":false,"has_wiki":false,"auto_init":false}))).await?;
        // Never silently fall back to a public repository.
        self.private_repo(&chat.repo).await?;
        let content = STANDARD.encode(serde_json::to_vec_pretty(&chat).map_err(|e| e.to_string())?);
        self.request(
            Method::PUT,
            &format!("/repos/{}/contents/.cricket/chat.json", chat.repo),
            Some(json!({"message":"Create private Cricket chat","content":content})),
        )
        .await
        .map_err(|e| {
            format!(
                "The private repository {} was created, but its manifest could not be saved: {e}",
                chat.repo
            )
        })?;
        let mut warnings = Vec::new();
        for member in &chat.members {
            if member.eq_ignore_ascii_case(login) {
                continue;
            }
            if let Err(e) = self
                .request(
                    Method::PUT,
                    &format!("/repos/{}/collaborators/{member}", chat.repo),
                    Some(json!({"permission":"push"})),
                )
                .await
            {
                warnings.push(format!(
                    "Invite @{member} from GitHub's repository settings: {e}"
                ));
            }
        }
        Ok((chat, warnings))
    }
    pub async fn invitations(&self) -> Result<Vec<Invitation>, String> {
        self.pages("/user/repository_invitations")
            .await?
            .into_iter()
            .map(serde_json::from_value::<Invitation>)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
            .map(|items| {
                items
                    .into_iter()
                    .filter(|i| {
                        i.repository
                            .description
                            .as_deref()
                            .is_some_and(|s| s.starts_with(CHAT_MARKER))
                    })
                    .collect()
            })
    }
    pub async fn accept(&self, invitation: u64) -> Result<(), String> {
        if !self.invitations().await?.iter().any(|i| i.id == invitation) {
            return Err("Cricket invitation not found.".into());
        }
        self.request(
            Method::PATCH,
            &format!("/user/repository_invitations/{invitation}"),
            None,
        )
        .await?;
        Ok(())
    }
    pub async fn transfers(&self, chat: &Chat) -> Result<Vec<Transfer>, String> {
        self.private_repo(&chat.repo).await?;
        let issues = self
            .pages(&format!(
                "/repos/{}/issues?state=all&sort=created&direction=asc",
                chat.repo
            ))
            .await?;
        let mut transfers = Vec::new();
        for value in issues {
            let issue: Issue = serde_json::from_value(value).map_err(|e| e.to_string())?;
            if let Some(transfer) = self.parse_transfer(chat, issue).await? {
                transfers.push(transfer);
            }
        }
        Ok(transfers)
    }
    pub async fn transfer(&self, chat: &Chat, number: u64) -> Result<Transfer, String> {
        self.private_repo(&chat.repo).await?;
        // GitHub's issue list can lag behind creation. Read the exact issue before acting.
        let issue: Issue = serde_json::from_value(
            self.get(&format!("/repos/{}/issues/{number}", chat.repo))
                .await?,
        )
        .map_err(|e| e.to_string())?;
        self.parse_transfer(chat, issue)
            .await?
            .ok_or_else(|| "This issue no longer contains a valid Cricket transfer.".into())
    }
    async fn parse_transfer(&self, chat: &Chat, issue: Issue) -> Result<Option<Transfer>, String> {
        if issue.pull_request.is_some() {
            return Ok(None);
        }
        let Some(body) = issue
            .body
            .as_deref()
            .and_then(|s| s.strip_prefix(OFFER_MARKER))
        else {
            return Ok(None);
        };
        if body.len() > 60_000 {
            return Ok(None);
        }
        let Ok(offer) = serde_json::from_str::<Offer>(body) else {
            return Ok(None);
        };
        if !offer.validate(chat, &issue.user.login) {
            return Ok(None);
        }
        let key = format!("{}#{}", chat.repo, issue.number);
        let revision = format!("{}:{}:{}", issue.updated_at, issue.comments, body);
        if let Some((updated, transfer)) = self.transfers.lock().await.get(&key) {
            if updated == &revision {
                return Ok(Some(transfer.clone()));
            }
        }
        let mut events = Vec::new();
        for value in self
            .pages(&format!(
                "/repos/{}/issues/{}/comments",
                chat.repo, issue.number
            ))
            .await?
        {
            let comment: Comment = serde_json::from_value(value).map_err(|e| e.to_string())?;
            if let Some(body) = comment
                .body
                .strip_prefix(EVENT_MARKER)
                .filter(|s| s.len() < 4096)
            {
                if let Ok(event) = serde_json::from_str::<Event>(body) {
                    events.push(AuthoredEvent {
                        author: comment.user.login,
                        event,
                    });
                }
            }
        }
        let transfer = Transfer {
            repo: chat.repo.clone(),
            issue: issue.number,
            offer,
            events,
        };
        self.transfers
            .lock()
            .await
            .insert(key, (revision, transfer.clone()));
        Ok(Some(transfer))
    }
    pub async fn offer(&self, repo: &str, offer: &Offer) -> Result<u64, String> {
        self.private_repo(repo).await?;
        let body = format!(
            "{OFFER_MARKER}{}",
            serde_json::to_string_pretty(offer).map_err(|e| e.to_string())?
        );
        let title = format!(
            "{} shared {} item{}",
            offer.sender,
            offer.files.len(),
            if offer.files.len() == 1 { "" } else { "s" }
        );
        self.request(
            Method::POST,
            &format!("/repos/{repo}/issues"),
            Some(json!({"title":title,"body":body})),
        )
        .await?["number"]
            .as_u64()
            .ok_or("GitHub did not return a transfer ID.".into())
    }
    pub async fn event(&self, repo: &str, issue: u64, event: &Event) -> Result<(), String> {
        self.private_repo(repo).await?;
        let body = format!(
            "{EVENT_MARKER}{}",
            serde_json::to_string(event).map_err(|e| e.to_string())?
        );
        self.request(
            Method::POST,
            &format!("/repos/{repo}/issues/{issue}/comments"),
            Some(json!({"body":body})),
        )
        .await?;
        self.transfers
            .lock()
            .await
            .remove(&format!("{repo}#{issue}"));
        Ok(())
    }
}
