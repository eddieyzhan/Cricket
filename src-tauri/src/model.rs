use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

pub const OFFER_MARKER: &str = "<!-- cricket-transfer:v1 -->\n";
pub const EVENT_MARKER: &str = "<!-- cricket-event:v1 -->\n";
pub const CHAT_MARKER: &str = "Cricket private chat · v1";
pub const TRANSFER_TTL: u64 = 15 * 60;

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub fn id() -> String {
    uuid::Uuid::new_v4().to_string()
}
pub fn valid_login(s: &str) -> bool {
    !s.is_empty() && s.len() <= 39 && s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
}
pub fn valid_repo(s: &str) -> bool {
    let parts: Vec<_> = s.split('/').collect();
    parts.len() == 2
        && valid_login(parts[0])
        && !parts[1].is_empty()
        && parts[1].len() <= 100
        && parts[1]
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
        && !matches!(parts[1], "." | "..")
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub login: String,
    pub name: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Chat {
    pub repo: String,
    pub name: String,
    pub members: Vec<String>,
}
impl Chat {
    pub fn validate(&self, login: &str) -> Result<(), String> {
        if !valid_repo(&self.repo)
            || self.name.trim().is_empty()
            || self.name.len() > 80
            || self.members.is_empty()
            || self.members.len() > 10
            || self.members.iter().any(|m| !valid_login(m))
            || !self.members.iter().any(|m| m.eq_ignore_ascii_case(login))
        {
            return Err(
                "This repository does not contain a valid Cricket chat for your account.".into(),
            );
        }
        let unique: HashSet<_> = self
            .members
            .iter()
            .map(|m| m.to_ascii_lowercase())
            .collect();
        if unique.len() != self.members.len() {
            return Err("Duplicate chat members.".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SharedFile {
    pub name: String,
    pub size: u64,
    pub directory: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceFile {
    pub path: String,
    pub file: SharedFile,
    pub modified: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Slot {
    pub recipient: String,
    pub code: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Offer {
    pub id: String,
    pub sender: String,
    pub source_device: String,
    pub device_name: String,
    pub created_at: u64,
    pub expires_at: u64,
    pub files: Vec<SharedFile>,
    pub slots: Vec<Slot>,
}
impl Offer {
    pub fn validate(&self, chat: &Chat, author: &str) -> bool {
        self.sender.eq_ignore_ascii_case(author)
            && chat.members.iter().any(|m| m.eq_ignore_ascii_case(author))
            && uuid::Uuid::parse_str(&self.id).is_ok()
            && uuid::Uuid::parse_str(&self.source_device).is_ok()
            && self.device_name.len() <= 100
            && !self.files.is_empty()
            && self.files.len() <= 200
            && self.files.iter().all(|f| {
                !f.name.is_empty()
                    && f.name.len() <= 255
                    && !f.name.contains(['/', '\\', '\0'])
                    && f.name != "."
                    && f.name != ".."
            })
            && !self.slots.is_empty()
            && self.slots.len() <= 10
            && self.slots.iter().all(|s| {
                chat.members
                    .iter()
                    .any(|m| m.eq_ignore_ascii_case(&s.recipient))
                    && valid_code(&s.code)
            })
            && self
                .slots
                .iter()
                .map(|s| s.recipient.to_ascii_lowercase())
                .collect::<HashSet<_>>()
                .len()
                == self.slots.len()
            && self.expires_at > self.created_at
            && self.expires_at - self.created_at <= TRANSFER_TTL
    }
}
pub fn valid_code(code: &str) -> bool {
    code.len() >= 32
        && code.len() <= 128
        && code.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Waiting,
    Seen,
    Receiving,
    Sent,
    Received,
    Failed,
    Cancelled,
    RetryRequested,
    Expired,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub recipient: String,
    pub attempt: u32,
    pub status: Status,
    pub at: u64,
    pub code: Option<String>,
    pub expires_at: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthoredEvent {
    pub author: String,
    pub event: Event,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Transfer {
    pub repo: String,
    pub issue: u64,
    pub offer: Offer,
    pub events: Vec<AuthoredEvent>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Delivery {
    pub recipient: String,
    pub attempt: u32,
    pub status: Status,
    pub expires_at: u64,
    #[serde(skip_serializing)]
    pub code: String,
}
impl Transfer {
    pub fn key(&self) -> String {
        format!("{}#{}", self.repo, self.issue)
    }
    pub fn deliveries(&self, at: u64) -> Vec<Delivery> {
        self.offer
            .slots
            .iter()
            .map(|slot| {
                let mut d = Delivery {
                    recipient: slot.recipient.clone(),
                    attempt: 1,
                    status: Status::Waiting,
                    expires_at: self.offer.expires_at,
                    code: slot.code.clone(),
                };
                let mut ids = HashSet::new();
                for entry in &self.events {
                    let e = &entry.event;
                    if !e.recipient.eq_ignore_ascii_case(&slot.recipient)
                        || !ids.insert(&e.id)
                        || uuid::Uuid::parse_str(&e.id).is_err()
                    {
                        continue;
                    }
                    let sender = entry.author.eq_ignore_ascii_case(&self.offer.sender);
                    let recipient = entry.author.eq_ignore_ascii_case(&slot.recipient);
                    if e.status == Status::Waiting {
                        if sender
                            && e.attempt == d.attempt + 1
                            && e.code.as_deref().is_some_and(valid_code)
                            && e.expires_at
                                .is_some_and(|x| x > e.at && x - e.at <= TRANSFER_TTL)
                            && d.status != Status::Received
                        {
                            d.attempt = e.attempt;
                            d.status = Status::Waiting;
                            d.code = e.code.clone().unwrap();
                            d.expires_at = e.expires_at.unwrap();
                        }
                        continue;
                    }
                    if e.attempt != d.attempt || d.status == Status::Received {
                        continue;
                    }
                    match e.status {
                        Status::Received if recipient => d.status = Status::Received,
                        Status::RetryRequested if recipient => d.status = Status::RetryRequested,
                        Status::Sent if sender && !matches!(d.status, Status::RetryRequested) => {
                            d.status = Status::Sent
                        }
                        Status::Failed | Status::Cancelled if sender || recipient => {
                            if d.status != Status::Sent {
                                d.status = e.status.clone();
                            }
                        }
                        Status::Seen if recipient && d.status == Status::Waiting => {
                            d.status = Status::Seen
                        }
                        Status::Receiving
                            if recipient && matches!(d.status, Status::Waiting | Status::Seen) =>
                        {
                            d.status = Status::Receiving;
                            d.expires_at = e.at.saturating_add(24 * 60 * 60);
                        }
                        _ => {}
                    }
                }
                if at >= d.expires_at
                    && matches!(d.status, Status::Waiting | Status::Seen | Status::Receiving)
                {
                    d.status = Status::Expired;
                }
                d
            })
            .collect()
    }
}

#[derive(Clone, Serialize)]
pub struct TransferView {
    pub key: String,
    pub repo: String,
    pub issue: u64,
    pub sender: String,
    pub source_device: String,
    pub device_name: String,
    pub created_at: u64,
    pub files: Vec<SharedFile>,
    pub deliveries: Vec<Delivery>,
    pub can_retry: bool,
    pub error: Option<String>,
}
impl TransferView {
    pub fn new(t: &Transfer, device: &str, has_source: bool) -> Self {
        Self {
            key: t.key(),
            repo: t.repo.clone(),
            issue: t.issue,
            sender: t.offer.sender.clone(),
            source_device: t.offer.source_device.clone(),
            device_name: t.offer.device_name.clone(),
            created_at: t.offer.created_at,
            files: t.offer.files.clone(),
            deliveries: t.deliveries(now()),
            can_retry: t.offer.source_device == device && has_source,
            error: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Transfer {
        Transfer {
            repo: "alice/cricket-a".into(),
            issue: 1,
            offer: Offer {
                id: id(),
                sender: "alice".into(),
                source_device: id(),
                device_name: "Laptop".into(),
                created_at: 100,
                expires_at: 1000,
                files: vec![SharedFile {
                    name: "hello.txt".into(),
                    size: 4,
                    directory: false,
                }],
                slots: vec![Slot {
                    recipient: "bob".into(),
                    code: "a".repeat(40),
                }],
            },
            events: vec![],
        }
    }
    fn add(t: &mut Transfer, author: &str, status: Status, attempt: u32) {
        t.events.push(AuthoredEvent {
            author: author.into(),
            event: Event {
                id: id(),
                recipient: "bob".into(),
                attempt,
                status,
                at: 200,
                code: None,
                expires_at: None,
            },
        });
    }
    #[test]
    fn forged_receipts_are_ignored() {
        let mut t = fixture();
        add(&mut t, "mallory", Status::Received, 1);
        add(&mut t, "alice", Status::Received, 1);
        assert_eq!(t.deliveries(300)[0].status, Status::Waiting);
    }
    #[test]
    fn receipt_wins_over_racing_sender_failure() {
        let mut t = fixture();
        add(&mut t, "bob", Status::Received, 1);
        add(&mut t, "alice", Status::Failed, 1);
        assert_eq!(t.deliveries(2000)[0].status, Status::Received);
    }
    #[test]
    fn timeout_is_visible_without_sender_online() {
        assert_eq!(fixture().deliveries(1001)[0].status, Status::Expired);
    }
    #[test]
    fn retry_rotates_code_and_ignores_stale_receipts() {
        let mut t = fixture();
        add(&mut t, "alice", Status::Waiting, 2);
        t.events[0].event.code = Some("b".repeat(40));
        t.events[0].event.expires_at = Some(1100);
        add(&mut t, "bob", Status::Received, 1);
        let d = &t.deliveries(300)[0];
        assert_eq!(d.attempt, 2);
        assert_eq!(d.status, Status::Waiting);
        assert_eq!(d.code, "b".repeat(40));
    }
    #[test]
    fn receiver_cannot_publish_retry_code() {
        let mut t = fixture();
        add(&mut t, "bob", Status::Waiting, 2);
        t.events[0].event.code = Some("b".repeat(40));
        t.events[0].event.expires_at = Some(1100);
        assert_eq!(t.deliveries(300)[0].attempt, 1);
    }
    #[test]
    fn codes_never_appear_in_agent_or_ui_views() {
        let t = fixture();
        let json = serde_json::to_string(&TransferView::new(&t, "device", false)).unwrap();
        assert!(!json.contains(&"a".repeat(40)));
        assert!(!json.contains("\"code\""));
    }
    #[test]
    fn repo_input_cannot_escape_endpoint() {
        for s in [
            "alice/../tokens",
            "alice/x?access_token=x",
            "/x",
            "alice/..",
            "alice/x#frag",
        ] {
            assert!(!valid_repo(s));
        }
        assert!(valid_repo("alice/cricket-personal"));
    }
}
