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

/// Generate an independent bearer code without requiring an internet relay.
/// Connection discovery and transport selection belong to croc.
pub fn new_code() -> String {
    format!("{}-{}", crate::model::id(), crate::model::id())
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
            Self::Protocol => "croc could not complete the transfer handshake. Update Cricket on both devices, then retry from the sending device with a fresh connection code.",
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
fn transfer_command(binary: &Path, code: &str, direction: Direction) -> Command {
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
                // Leave transport and local discovery at croc's normal defaults.
                .args(["send", "--"])
                .args(paths);
        }
        Direction::Receive(folder) => {
            command.arg("--out").arg(folder);
        }
    }
    command
}

pub async fn run(
    binary: PathBuf,
    code: String,
    direction: Direction,
    deadline: u64,
    cancel: oneshot::Receiver<()>,
) -> Result<(), String> {
    run_command(
        transfer_command(&binary, &code, direction),
        deadline,
        cancel,
    )
    .await
}

async fn run_command(
    mut command: Command,
    deadline: u64,
    cancel: oneshot::Receiver<()>,
) -> Result<(), String> {
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
    fn codes_are_valid_and_independent() {
        let first = new_code();
        let second = new_code();
        assert!(crate::model::valid_code(&first));
        assert!(crate::model::valid_code(&second));
        assert!(first != second);
    }

    #[test]
    fn commands_preserve_croc_connection_defaults_and_secret_privacy() {
        let binary = Path::new("croc");
        for (direction, expected) in [
            (
                Direction::Send(vec!["a folder/file.txt".into(), "--literal.txt".into()]),
                vec![
                    "--yes",
                    "--disable-clipboard",
                    "--ignore-stdin",
                    "send",
                    "--",
                    "a folder/file.txt",
                    "--literal.txt",
                ],
            ),
            (
                Direction::Receive(PathBuf::from("output folder")),
                vec![
                    "--yes",
                    "--disable-clipboard",
                    "--ignore-stdin",
                    "--out",
                    "output folder",
                ],
            ),
        ] {
            let command = transfer_command(binary, "test-secret", direction);
            let args: Vec<_> = command.as_std().get_args().collect();
            assert_eq!(args, expected);
            assert!(command
                .as_std()
                .get_envs()
                .any(|(key, value)| key == "CROC_SECRET"
                    && value == Some(std::ffi::OsStr::new("test-secret"))));
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
    #[ignore = "requires the prepared croc binary and loopback sockets; run explicitly"]
    async fn native_default_transfer() {
        let name = if cfg!(windows) { "croc.exe" } else { "croc" };
        // Cargo runs unit tests in the package directory, including remapped builds.
        let binary = std::env::current_dir().unwrap().join("binaries").join(name);
        let sockets: Vec<_> = (0..5)
            .map(|_| std::net::TcpListener::bind("127.0.0.1:0").unwrap())
            .collect();
        let port = sockets[0].local_addr().unwrap().port();
        let ports = sockets
            .iter()
            .map(|s| s.local_addr().unwrap().port().to_string())
            .collect::<Vec<_>>()
            .join(",");
        drop(sockets);
        let mut relay =
            hidden(Command::new(&binary).args(["relay", "--host", "127.0.0.1", "--ports", &ports]))
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .kill_on_drop(true)
                .spawn()
                .unwrap();
        let address = format!("127.0.0.1:{port}");
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if tokio::net::TcpStream::connect(&address).await.is_ok() {
                    break;
                }
                assert!(relay.try_wait().unwrap().is_none(), "fixture relay exited");
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .unwrap();
        let temp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(temp.path()).unwrap();
        let input = root.join("a folder with spaces");
        let output = root.join("received");
        std::fs::create_dir_all(input.join("empty")).unwrap();
        std::fs::create_dir_all(input.join("nested")).unwrap();
        std::fs::create_dir(&output).unwrap();
        let content = b"Cricket native automatic transport fixture.\n".repeat(4096);
        std::fs::write(input.join("nested").join("unicode-🦗.txt"), &content).unwrap();
        let code = new_code();
        let (_sender_cancel, sender_rx) = oneshot::channel();
        let (_receiver_cancel, receiver_rx) = oneshot::channel();
        let deadline = crate::model::now() + 40;
        let mut send_command = transfer_command(
            &binary,
            &code,
            Direction::Send(vec![input.to_string_lossy().into_owned()]),
        );
        send_command
            .env("CROC_RELAY", &address)
            .env("CROC_RELAY6", &address);
        let sender = run_command(send_command, deadline, sender_rx);
        let receiver = async {
            tokio::time::sleep(Duration::from_millis(1000)).await;
            let mut receive_command =
                transfer_command(&binary, &code, Direction::Receive(output.clone()));
            receive_command
                .env("CROC_RELAY", &address)
                .env("CROC_RELAY6", &address);
            run_command(receive_command, deadline, receiver_rx).await
        };
        let result = tokio::time::timeout(Duration::from_secs(50), async {
            tokio::join!(sender, receiver)
        })
        .await
        .unwrap();
        relay.kill().await.unwrap();
        assert!(result.0.is_ok(), "sender: {:?}", result.0);
        assert!(result.1.is_ok(), "receiver: {:?}", result.1);
        assert_eq!(
            std::fs::read(output.join("a folder with spaces/nested/unicode-🦗.txt")).unwrap(),
            content
        );
        assert!(output.join("a folder with spaces/empty").is_dir());
    }
}
