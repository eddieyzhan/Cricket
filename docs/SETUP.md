# GitHub and platform setup

## Install the desktop preview

Download a package and `SHA256SUMS` from [Cricket's release page](https://github.com/eddieyzhan/Cricket/releases/tag/v0.1.0).

| Computer                 | Package          | Install                                                                                          |
| ------------------------ | ---------------- | ------------------------------------------------------------------------------------------------ |
| Windows x64              | `x64-setup.exe`  | Open the installer. For the portable ZIP, extract the whole folder before opening `Cricket.exe`. |
| Linux x64, Debian/Ubuntu | `amd64.deb`      | Install with your package manager, for example `sudo apt install ./Cricket_0.1.0_amd64.deb`.     |
| Linux x64, AppImage      | `amd64.AppImage` | Make the downloaded file executable, then open it.                                               |
| Apple Silicon Mac        | `aarch64.dmg`    | Open the disk image and drag Cricket into Applications.                                          |
| Intel Mac                | `x64.dmg`        | Open the disk image and drag Cricket into Applications.                                          |

Linux packages are built on Ubuntu 22.04; other distributions must provide compatible system libraries. These preview packages are unsigned and Mac builds are not notarized, so the OS may show a trust prompt. See [verification details](VALIDATION.md) for the scope of testing.

## Preview sign-in

An installed build includes croc and needs a GitHub account. You can:

1. Use [GitHub CLI](https://cli.github.com/): run `gh auth login --hostname github.com --git-protocol https --web --scopes repo`, finish browser authorization, then click **Use GitHub CLI sign-in** in Cricket. Cricket retrieves the credential from the installed CLI only after that explicit click, verifies the account, and saves it in the system credential vault.
2. Use a personal access token: open **Connect with an access token instead**. A classic token with the `repo` scope supports private repository creation, invitations, issues, and manifests. This scope grants broad access to repositories; consider a dedicated GitHub account if you want to isolate Cricket's authorization. Never put a token in a repository or an environment file committed to Git.

Fine-grained tokens can be used for existing chat repositories when granted the necessary repository permissions, but their owner/repository restrictions can prevent discovery, new repository creation, or invitations. Full first-run behavior is tested with a GitHub CLI token.

On Linux, Cricket requires a running and unlocked Secret Service credential vault (for example GNOME Keyring or a compatible service). It will refuse to persist a credential in plaintext if that vault is unavailable.

## Register a branded OAuth application

This is a maintainer step for builds that offer browser sign-in without requiring GitHub CLI or token entry.

1. Go to GitHub **Settings → Developer settings → OAuth Apps → New OAuth App**.
2. Name it **Cricket**, set the homepage to `https://github.com/eddieyzhan/Cricket`, and provide a callback URL such as `http://127.0.0.1` (device flow does not use a callback).
3. Enable **Device Flow** in the application's settings.
4. Set `CRICKET_GITHUB_CLIENT_ID` to the public client ID in the environment running the native build. Never bundle a client secret.

```powershell
# Windows PowerShell, before building
$env:CRICKET_GITHUB_CLIENT_ID = 'your-public-client-id'
npm run tauri -- build --bundles nsis
```

```sh
# macOS / Linux
CRICKET_GITHUB_CLIENT_ID=your-public-client-id npm run tauri -- build
```

Cricket displays the short user code, opens `https://github.com/login/device`, and polls at GitHub's requested interval. Pending authorization, `slow_down`, denial, and expiry are handled. This preview targets the default non-expiring OAuth token configuration. Reconnect if authorization is revoked or expired; automatic refresh of expiring OAuth tokens is not yet implemented.

References: [GitHub device authorization](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps#device-flow), [OAuth scopes](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/scopes-for-oauth-apps).

## Native prerequisites

Follow [Tauri's platform instructions](https://v2.tauri.app/start/prerequisites/). On Windows install stable Rust with the MSVC toolchain, Visual Studio C++ Build Tools, a Windows SDK, and WebView2. The installer uses the system WebView2 runtime. On macOS use Xcode command-line tools. On Linux install WebKitGTK 4.1, AppIndicator, build tools, OpenSSL, and D-Bus development packages.

Packaged previews are unsigned. Signing identities, certificates, and Apple notarization credentials are not included. Public installers should eventually be signed before calling Cricket stable.

## Troubleshooting

- **Chat not appearing:** accept the GitHub repository invitation, then click **Sync chats**. Importing `owner/repo` also works if it contains a valid Cricket manifest naming your account.
- **Permission denied / rate limit:** verify the account has repository access and its authorization scope is sufficient; wait before retrying. Failed receipt updates remain in the local outbox.
- **Disconnected sender:** use **Ask to resend**, then retry from the source device when it is online. New attempts get fresh codes.
- **No notification:** leave Cricket running in the system tray and enable notifications in OS settings. Incoming offers are polled, not pushed instantly.
- **Changed files on retry:** reselect and send a new transfer. Source metadata is rechecked to avoid silently changing an old offer.
- **croc missing in a development build:** run `npm run prepare:croc`, then rebuild. Production packages include the verified binary.
