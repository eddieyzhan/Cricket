# Build and develop Cricket

[Back to the user guide](../README.md)

Clone the repository and open its directory before running the commands below:

```sh
git clone https://github.com/eddieyzhan/Cricket.git
cd Cricket
```

Install Node.js **22.12+**, stable Rust, and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your operating system. On Windows, Rust must be on `PATH`, and Visual Studio C++ Build Tools plus the Windows SDK are required. On Linux, also install the development packages for Secret Service/D-Bus and OpenSSL (`libdbus-1-dev`, `libssl-dev` on Debian/Ubuntu).

```sh
npm ci
npm run prepare:croc
npm run desktop
```

`prepare:croc` downloads the host platform's official croc release, verifies a SHA-256 pinned in source, and includes the upstream license. Supported build hosts: Windows x64/ARM64, macOS Intel/Apple Silicon, Linux x64/ARM64. Build on the target operating system. Release signing and macOS notarization are not configured in this preview.

```sh
# Browser-only interactive design preview (no real transfers or GitHub writes)
npm run dev

# Local checks
npm run check
npm run audit:privacy
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
node tools/test-croc.mjs

# Package on the current platform
npm run tauri -- build
# Or choose one bundle, e.g. Windows:
npm run tauri -- build --bundles nsis
```

The browser preview is explicitly labelled and uses fictional data. Native builds start with GitHub setup and use the real backend.

The optional packaging workflow is **manual only**. It refuses to run in private repositories, uses standard public runners, and uploads packages directly to an existing draft release. It creates no Actions artifacts or caches and never runs on a push. Windows packaging is performed locally; Linux and macOS builds can be requested together once the local checks pass. Repository Actions are disabled between releases; enable them only for a deliberate packaging run. Standard public runners are [free under GitHub's runner policy](https://docs.github.com/en/actions/reference/runners/github-hosted-runners#standard-github-hosted-runners-for-public-repositories).

## GitHub browser sign-in

Branded browser sign-in uses GitHub's device authorization flow and requires registering a Cricket OAuth app with device flow enabled. Set `CRICKET_GITHUB_CLIENT_ID` **when compiling Rust** to show the **Continue with GitHub** button. No client secret belongs in the app. Builds without this configuration still support GitHub CLI and token sign-in. See [SETUP.md](SETUP.md).
