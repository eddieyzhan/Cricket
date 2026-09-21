# Preview verification

Versions: **0.1.0 / 0.1.1**, September 2026. This is a working preview, not a claim of production readiness.

## Windows x64

The native Tauri application was built and launched locally. GitHub CLI sign-in connected the intended account, saved the credential in the OS vault, and survived app restarts. A private personal chat was created through Cricket.

The real native backend completed send and receive operations, compared downloaded contents with the original fixture, persisted a receiver receipt to GitHub, cancelled a sender, rotated the code on retry, and recorded the successful second attempt. The complete workflow used a local croc relay to isolate app behavior from public relay availability. A public-relay transfer also succeeded, but subsequent public-relay checks returned connection/decoding errors; network reliability is not guaranteed by these checks.

The packaged Windows executable was launched and its local agent API checked for authentication, loopback binding, browser Origin rejection, unknown-field rejection, platform detection, and snapshots without credentials or croc codes. There were no active jobs or unsynced receipts at handoff.

Local checks passed: 11 Rust tests, Clippy with warnings denied, 4 frontend tests, TypeScript checks, and the production frontend build. The UI was exercised in the browser preview for chat selection, group receipts, retry, new-chat forms, file selection, sending, and settings. Automated axe checks reported no WCAG 2 A/AA or 2.1 AA violations in the inspected chat and dialog states; this is not a complete accessibility audit.

## Linux and macOS

The [manual packaging run](https://github.com/eddieyzhan/Cricket/actions/runs/35647699555) uses native standard runners for Ubuntu 22.04 x64, macOS 14 Apple Silicon, and macOS 15 Intel. Each platform verifies the pinned croc archive, performs a real file round trip through a local relay, runs the native core tests, and builds its desktop packages. Packages upload directly to the release, without Actions artifact or cache storage.

This does **not** establish interactive GUI, credential-vault, notification, or cross-device behavior on those desktops. Testing with separate physical computers and different GitHub accounts remains part of the preview's next validation stage. Group authorization/receipt rules are covered by model tests; live group invitations and delivery have not been exercised with friends' accounts.

## Known boundaries

- Installers are unsigned; Mac builds are not notarized.
- The shipped sign-in options are GitHub CLI and access tokens. Branded browser sign-in needs a configured OAuth client.
- Both endpoints must remain online. GitHub stores metadata and codes, not the shared file bytes.
- Public croc relay outages can stop a transfer. The failed attempt remains in the history and can be retried.
- Local source paths, credentials, test fixtures, and private chat metadata are excluded from the public repository.

## 0.1.1 update

The interface now defaults to 900 × 640, with a compact send bar, shorter labels, and expandable group receipts. Browser checks cover the 900 × 640 layout, the 720 × 520 minimum, saved-person selection, remembering a newly added username, removing a selection, group creation, send, retry, and settings. The inspected chat and new-chat dialog states pass axe's WCAG 2 A/AA and 2.1 AA checks.

The saved-contact regression test loads an older state file without contacts, derives people from existing chats, excludes the current account and invalid usernames, deduplicates case-insensitively, and confirms persistence after saving/reloading. Local checks passed: 12 Rust tests, 5 frontend tests, TypeScript, and Clippy.

The [0.1.1 packaging run](https://github.com/eddieyzhan/Cricket/actions/runs/35652571107) repeats the croc round trip, 11 native tests, and package builds on Linux and both Mac architectures, and adds an executable privacy scan before uploading each platform's packages.

### Public-data audit

Reachable Git history and current project files were checked against private runtime identifiers, chat repository names, transfer codes, agent credentials, local home-directory paths, and credential patterns. No matching private data was found in the source/history. The screenshot uses fictional preview data. The repository owner's public GitHub identity and intentional examples remain visible.

The old 0.1.0 Windows executable did contain local user-directory paths in build diagnostics and a development-only lookup. Its Windows installer and portable ZIP were removed from the release. Version 0.1.1 remaps build paths to generic locations and checks the uncompressed release executable before upload. The Windows installer and portable ZIP were also extracted and their contents scanned; both passed. The installed executable matches the audited portable build apart from Tauri's expected installer marker. Previously downloaded copies cannot be recalled. No credential or private chat-data exposure was found.

`npm run audit:privacy` repeats the repository check. Pass an uncompressed executable path to `node tools/audit-public.mjs` to include its bytes in the scan. Automated scans reduce risk; they are not a proof that every possible kind of private information is absent.
