use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{io::AsyncReadExt, process::Command, sync::oneshot};

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
    let development = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("binaries")
        .join(name);
    if cfg!(debug_assertions) && development.is_file() {
        return development;
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
        .stdout(Stdio::null())
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
    let mut stderr = child.stderr.take().ok_or("Couldn't monitor croc")?;
    let (activity, mut progress) = tokio::sync::watch::channel(false);
    let reader = tokio::spawn(async move {
        let mut buffer = [0u8; 4096];
        while let Ok(n) = stderr.read(&mut buffer).await {
            if n == 0 {
                break;
            }
            // Inspect only progress markers. Raw output can contain bearer codes and is discarded.
            if buffer[..n].contains(&b'%') {
                let _ = activity.send(true);
            }
        }
    });
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
            _ => Err("The transfer stopped before it finished. Check that both devices are online, then retry.".into()),
        },
        _ = timeout => { let _ = child.kill().await; Err("The transfer timed out. You can retry from its chat.".into()) },
        _ = cancel => { let _ = child.kill().await; Err("Transfer cancelled.".into()) }
    };
    reader.abort();
    result
}
