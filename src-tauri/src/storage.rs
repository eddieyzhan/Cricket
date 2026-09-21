use crate::model::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Serialize, Deserialize)]
pub struct PendingEvent {
    pub repo: String,
    pub issue: u64,
    pub authored: AuthoredEvent,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Inflight {
    pub key: String,
    pub recipient: String,
    pub attempt: u32,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Store {
    pub device_id: String,
    pub device_name: String,
    pub user: Option<User>,
    pub chats: Vec<Chat>,
    pub transfers: Vec<Transfer>,
    pub sources: HashMap<String, Vec<SourceFile>>,
    pub outbox: Vec<PendingEvent>,
    pub notified: HashSet<String>,
    pub last_sync: Option<u64>,
    #[serde(default)]
    pub inflight: Vec<Inflight>,
}
impl Default for Store {
    fn default() -> Self {
        Self {
            device_id: id(),
            device_name: hostname::get()
                .unwrap_or_default()
                .to_string_lossy()
                .chars()
                .take(100)
                .collect(),
            user: None,
            chats: vec![],
            transfers: vec![],
            sources: HashMap::new(),
            outbox: vec![],
            notified: HashSet::new(),
            last_sync: None,
            inflight: vec![],
        }
    }
}
pub fn recover(store: &mut Store) {
    let Some(user) = &store.user else {
        return;
    };
    for t in &mut store.transfers {
        for d in t.deliveries(now()) {
            let was_local = t.offer.source_device == store.device_id
                || store.inflight.iter().any(|j| {
                    j.key == t.key() && j.recipient == d.recipient && j.attempt == d.attempt
                });
            if was_local
                && matches!(
                    d.status,
                    Status::Waiting | Status::Seen | Status::Receiving | Status::Expired
                )
            {
                let authored = AuthoredEvent {
                    author: user.login.clone(),
                    event: Event {
                        id: id(),
                        recipient: d.recipient,
                        attempt: d.attempt,
                        status: Status::Failed,
                        at: now(),
                        code: None,
                        expires_at: None,
                    },
                };
                t.events.push(authored.clone());
                store.outbox.push(PendingEvent {
                    repo: t.repo.clone(),
                    issue: t.issue,
                    authored,
                });
            }
        }
    }
    store.inflight.clear();
}
pub fn load(dir: &Path) -> Result<Store, String> {
    let file = dir.join("state.json");
    if !file.exists() {
        return Ok(Store::default());
    }
    serde_json::from_slice(&std::fs::read(file).map_err(|e| e.to_string())?).map_err(|_| {
        "Cricket's local history could not be read. Back up state.json before repairing it.".into()
    })
}
pub fn save(dir: &Path, store: &Store) -> Result<(), String> {
    write_private(
        &dir.join("state.json"),
        &serde_json::to_vec(store).map_err(|e| e.to_string())?,
    )
}
pub fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let dir = path.parent().ok_or("Missing state directory")?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut file = tempfile::NamedTempFile::new_in(dir).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.as_file()
            .set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
    }
    file.write_all(bytes).map_err(|e| e.to_string())?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    file.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
pub fn credential() -> Result<keyring::Entry, String> {
    keyring::Entry::new("app.cricket.desktop", "github")
        .map_err(|_| "The operating system credential vault is unavailable.".into())
}
pub fn source_files(paths: &[String]) -> Result<Vec<SourceFile>, String> {
    use std::hash::{Hash, Hasher};
    if paths.is_empty() || paths.len() > 200 {
        return Err("Choose between 1 and 200 files or folders.".into());
    }
    let mut names = HashSet::new();
    let mut results = Vec::new();
    for input in paths {
        let raw = PathBuf::from(input);
        if !raw.is_absolute() {
            return Err("File paths must be absolute.".into());
        }
        let path = dunce::canonicalize(&raw)
            .map_err(|_| "One of the selected files is no longer available.".to_string())?;
        let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
        if !meta.is_file() && !meta.is_dir() {
            return Err("Only regular files and folders can be sent.".into());
        }
        let name = path
            .file_name()
            .ok_or("Select a file or folder, not a filesystem root.")?
            .to_string_lossy()
            .to_string();
        if name.contains(['/', '\\', '\0']) || name.len() > 255 {
            return Err("This filename is not supported across platforms.".into());
        }
        if !names.insert(name.to_lowercase()) {
            return Err(
                "Selected items have the same name. Put them in different folders before sending."
                    .into(),
            );
        }
        let mut stamp = std::collections::hash_map::DefaultHasher::new();
        let mut size = 0u64;
        for (count, entry) in walkdir::WalkDir::new(&path)
            .follow_links(false)
            .sort_by_file_name()
            .into_iter()
            .enumerate()
        {
            if count >= 100_000 {
                return Err(
                    "This selection contains more than 100,000 entries. Archive it before sending."
                        .into(),
                );
            }
            let entry = entry.map_err(|_| "Couldn't read every item in the selected folder.")?;
            if entry.file_type().is_symlink() {
                return Err("The selected folder contains a symbolic link. Remove it or send the target explicitly.".into());
            }
            let info = entry
                .metadata()
                .map_err(|_| "Couldn't inspect a selected file.")?;
            if !info.is_dir() && !info.is_file() {
                return Err("Only regular files and folders can be sent.".into());
            }
            entry
                .path()
                .strip_prefix(&path)
                .unwrap_or(entry.path())
                .hash(&mut stamp);
            info.len().hash(&mut stamp);
            info.modified()
                .map_err(|_| "Couldn't read file modification times.")?
                .hash(&mut stamp);
            if info.is_file() {
                size = size
                    .checked_add(info.len())
                    .ok_or("Selected files are too large.")?;
            }
        }
        let modified = stamp.finish();
        results.push(SourceFile {
            path: path.to_string_lossy().into_owned(),
            file: SharedFile {
                name,
                size,
                directory: meta.is_dir(),
            },
            modified,
        });
    }
    Ok(results)
}
pub async fn inspect(paths: Vec<String>) -> Result<Vec<SourceFile>, String> {
    tokio::task::spawn_blocking(move || source_files(&paths))
        .await
        .map_err(|_| "File inspection was interrupted.".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn folder_fingerprint_catches_changed_children_and_counts_bytes() {
        let temp = tempfile::tempdir().unwrap();
        let child = temp.path().join("hello.txt");
        std::fs::write(&child, "hello").unwrap();
        let paths = vec![temp.path().to_string_lossy().into_owned()];
        let before = source_files(&paths).unwrap();
        assert_eq!(before[0].file.size, 5);
        std::fs::write(&child, "hello again").unwrap();
        let after = source_files(&paths).unwrap();
        assert_ne!(before[0].modified, after[0].modified);
        assert_eq!(after[0].file.size, 11);
    }
    #[test]
    fn duplicate_destination_names_are_rejected() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join("a")).unwrap();
        std::fs::create_dir(temp.path().join("b")).unwrap();
        let paths: Vec<_> = ["a", "b"]
            .iter()
            .map(|dir| {
                let path = temp.path().join(dir).join("hello.txt");
                std::fs::write(&path, "test").unwrap();
                path.to_string_lossy().into_owned()
            })
            .collect();
        assert!(source_files(&paths).is_err());
    }
    #[test]
    fn atomic_state_roundtrip_retains_device_identity() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::default();
        save(temp.path(), &store).unwrap();
        assert_eq!(load(temp.path()).unwrap().device_id, store.device_id);
        save(temp.path(), &store).unwrap();
        assert_eq!(load(temp.path()).unwrap().device_id, store.device_id);
    }
    #[cfg(windows)]
    #[test]
    fn croc_receives_normal_windows_paths() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("a file with spaces.txt");
        std::fs::write(&file, "hello").unwrap();
        let verbatim = file.canonicalize().unwrap().to_string_lossy().into_owned();
        assert!(verbatim.starts_with("\\\\?\\"));
        let inspected = source_files(&[verbatim]).unwrap();
        assert!(!inspected[0].path.starts_with("\\\\?\\"));
        assert!(Path::new(&inspected[0].path).is_file());
    }
}
