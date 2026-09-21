<p align="center"><img src="assets/cricket.svg" width="76" alt="Cricket file transfer app" /></p>

# Cricket — cross-platform file transfer

**Send files and folders between Windows, Linux and macOS computers with a lightweight desktop GUI for [croc](https://github.com/schollz/croc).** Choose a chat, add files and click **Send**. Cricket handles transfer codes for you and shows delivery receipts for your devices, friends and groups.

File contents travel through croc's encrypted connection, using its normal automatic transport and local discovery. Cricket does not force an internet relay. Your GitHub account provides private chats and transfer history, so internet access is still needed for chat coordination. Keep both apps running until the transfer completes.

[Download Cricket](https://github.com/eddieyzhan/Cricket/releases) · [First transfer](#send-your-first-file) · [Troubleshooting](#troubleshooting) · [Build from source](docs/DEVELOPMENT.md)

![Cricket desktop file sharing interface with a transfer history and Receive button; fictional preview data](docs/preview.png)

## Download and install

Cricket is an **early preview**. Version **0.1.3** restores croc's normal connection selection on Windows, Linux and macOS. Update both devices, especially the sender, and retry existing failed offers after updating. [See what has been tested](docs/VALIDATION.md).

| Your computer | Download | How to install |
| --- | --- | --- |
| Windows x64 | [Installer](https://github.com/eddieyzhan/Cricket/releases/download/v0.1.3/Cricket_0.1.3_x64-setup.exe) or [portable ZIP](https://github.com/eddieyzhan/Cricket/releases/download/v0.1.3/Cricket_0.1.3_windows_x64-portable.zip) | Run the installer, or extract the **whole ZIP** and open `Cricket.exe`. |
| Debian / Ubuntu x64 | [DEB package](https://github.com/eddieyzhan/Cricket/releases/download/v0.1.3/Cricket_0.1.3_amd64.deb) | In the download folder, run `sudo apt install ./Cricket_0.1.3_amd64.deb`. |
| Fedora KDE / other Linux x64 | [AppImage](https://github.com/eddieyzhan/Cricket/releases/download/v0.1.3/Cricket_0.1.3_amd64.AppImage) | Make it executable with `chmod +x Cricket_0.1.3_amd64.AppImage`, then run `./Cricket_0.1.3_amd64.AppImage`. Compatible system libraries are required. |
| Apple Silicon Mac | [DMG](https://github.com/eddieyzhan/Cricket/releases/download/v0.1.3/Cricket_0.1.3_aarch64.dmg) | Open the disk image and drag Cricket into Applications. |
| Intel Mac | [DMG](https://github.com/eddieyzhan/Cricket/releases/download/v0.1.3/Cricket_0.1.3_x64.dmg) | Open the disk image and drag Cricket into Applications. |

Packaged builds include **croc 11.5.3**. You do **not** need Node.js, Rust or a separate croc installation to use them. Packages are unsigned; Mac builds are not notarized. Checksums are on the corresponding release page. See the [platform setup guide](docs/SETUP.md) for prerequisites and sign-in options.

## Send your first file

### 1. Connect GitHub on both computers

Install [GitHub CLI](https://cli.github.com/) and sign in:

```sh
gh auth login --hostname github.com --git-protocol https --web --scopes repo
```

Open Cricket and click **Sign in with GitHub CLI**. You can instead choose **Use an access token**; see [token setup](docs/SETUP.md#preview-sign-in). The `repo` scope allows Cricket to create private chats and grants broad private-repository access. Cricket stores its credential in your operating system's vault.

### 2. Choose who receives the files

| Send to… | What to do |
| --- | --- |
| Your other computer | Sign into the **same GitHub account** on both. On one computer, select **New chat**, leave recipients empty and click **My devices**. Click **Sync chats** on the other. |
| A friend | Select **New chat**, add their GitHub username and create the chat. They accept the repository invitation in Cricket or GitHub, then click **Sync chats**. |
| A group | Add multiple GitHub usernames. Each person accepts the invitation; the chat tracks delivery separately for each recipient. |

### 3. Send and receive

1. Select the chat on the sending computer.
2. Drag in files or folders, or use **Add files** / **Add folders**.
3. Click **Send**. Cricket shares the offer and starts croc without a public-relay preflight.
4. On the receiving computer, open the chat, click **Receive** and choose a download folder. Cricket creates a new subfolder for the transfer.
5. Wait for **Received**. Keep both computers online and Cricket running while files transfer.

A new offer waits for up to **15 minutes**. Closing the window keeps Cricket in the system tray; use the tray menu to quit. Incoming offers are polled rather than delivered instantly, so use **Sync chats** when you want to check immediately.

## What Cricket does

- **File and folder sharing:** transfer individual files or whole directories through croc's encrypted connection, with automatic transport selection and local discovery enabled.
- **Transfers between your own devices:** one GitHub account, with a shared transfer history.
- **Sharing with friends and groups:** saved usernames, private chat repositories and per-recipient receipts.
- **Retry failed transfers:** resend from the original computer with a fresh connection code, without losing history or useful failure explanations.
- **Lightweight desktop interface:** Tauri uses the system webview; there is no bundled Chromium or Node runtime.
- **Agent and CLI access:** an authenticated local API and Node helper support scripted transfers. See the [agent guide](docs/AGENT.md).

Cricket is for sending files while both sides are online. It does not provide offline cloud storage, automatic folder synchronization or text messaging. A group transfer sends a separate copy to each recipient.

## Troubleshooting

| Problem | What to try |
| --- | --- |
| Transfer fails a few seconds after **Receive** | Update both computers to 0.1.3, then click **Retry** on the sender and **Receive** again on the other computer. croc selects the connection; public-relay outages and network restrictions can still cause failures. |
| Transfer expired or the sender went offline | Keep both apps open. The receiver can choose **Ask to resend**; the original sending computer must choose **Retry**. |
| Source files changed or moved | Add the files again and create a new transfer. Retry checks the original files before sending. |
| Chat does not appear | Check the GitHub account, accept the invitation and click **Sync chats**. **New chat → Join existing chat** can import a known Cricket repository. |
| Linux sign-in cannot save a credential | Start and unlock a Secret Service-compatible credential vault, such as GNOME Keyring. See [Linux setup](docs/SETUP.md#preview-sign-in). |
| No desktop notification | Leave Cricket running and allow notifications in your OS. Open the chat and sync manually to check for an offer. |

**Sent** confirms that the sender finished; **Received** confirms the recipient's successful receipt. **Seen** only means the chat was opened. Failed receipts can wait locally until GitHub is reachable again.

If you report a problem, include the app version, operating systems on both sides and the visible error. Do not post transfer codes, tokens, local state files or private-chat contents in a public issue. More details: [setup and troubleshooting](docs/SETUP.md) · [validation](docs/VALIDATION.md).

## Privacy and limits

**File contents are not uploaded to GitHub.** GitHub stores private chat metadata: filenames, sizes, participants, expiring croc bearer codes and receipts. GitHub and every chat member can read that metadata and those codes, so only share a chat with people you trust. Encryption of file data does not hide chat metadata from GitHub. See [Security](SECURITY.md).

Current limits include ten members per chat, ten simultaneous local deliveries, 200 selected top-level items per transfer and 100,000 entries per selected folder. GitHub list operations are limited to 2,000 entries. Very large histories need future archiving support.

## For developers and agents

- [Build and develop](docs/DEVELOPMENT.md): source setup, local tests, packaging and browser-only demo.
- [Agent guide](docs/AGENT.md): status, send, receive, retry and authenticated local API.
- [Architecture](docs/ARCHITECTURE.md): private chat manifests, transfer offers and receipt protocol.
- [Validation](docs/VALIDATION.md): checks performed and platform limitations.

Licensed under [MIT](LICENSE). [croc](https://github.com/schollz/croc) is MIT licensed by Zack Scholl and contributors. Cricket is independent of GitHub and croc's maintainers.
