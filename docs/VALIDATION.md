# Preview verification

## 0.1.3 normal croc connection selection

The sender now runs `croc --yes --disable-clipboard --ignore-stdin send -- <paths>`.
The receiver retains croc's normal defaults. Cricket no longer supplies
`--transport relay` or `--no-local`, maps codes to particular public relays, or
requires a successful public-relay preflight before sending. Fresh independent
codes remain environment-only, including retries and group deliveries.

Windows local checks: frontend tests/build, native unit tests, clippy, a real
native-backend nested-folder round trip using normal transport selection and an
isolated loopback rendezvous, plus a separate explicit relay-fallback round trip.
Tests check spaces, Unicode filenames, empty folders and exact content equality.
Command regression tests protect both automatic defaults and secret handling.

The manual CI workflows run the same native tests and package for Linux and Mac.
Codemagic is limited to one manually requested Apple Silicon run with a 20-minute
cap; Linux and Intel Mac use free public GitHub runners. Build results are linked
in the release notes. Mac package checks start the actual DMG app after relocating
it outside the checkout and verify bundled croc discovery and the authenticated API.

These same-host checks do not prove two-device LAN discovery, WAN traversal, or
Fedora KDE desktop integration. A same-host test with deliberately unreachable
rendezvous did not connect; we do not claim offline operation or guaranteed LAN
routing. GitHub chat coordination still needs internet. Public relay failures
observed in 0.1.2 can still affect croc when no alternate route is available.

## 0.1.2 relay fix (historical; preflight removed in 0.1.3)

Public-relay tests on September 22 reproduced a peer-handshake decoding failure
(`flate: corrupt input before offset 6`) on relays 1 and 4 with the pinned croc
11.5.3. The same fixture passed on relays 2 and 3. A successful local-relay test
did not establish public-relay compatibility. These observations describe the
relays at test time; they are not a permanent blocklist.

Before publishing an offer, Cricket now tests an encrypted round trip of a small
generated fixture through a candidate relay. It tries another candidate if the
check fails. Real transfer codes are independently generated for a working relay,
using croc's existing code-to-relay mapping, so older receivers can still receive.
Retry starts with a different candidate from the previous attempt. Each check is
bounded to 12 seconds and a send checks at most four candidates. Group recipients
share the checked relay but each has an independent code.

The sender needs this update; updating only the receiver cannot move an existing
offer to another relay. A sender-side check also cannot establish that the
recipient's network permits the connection or guarantee future relay availability.

Both croc output streams are inspected for fixed error categories and discarded;
raw output and transfer codes are never shown or persisted. The local failure
explanation stays with its transfer attempt across sync and app restarts.

Windows checks include the native public-relay preflight (starting at the failing
relay and falling through to a working one), frontend checks/build, and an actual
public-relay folder round trip with nested paths, spaces, Unicode filenames,
binary data, and an empty directory. Received file hashes match. Network tests
remain opt-in:

```sh
cargo test --manifest-path src-tauri/Cargo.toml public_relay_preflight -- --ignored
node tools/test-croc.mjs --public-relay --relay-index 1 --folder
```

`--relay-index` uses zero-based indexes (1 selects public relay 2). Interactive
Fedora-to-Windows verification requires the sender update and a fresh retry.

The packaged Windows 0.1.2 app also passed actual native send and receive flows
using a generated fixture through a public relay, exact content checks, and
durable received receipts. Its authenticated API checks and executable privacy
scan passed. Fourteen local Rust tests and five frontend tests passed; the
network preflight test was run separately. Clippy passed with warnings denied.
The Linux release uses one manual standard public-runner packaging job, with
native tests and a local-relay round trip; no Mac rebuild is part of this update.

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
