use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    sync::oneshot,
};

// croc 11.5.3 maps the SHA-256 of the complete code to its four public relays.
// Keep this mapping in sync with the pinned engine's codephrase.RelayIndex.
fn relay_index(code: &str) -> usize {
    usize::from(Sha256::digest(code.as_bytes())[31] % 4)
}

fn code_for_relay(index: usize) -> String {
    loop {
        let code = format!("{}-{}", crate::model::id(), crate::model::id());
        if relay_index(&code) == index {
            return code;
        }
    }
}

pub fn code_on_same_relay(checked_code: &str) -> String {
    code_for_relay(relay_index(checked_code))
}

/// Check a real encrypted round trip before publishing an offer. A TCP ping is
/// insufficient: some reachable public relays fail croc's peer handshake.
/// The probe sends only a generated fixture back to this device, never user files.
pub async fn new_code(binary: &Path, previous: Option<&str>) -> Result<String, String> {
    let first = previous
        .map(|code| (relay_index(code) + 1) % 4)
        .unwrap_or_else(|| relay_index(&crate::model::id()));
    for offset in 0..4 {
        let index = (first + offset) % 4;
        let probe = code_for_relay(index);
        if probe_relay(binary, probe).await.is_ok() {
            // Never reuse a probe's room or bearer code for the real transfer.
            return Ok(code_for_relay(index));
        }
    }
    Err("No compatible transfer relay is available. Check your connection and try sending again. No new transfer was started.".into())
}

async fn probe_relay(binary: &Path, code: String) -> Result<(), String> {
    let temp = tempfile::tempdir().map_err(|_| "Couldn't prepare a relay check.")?;
    let root = dunce::canonicalize(temp.path()).map_err(|_| "Couldn't prepare a relay check.")?;
    let input = root.join("relay-check.txt");
    let output = root.join("received");
    const CONTENT: &[u8] = b"Cricket relay connectivity check.\n";
    std::fs::write(&input, CONTENT).map_err(|_| "Couldn't prepare a relay check.")?;
    std::fs::create_dir(&output).map_err(|_| "Couldn't prepare a relay check.")?;
    let (sender_cancel, sender_rx) = oneshot::channel();
    let (receiver_cancel, receiver_rx) = oneshot::channel();
    let deadline = crate::model::now() + 10;
    let sender = run(
        binary.to_path_buf(),
        code.clone(),
        Direction::Send(vec![input.to_string_lossy().into_owned()]),
        deadline,
        sender_rx,
    );
    let receiver = async {
        tokio::time::sleep(Duration::from_millis(250)).await;
        run(
            binary.to_path_buf(),
            code,
            Direction::Receive(output.clone()),
            deadline,
            receiver_rx,
        )
        .await
    };
    // Bound the entire probe, including after a progress marker. Wait for both
    // child processes to exit before the temporary directory is removed.
    let transfers = async { tokio::join!(sender, receiver) };
    tokio::pin!(transfers);
    let result = tokio::select! {
        result = &mut transfers => result,
        _ = tokio::time::sleep(Duration::from_secs(12)) => {
            let _ = sender_cancel.send(());
            let _ = receiver_cancel.send(());
            transfers.await
        }
    };
    match result {
        (Ok(()), Ok(())) => {}
        _ => return Err("Relay connectivity check failed.".into()),
    }
    if std::fs::read(output.join("relay-check.txt"))
        .ok()
        .as_deref()
        != Some(CONTENT)
    {
        return Err("Relay connectivity check failed.".into());
    }
    Ok(())
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Failure {
    #[default]
    Unknown,
    Connection,
    Filesystem,
    Protocol,
}

impl Failure {
    fn inspect(text: &str) -> Self {
        let text = text.to_ascii_lowercase();
        if text.contains("problem with decoding") || text.contains("decompress message") {
            Self::Protocol
        } else if text.contains("permission denied")
            || text.contains("access is denied")
            || text.contains("no space left")
            || text.contains("not enough space")
            || text.contains("filename, directory name")
        {
            Self::Filesystem
        } else if text.contains("connection")
            || text.contains("no such host")
            || text.contains("could not find sender")
            || text.contains("no public relay")
            || text.contains("timed out")
        {
            Self::Connection
        } else {
            Self::Unknown
        }
    }

    fn message(self) -> String {
        match self {
            Self::Protocol => "The relay could not complete the transfer handshake. Update Cricket on the sending device, then retry there to select a working relay.",
            Self::Connection => "Could not connect to the sender or transfer relay. Keep both devices online and retry from the sending device.",
            Self::Filesystem => "croc could not read or write a file. Check folder permissions, available disk space, and filenames supported by the receiving device.",
            Self::Unknown => "The transfer stopped before it finished. Check that both devices are online, then retry.",
        }.into()
    }
}

async fn monitor(
    mut stream: impl AsyncRead + Unpin,
    activity: tokio::sync::watch::Sender<bool>,
) -> Failure {
    let mut buffer = [0u8; 4096];
    let mut tail = Vec::new();
    let mut failure = Failure::Unknown;
    while let Ok(n) = stream.read(&mut buffer).await {
        if n == 0 {
            break;
        }
        if buffer[..n].contains(&b'%') {
            let _ = activity.send(true);
        }
        tail.extend_from_slice(&buffer[..n]);
        let found = Failure::inspect(&String::from_utf8_lossy(&tail));
        if found != Failure::Unknown && failure != Failure::Protocol {
            failure = found;
        }
        if tail.len() > 4096 {
            tail.drain(..tail.len() - 4096);
        }
    }
    // Only a fixed error category leaves this function. Output may contain
    // secrets and paths; never return, persist, or log it.
    failure
}

pub fn hidden(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    command
}
pub fn executable(resource_dir: &Path) -> PathBuf {
    let name = if cfg!(windows) { "croc.exe" } else { "croc" };
    let bundled = resource_dir.join("binaries").join(name);
    if bundled.is_file() {
        return bundled;
    }
    #[cfg(debug_assertions)]
    {
        let development = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("binaries")
            .join(name);
        if development.is_file() {
            return development;
        }
    }
    PathBuf::from(name)
}
pub async fn version(binary: &Path) -> Result<String, String> {
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        hidden(Command::new(binary).arg("--version")).output(),
    )
    .await
    .map_err(|_| "croc did not respond".to_string())?
    .map_err(|_| {
        "croc isn't installed. Run npm run prepare:croc when building Cricket.".to_string()
    })?;
    if !result.status.success() {
        return Err("Could not start croc.".into());
    }
    let version = String::from_utf8_lossy(&result.stdout).trim().to_string();
    // v11.5.3 fixes headless stdin behavior and is the pinned, tested baseline.
    let number = version
        .split_whitespace()
        .last()
        .unwrap_or("")
        .trim_start_matches('v');
    let parts: Vec<u32> = number
        .split('.')
        .take(3)
        .map(|p| p.parse().unwrap_or(0))
        .collect();
    if parts.as_slice() < [11, 5, 3].as_slice() {
        return Err("Cricket needs croc 11.5.3 or newer. Run npm run prepare:croc.".into());
    }
    Ok(version)
}
#[derive(Clone)]
pub enum Direction {
    Send(Vec<String>),
    Receive(PathBuf),
}
pub async fn run(
    binary: PathBuf,
    code: String,
    direction: Direction,
    deadline: u64,
    cancel: oneshot::Receiver<()>,
) -> Result<(), String> {
    let mut command = Command::new(binary);
    hidden(&mut command);
    // The bearer secret is never placed in the process arguments or clipboard.
    command
        .env("CROC_SECRET", code)
        .args(["--yes", "--disable-clipboard", "--ignore-stdin"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    match direction {
        Direction::Send(paths) => {
            command
                .args(["send", "--transport", "relay", "--no-local", "--"])
                .args(paths);
        }
        Direction::Receive(folder) => {
            command.arg("--out").arg(folder);
        }
    }
    let mut child = command.spawn().map_err(|_| {
        "Could not start croc. Check that it is installed and executable.".to_string()
    })?;
    let stderr = child.stderr.take().ok_or("Couldn't monitor croc")?;
    let stdout = child.stdout.take().ok_or("Couldn't monitor croc")?;
    let (activity, mut progress) = tokio::sync::watch::channel(false);
    let mut stderr_reader = tokio::spawn(monitor(stderr, activity.clone()));
    let mut stdout_reader = tokio::spawn(monitor(stdout, activity));
    let deadline = tokio::time::Instant::now()
        + Duration::from_secs(deadline.saturating_sub(crate::model::now()).max(1));
    let timeout = async move {
        let mut progress_open = true;
        loop {
            tokio::select! {
                _ = tokio::time::sleep_until(deadline) => break,
                changed = progress.changed(), if progress_open => {
                    if changed.is_err() { progress_open = false; }
                    else if *progress.borrow() { tokio::time::sleep(Duration::from_secs(24*60*60)).await; break; }
                }
            }
        }
    };
    let result = tokio::select! {
        result = child.wait() => match result {
            Ok(status) if status.success() => Ok(()),
            _ => {
                let outputs = tokio::time::timeout(Duration::from_secs(1), async {
                    tokio::join!(&mut stderr_reader, &mut stdout_reader)
                }).await;
                let failure = match outputs {
                    Ok((Ok(Failure::Protocol), _)) | Ok((_, Ok(Failure::Protocol))) => Failure::Protocol,
                    Ok((Ok(Failure::Filesystem), _)) | Ok((_, Ok(Failure::Filesystem))) => Failure::Filesystem,
                    Ok((Ok(Failure::Connection), _)) | Ok((_, Ok(Failure::Connection))) => Failure::Connection,
                    _ => Failure::Unknown,
                };
                Err(failure.message())
            },
        },
        _ = timeout => { let _ = child.kill().await; Err("The transfer timed out. You can retry from its chat.".into()) },
        _ = cancel => { let _ = child.kill().await; Err("Transfer cancelled.".into()) }
    };
    stderr_reader.abort();
    stdout_reader.abort();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_select_the_requested_public_relay() {
        for index in 0..4 {
            let code = code_for_relay(index);
            assert!(crate::model::valid_code(&code));
            assert!(relay_index(&code) == index);
        }
    }

    #[tokio::test]
    async fn split_output_is_classified_without_exposing_secrets() {
        use tokio::io::AsyncWriteExt;
        let (mut writer, reader) = tokio::io::duplex(16);
        let (activity, _) = tokio::sync::watch::channel(false);
        let reading = tokio::spawn(monitor(reader, activity));
        writer
            .write_all(b"private-code: problem with decoding: decompress message")
            .await
            .unwrap();
        drop(writer);
        let failure = reading.await.unwrap();
        assert!(failure == Failure::Protocol);
        assert!(!failure.message().contains("private-code"));
    }

    #[tokio::test]
    #[ignore = "requires the prepared croc binary and network; run explicitly"]
    async fn public_relay_preflight() {
        let name = if cfg!(windows) { "croc.exe" } else { "croc" };
        let binary = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("binaries")
            .join(name);
        // Start at relay 1, which reproduced the compatibility failure. The
        // check must continue to another relay when the first fails.
        let previous = code_for_relay(3);
        let code = new_code(&binary, Some(&previous)).await.unwrap();
        probe_relay(&binary, code).await.unwrap();
    }
}
