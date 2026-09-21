use crate::{
    croc::{self, Direction},
    github::{GitHub, Invitation},
    model::*,
    storage::{self, PendingEvent, Store},
};
use serde::Serialize;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::{oneshot, Mutex as AsyncMutex};

struct Job {
    cancel: Option<oneshot::Sender<()>>,
    view: JobView,
}
#[derive(Clone, Serialize)]
pub struct JobView {
    pub id: String,
    pub key: String,
    pub recipient: String,
    pub direction: String,
}
#[derive(Serialize)]
pub struct Snapshot {
    pub user: Option<User>,
    pub connected: bool,
    pub platform: String,
    pub device_id: String,
    pub device_name: String,
    pub chats: Vec<Chat>,
    pub transfers: Vec<TransferView>,
    pub jobs: Vec<JobView>,
    pub invitations: Vec<Invitation>,
    pub pending_receipts: usize,
    pub last_sync: Option<u64>,
    pub error: Option<String>,
    pub croc_version: Option<String>,
    pub oauth_available: bool,
}
pub struct Runtime {
    pub app: AppHandle,
    pub dir: PathBuf,
    pub binary: PathBuf,
    pub store: Mutex<Store>,
    pub api: Mutex<Option<GitHub>>,
    jobs: Mutex<HashMap<String, Job>>,
    pub error: Mutex<Option<String>>,
    pub croc_version: Mutex<Option<String>>,
    invitations: Mutex<Vec<Invitation>>,
    operation: AsyncMutex<()>,
    flushing: AsyncMutex<()>,
    discovered: Mutex<u64>,
    shutting_down: AtomicBool,
}
impl Runtime {
    pub fn new(app: AppHandle, dir: PathBuf, binary: PathBuf) -> Result<Arc<Self>, String> {
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let mut store = storage::load(&dir)?;
        storage::recover(&mut store);
        storage::save(&dir, &store)?;
        let token = storage::credential().and_then(|e| e.get_password().map_err(|e| e.to_string()));
        let api = token
            .ok()
            .filter(|s| !s.is_empty())
            .map(GitHub::new)
            .transpose()?;
        Ok(Arc::new(Self {
            app,
            dir,
            binary,
            store: Mutex::new(store),
            api: Mutex::new(api),
            jobs: Mutex::new(HashMap::new()),
            error: Mutex::new(None),
            croc_version: Mutex::new(None),
            invitations: Mutex::new(vec![]),
            operation: AsyncMutex::new(()),
            flushing: AsyncMutex::new(()),
            discovered: Mutex::new(0),
            shutting_down: AtomicBool::new(false),
        }))
    }
    pub fn mutate<T>(&self, f: impl FnOnce(&mut Store) -> Result<T, String>) -> Result<T, String> {
        let mut store = self.store.lock().unwrap();
        let mut next = store.clone();
        let result = f(&mut next)?;
        storage::save(&self.dir, &next)?;
        *store = next;
        Ok(result)
    }
    pub fn changed(&self) {
        let _ = self.app.emit("cricket:changed", ());
    }
    pub fn problem(&self, error: String) {
        *self.error.lock().unwrap() = Some(error);
        self.changed();
    }
    pub fn github(&self) -> Result<GitHub, String> {
        self.api
            .lock()
            .unwrap()
            .clone()
            .ok_or("Connect your GitHub account first.".into())
    }
    pub fn login(&self) -> Result<String, String> {
        self.store
            .lock()
            .unwrap()
            .user
            .as_ref()
            .map(|u| u.login.clone())
            .ok_or("Connect GitHub first.".into())
    }
    pub fn snapshot(&self) -> Snapshot {
        let s = self.store.lock().unwrap();
        Snapshot {
            user: s.user.clone(),
            connected: self.api.lock().unwrap().is_some(),
            platform: std::env::consts::OS.into(),
            device_id: s.device_id.clone(),
            device_name: s.device_name.clone(),
            chats: s.chats.clone(),
            transfers: s
                .transfers
                .iter()
                .map(|t| TransferView::new(t, &s.device_id, s.sources.contains_key(&t.key())))
                .collect(),
            jobs: self
                .jobs
                .lock()
                .unwrap()
                .values()
                .map(|j| j.view.clone())
                .collect(),
            invitations: self.invitations.lock().unwrap().clone(),
            pending_receipts: s.outbox.len(),
            last_sync: s.last_sync,
            error: self.error.lock().unwrap().clone(),
            croc_version: self.croc_version.lock().unwrap().clone(),
            oauth_available: option_env!("CRICKET_GITHUB_CLIENT_ID").is_some(),
        }
    }
    pub async fn connect(self: &Arc<Self>, token: String) -> Result<(), String> {
        let _guard = self.operation.lock().await;
        let token = token.trim().to_string();
        if token.len() < 20 || token.len() > 300 {
            return Err("Enter a valid GitHub access token.".into());
        }
        let api = GitHub::new(token.clone())?;
        let user = api.user().await?;
        if self
            .store
            .lock()
            .unwrap()
            .user
            .as_ref()
            .is_some_and(|old| !old.login.eq_ignore_ascii_case(&user.login))
        {
            return Err("Disconnect the current account before signing into another one.".into());
        }
        storage::credential()?.set_password(&token).map_err(|_| "Couldn't save the token in your operating system's credential vault. On Linux, unlock your Secret Service keyring.".to_string())?;
        self.mutate(|s| {
            s.user = Some(user);
            Ok(())
        })?;
        *self.api.lock().unwrap() = Some(api);
        *self.discovered.lock().unwrap() = 0;
        *self.error.lock().unwrap() = None;
        self.changed();
        Ok(())
    }
    pub async fn connect_cli(self: &Arc<Self>) -> Result<(), String> {
        let result = croc::hidden(tokio::process::Command::new("gh").args([
            "auth",
            "token",
            "--hostname",
            "github.com",
        ]))
        .output()
        .await
        .map_err(|_| {
            "Install GitHub CLI and run gh auth login, or connect with an access token below."
                .to_string()
        })?;
        if !result.status.success() {
            return Err("Sign in with gh auth login first, or use an access token below.".into());
        }
        self.connect(String::from_utf8(result.stdout).map_err(|_| "Invalid GitHub CLI credential")?)
            .await
    }
    pub async fn logout(&self) -> Result<(), String> {
        let _guard = self.operation.lock().await;
        if !self.jobs.lock().unwrap().is_empty() {
            return Err("Finish or cancel your active transfers before disconnecting.".into());
        }
        if !self.store.lock().unwrap().outbox.is_empty() {
            return Err("Sync your pending delivery receipts before disconnecting.".into());
        }
        let entry = storage::credential()?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(_) => return Err("Couldn't remove the stored GitHub credential.".into()),
        }
        self.mutate(|s| {
            *s = Store {
                device_id: s.device_id.clone(),
                device_name: s.device_name.clone(),
                ..Store::default()
            };
            Ok(())
        })?;
        *self.api.lock().unwrap() = None;
        self.invitations.lock().unwrap().clear();
        self.changed();
        Ok(())
    }
    pub async fn create_chat(
        self: &Arc<Self>,
        name: String,
        usernames: Vec<String>,
    ) -> Result<Chat, String> {
        let _guard = self.operation.lock().await;
        let login = self.login()?;
        let mut members = vec![login.clone()];
        for name in usernames {
            let name = name.trim().trim_start_matches('@').to_string();
            if !name.is_empty() && !members.iter().any(|m| m.eq_ignore_ascii_case(&name)) {
                members.push(name);
            }
        }
        let (chat, warnings) = self
            .github()?
            .create_chat(&login, name.trim().to_string(), members)
            .await?;
        self.mutate(|s| {
            s.chats.push(chat.clone());
            Ok(())
        })?;
        if !warnings.is_empty() {
            self.problem(warnings.join(" "));
        }
        self.changed();
        Ok(chat)
    }
    pub async fn import_chat(self: &Arc<Self>, repo: String) -> Result<Chat, String> {
        let _guard = self.operation.lock().await;
        let chat = self.github()?.chat(repo.trim(), &self.login()?).await?;
        self.mutate(|s| {
            s.chats.retain(|c| !c.repo.eq_ignore_ascii_case(&chat.repo));
            s.chats.push(chat.clone());
            Ok(())
        })?;
        self.changed();
        Ok(chat)
    }
    pub async fn accept_invitation(self: &Arc<Self>, invitation: u64) -> Result<(), String> {
        self.github()?.accept(invitation).await?;
        self.sync(true).await
    }
    pub async fn sync(self: &Arc<Self>, discover: bool) -> Result<(), String> {
        let Ok(_guard) = self.operation.try_lock() else {
            return Ok(());
        };
        let api = self.github()?;
        let login = self.login()?;
        self.flush().await?;
        if discover || now().saturating_sub(*self.discovered.lock().unwrap()) > 300 {
            let found = api.discover(&login).await?;
            self.mutate(|s| {
                for chat in found {
                    s.chats.retain(|c| !c.repo.eq_ignore_ascii_case(&chat.repo));
                    s.chats.push(chat);
                }
                Ok(())
            })?;
            *self.invitations.lock().unwrap() = api.invitations().await?;
            *self.discovered.lock().unwrap() = now();
        }
        let chats = self.store.lock().unwrap().chats.clone();
        let mut errors = Vec::new();
        for chat in chats {
            let result = async {
                let fresh = api.chat(&chat.repo, &login).await?;
                api.transfers(&fresh).await
            }
            .await;
            match result {
                Ok(mut incoming) => {
                    let notifications = self.mutate(|s| {
                        let mut notices = Vec::new();
                        for t in &mut incoming {
                            for pending in s
                                .outbox
                                .iter()
                                .filter(|p| p.repo == t.repo && p.issue == t.issue)
                            {
                                if !t
                                    .events
                                    .iter()
                                    .any(|e| e.event.id == pending.authored.event.id)
                                {
                                    t.events.push(pending.authored.clone());
                                }
                            }
                            if t.offer.source_device != s.device_id {
                                for d in t
                                    .deliveries(now())
                                    .into_iter()
                                    .filter(|d| d.recipient.eq_ignore_ascii_case(&login))
                                {
                                    let notice =
                                        format!("{}:{}:{}", t.key(), d.recipient, d.attempt);
                                    if matches!(d.status, Status::Waiting | Status::Seen)
                                        && s.notified.insert(notice)
                                    {
                                        notices.push(format!(
                                            "@{} shared {} item(s) in {}. Open Cricket to receive.",
                                            t.offer.sender,
                                            t.offer.files.len(),
                                            chat.name
                                        ));
                                    }
                                }
                            } else {
                                for d in t.deliveries(now()).into_iter().filter(|d| d.status == Status::RetryRequested) {
                                    if s.notified.insert(format!("retry:{}:{}:{}", t.key(), d.recipient, d.attempt)) {
                                        notices.push(format!("@{} asked you to retry a transfer in {}. Open Cricket to resend.", d.recipient, chat.name));
                                    }
                                }
                            }
                        }
                        // A list response can briefly omit a just-created issue. Keep local
                        // history and active jobs; actions revalidate the exact remote issue.
                        s.transfers.retain(|t| {
                            t.repo != chat.repo || !incoming.iter().any(|fresh| fresh.issue == t.issue)
                        });
                        s.transfers.extend(incoming);
                        Ok(notices)
                    })?;
                    for body in notifications {
                        let _ = self
                            .app
                            .notification()
                            .builder()
                            .title("Files are waiting for you")
                            .body(body)
                            .show();
                    }
                }
                Err(e) => errors.push(format!("{}: {e}", chat.name)),
            }
        }
        if errors.is_empty() {
            self.mutate(|s| {
                s.last_sync = Some(now());
                Ok(())
            })?;
            *self.error.lock().unwrap() = None;
        } else {
            self.problem(errors.join(" "));
        }
        self.changed();
        Ok(())
    }
    pub async fn flush(&self) -> Result<(), String> {
        let _guard = self.flushing.lock().await;
        let api = self.github()?;
        let outbox = self.store.lock().unwrap().outbox.clone();
        for pending in outbox {
            api.event(&pending.repo, pending.issue, &pending.authored.event)
                .await?;
            self.mutate(|s| {
                s.outbox
                    .retain(|p| p.authored.event.id != pending.authored.event.id);
                Ok(())
            })?;
        }
        Ok(())
    }
    pub fn enqueue(&self, transfer: &Transfer, event: Event) -> Result<(), String> {
        let authored = AuthoredEvent {
            author: self.login()?,
            event,
        };
        self.mutate(|s| {
            let current = s
                .transfers
                .iter_mut()
                .find(|t| t.key() == transfer.key())
                .ok_or("Transfer no longer exists locally")?;
            current.events.push(authored.clone());
            s.outbox.push(PendingEvent {
                repo: transfer.repo.clone(),
                issue: transfer.issue,
                authored,
            });
            Ok(())
        })?;
        self.changed();
        Ok(())
    }
    pub fn event(recipient: &str, attempt: u32, status: Status) -> Event {
        Event {
            id: id(),
            recipient: recipient.into(),
            attempt,
            status,
            at: now(),
            code: None,
            expires_at: None,
        }
    }
    fn transfer(&self, key: &str) -> Result<Transfer, String> {
        self.store
            .lock()
            .unwrap()
            .transfers
            .iter()
            .find(|t| t.key() == key)
            .cloned()
            .ok_or("Transfer not found. Refresh this chat.".into())
    }
    async fn fresh_transfer(&self, key: &str) -> Result<Transfer, String> {
        let old = self.transfer(key)?;
        let api = self.github()?;
        let chat = api.chat(&old.repo, &self.login()?).await?;
        let mut fresh = api.transfer(&chat, old.issue).await?;
        self.mutate(|s| {
            for p in s
                .outbox
                .iter()
                .filter(|p| p.repo == fresh.repo && p.issue == fresh.issue)
            {
                if !fresh
                    .events
                    .iter()
                    .any(|e| e.event.id == p.authored.event.id)
                {
                    fresh.events.push(p.authored.clone());
                }
            }
            s.transfers.retain(|t| t.key() != key);
            s.transfers.push(fresh.clone());
            Ok(())
        })?;
        Ok(fresh)
    }
    async fn require_croc(&self) -> Result<(), String> {
        let version = croc::version(&self.binary).await?;
        *self.croc_version.lock().unwrap() = Some(version);
        Ok(())
    }
    pub async fn send(
        self: &Arc<Self>,
        repo: String,
        paths: Vec<String>,
    ) -> Result<String, String> {
        let _guard = self.operation.lock().await;
        self.require_croc().await?;
        let login = self.login()?;
        let api = self.github()?;
        let chat = api.chat(&repo, &login).await?;
        let files = storage::inspect(paths).await?;
        let recipients: Vec<_> = chat
            .members
            .iter()
            .filter(|m| chat.members.len() == 1 || !m.eq_ignore_ascii_case(&login))
            .cloned()
            .collect();
        if self.jobs.lock().unwrap().len() + recipients.len() > 10 {
            return Err(
                "Finish some active transfers first (10 simultaneous deliveries maximum).".into(),
            );
        }
        let (device, device_name) = {
            let s = self.store.lock().unwrap();
            (s.device_id.clone(), s.device_name.clone())
        };
        let created_at = now();
        let offer = Offer {
            id: id(),
            sender: login,
            source_device: device,
            device_name,
            created_at,
            expires_at: created_at + TRANSFER_TTL,
            files: files.iter().map(|f| f.file.clone()).collect(),
            slots: recipients
                .into_iter()
                .map(|recipient| Slot {
                    recipient,
                    code: format!("{}-{}", id(), id()),
                })
                .collect(),
        };
        let issue = api.offer(&repo, &offer).await?;
        let transfer = Transfer {
            repo,
            issue,
            offer,
            events: vec![],
        };
        let key = transfer.key();
        self.mutate(|s| {
            s.sources.insert(key.clone(), files.clone());
            s.transfers.push(transfer.clone());
            Ok(())
        })?;
        for d in transfer.deliveries(now()) {
            self.start(
                transfer.clone(),
                d,
                Direction::Send(files.iter().map(|f| f.path.clone()).collect()),
            )?;
        }
        self.changed();
        Ok(key)
    }
    fn start(
        self: &Arc<Self>,
        t: Transfer,
        delivery: Delivery,
        direction: Direction,
    ) -> Result<(), String> {
        let key = t.key();
        let job_id = format!("{}:{}", key, delivery.recipient.to_lowercase());
        let sending = matches!(direction, Direction::Send(_));
        let (tx, rx) = oneshot::channel();
        {
            let jobs = self.jobs.lock().unwrap();
            if jobs.contains_key(&job_id) {
                return Err("This delivery is already running on this device.".into());
            }
            if jobs.len() >= 10 {
                return Err("Too many active deliveries. Try again when one finishes.".into());
            }
            drop(jobs);
            self.mutate(|s| {
                s.inflight.push(storage::Inflight {
                    key: key.clone(),
                    recipient: delivery.recipient.clone(),
                    attempt: delivery.attempt,
                });
                Ok(())
            })?;
            let mut jobs = self.jobs.lock().unwrap();
            if self.shutting_down.load(Ordering::Acquire) {
                return Err(
                    "Cricket is closing. Retry this transfer when it is open again.".into(),
                );
            }
            jobs.insert(
                job_id.clone(),
                Job {
                    cancel: Some(tx),
                    view: JobView {
                        id: job_id.clone(),
                        key,
                        recipient: delivery.recipient.clone(),
                        direction: if sending { "send" } else { "receive" }.into(),
                    },
                },
            );
        }
        let runtime = self.clone();
        tauri::async_runtime::spawn(async move {
            let result = croc::run(
                runtime.binary.clone(),
                delivery.code.clone(),
                direction,
                delivery.expires_at,
                rx,
            )
            .await;
            let status = match &result {
                Ok(()) if sending => Status::Sent,
                Ok(()) => Status::Received,
                Err(e) if e == "Transfer cancelled." => Status::Cancelled,
                Err(_) => Status::Failed,
            };
            if let Err(error) = runtime.enqueue(
                &t,
                Self::event(&delivery.recipient, delivery.attempt, status),
            ) {
                runtime.problem(format!("Couldn't save the delivery receipt: {error}"));
            }
            if let Err(error) = runtime.mutate(|s| {
                s.inflight.retain(|j| {
                    !(j.key == t.key()
                        && j.recipient == delivery.recipient
                        && j.attempt == delivery.attempt)
                });
                Ok(())
            }) {
                runtime.problem(error);
            }
            runtime.jobs.lock().unwrap().remove(&job_id);
            if let Err(error) = result {
                runtime.problem(error);
            } else if !sending {
                let _ = runtime
                    .app
                    .notification()
                    .builder()
                    .title("Files received")
                    .body("Your files are saved in the folder you chose.")
                    .show();
            }
            if let Err(error) = runtime.flush().await {
                runtime.problem(format!("Receipt saved locally; waiting to sync. {error}"));
            }
            runtime.changed();
        });
        Ok(())
    }
    pub async fn receive(
        self: &Arc<Self>,
        key: String,
        directory: String,
    ) -> Result<String, String> {
        let _guard = self.operation.lock().await;
        self.require_croc().await?;
        let login = self.login()?;
        let transfer = self.fresh_transfer(&key).await?;
        if transfer.offer.source_device == self.store.lock().unwrap().device_id {
            return Err("Open Cricket on your other device to receive these files.".into());
        }
        let delivery = transfer
            .deliveries(now())
            .into_iter()
            .find(|d| d.recipient.eq_ignore_ascii_case(&login))
            .ok_or("This transfer is addressed to someone else.")?;
        if !matches!(delivery.status, Status::Waiting | Status::Seen) {
            return Err(
                "This delivery is no longer waiting. Ask the sender to retry if needed.".into(),
            );
        }
        {
            let jobs = self.jobs.lock().unwrap();
            if jobs.len() >= 10 || jobs.contains_key(&format!("{}:{}", key, login.to_lowercase())) {
                return Err("This delivery is already running or this device has too many active transfers.".into());
            }
        }
        let parent = dunce::canonicalize(PathBuf::from(directory))
            .map_err(|_| "Choose an existing download folder.")?;
        if !parent.is_dir() {
            return Err("Choose a folder for your download.".into());
        }
        let folder = parent.join(format!(
            "Cricket-{}-{}",
            &transfer.offer.id[..8],
            &id()[..8]
        ));
        std::fs::create_dir(&folder).map_err(|_| "Couldn't create a download folder here.")?;
        self.enqueue(
            &transfer,
            Self::event(&login, delivery.attempt, Status::Receiving),
        )?;
        // A receipt can sync later; a temporary GitHub failure must not discard a valid offer.
        if let Err(error) = self.flush().await {
            self.problem(error);
        }
        self.start(transfer, delivery, Direction::Receive(folder.clone()))?;
        self.changed();
        Ok(folder.to_string_lossy().into_owned())
    }
    pub async fn retry(self: &Arc<Self>, key: String, recipient: String) -> Result<(), String> {
        let _guard = self.operation.lock().await;
        self.require_croc().await?;
        let transfer = self.fresh_transfer(&key).await?;
        let login = self.login()?;
        if !transfer.offer.sender.eq_ignore_ascii_case(&login)
            || transfer.offer.source_device != self.store.lock().unwrap().device_id
        {
            return Err("Retry from the device that originally sent these files.".into());
        }
        let mut delivery = transfer
            .deliveries(now())
            .into_iter()
            .find(|d| d.recipient.eq_ignore_ascii_case(&recipient))
            .ok_or("Recipient not found")?;
        if delivery.status == Status::Received {
            return Err("This recipient already received the files.".into());
        }
        if self
            .jobs
            .lock()
            .unwrap()
            .contains_key(&format!("{}:{}", key, recipient.to_lowercase()))
        {
            return Err("This transfer is still running. Cancel it before retrying.".into());
        }
        let files = self
            .store
            .lock()
            .unwrap()
            .sources
            .get(&key)
            .cloned()
            .ok_or("Reselect the original files to send a new transfer.")?;
        let current = storage::inspect(files.iter().map(|f| f.path.clone()).collect()).await?;
        if files
            .iter()
            .zip(&current)
            .any(|(old, new)| old.file.size != new.file.size || old.modified != new.modified)
        {
            return Err("A source changed. Reselect the files and send a new transfer so the recipient gets an accurate manifest.".into());
        }
        delivery.attempt += 1;
        delivery.code = format!("{}-{}", id(), id());
        delivery.expires_at = now() + TRANSFER_TTL;
        let mut event = Self::event(&recipient, delivery.attempt, Status::Waiting);
        event.code = Some(delivery.code.clone());
        event.expires_at = Some(delivery.expires_at);
        self.enqueue(&transfer, event)?;
        self.flush().await?;
        self.start(
            transfer,
            delivery,
            Direction::Send(current.into_iter().map(|f| f.path).collect()),
        )?;
        self.changed();
        Ok(())
    }
    pub async fn request_retry(self: &Arc<Self>, key: String) -> Result<(), String> {
        let _guard = self.operation.lock().await;
        let transfer = self.fresh_transfer(&key).await?;
        let login = self.login()?;
        let delivery = transfer
            .deliveries(now())
            .into_iter()
            .find(|d| d.recipient.eq_ignore_ascii_case(&login))
            .ok_or("This transfer is not addressed to you.")?;
        if matches!(
            delivery.status,
            Status::Received | Status::Waiting | Status::Seen | Status::Receiving
        ) {
            return Err("This delivery doesn't need a retry request.".into());
        }
        if delivery.status == Status::RetryRequested {
            return Ok(());
        }
        self.enqueue(
            &transfer,
            Self::event(&login, delivery.attempt, Status::RetryRequested),
        )?;
        if let Err(e) = self.flush().await {
            self.problem(format!("Retry request saved locally. {e}"));
        }
        Ok(())
    }
    pub async fn seen(self: &Arc<Self>, repo: String) -> Result<(), String> {
        let _guard = self.operation.lock().await;
        let login = self.login()?;
        let transfers = self.store.lock().unwrap().transfers.clone();
        let device = self.store.lock().unwrap().device_id.clone();
        for t in transfers
            .into_iter()
            .filter(|t| t.repo == repo && t.offer.source_device != device)
        {
            for d in t
                .deliveries(now())
                .into_iter()
                .filter(|d| d.recipient.eq_ignore_ascii_case(&login) && d.status == Status::Waiting)
            {
                self.enqueue(&t, Self::event(&login, d.attempt, Status::Seen))?;
            }
        }
        self.flush().await
    }
    pub fn cancel(&self, job_id: String) -> Result<(), String> {
        let mut jobs = self.jobs.lock().unwrap();
        let job = jobs
            .get_mut(&job_id)
            .ok_or("Transfer is no longer running.")?;
        if let Some(cancel) = job.cancel.take() {
            let _ = cancel.send(());
        }
        drop(jobs);
        self.changed();
        Ok(())
    }
    pub fn cancel_all(&self) {
        for job in self.jobs.lock().unwrap().values_mut() {
            if let Some(cancel) = job.cancel.take() {
                let _ = cancel.send(());
            }
        }
    }
    pub async fn shutdown(&self) {
        self.shutting_down.store(true, Ordering::Release);
        self.cancel_all();
        for _ in 0..60 {
            if self.jobs.lock().unwrap().is_empty() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        let _ = tokio::time::timeout(std::time::Duration::from_secs(4), self.flush()).await;
    }
}
