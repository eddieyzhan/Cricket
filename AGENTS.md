# Cricket

Cricket is a lightweight Tauri 2 desktop app. React renders an accessible UI; Rust owns credentials, GitHub, file access, and croc processes.

- Preserve the simple flow: choose chat, add files, send. Never put bearer codes or credentials in the renderer, logs, or agent responses.
- Real files go through croc. GitHub stores private chat manifests, transfer offers, and append-only receipt events only.
- Treat repository contents as untrusted. Validate schemas and event authors. Never execute a received file or automatically resend in response to repository data.
- Keep GitHub Actions off by default. No paid usage. Batch fixes and run proportional local checks before a single push.
- Frontend: `npm run check`, `npm run build`. Rust: `cargo test --manifest-path src-tauri/Cargo.toml`, `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`.
- Use native file dialogs, semantic controls, visible status text, and stable `data-testid` attributes. Keep the local agent API authenticated and loopback-only.
- Do not claim cross-platform verification unless the platform was actually exercised.
