# AI Gauge

Native Rust desktop app for monitoring AI usage limits and API balances in the system tray. Uses Slint with a software renderer; Python, Node.js, Qt, and a webview are not required.

Supports **Windows x64 (MSVC)** and **Linux x86-64**. macOS is not supported. The application interface is currently in Polish. See the [Polish user guide](README.pl.md) for detailed account setup and usage.

## Clone and run

Install [Git](https://git-scm.com/) and [Rust through rustup](https://rustup.rs/), plus the prerequisites below. `rust-toolchain.toml` selects Rust 1.97.0 and the formatting/lint tools automatically. Keep `Cargo.lock` to use the tested dependency versions.

### Windows

Install **Build Tools for Visual Studio** with **Desktop development with C++**, including the MSVC x64 compiler and Windows SDK. Use the default `x86_64-pc-windows-msvc` Rust host. Restart your terminal after installation.

In PowerShell:

```powershell
git clone https://github.com/BarMy49/ai-gauge-rust.git
cd ai-gauge-rust
cargo run --locked --release -- --demo
```

The first compilation downloads dependencies and can take several minutes. The executable is `target\release\ai-gauge.exe`.

### Linux

Use a graphical desktop session. On Ubuntu 24.04 / Debian-based systems:

```bash
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libfontconfig1-dev \
  libxkbcommon0 libxkbcommon-x11-0 libx11-6 libxcursor1 libxi6 libxrandr2 \
  libxcb-shape0 libxcb-xfixes0 dbus-x11 gnome-keyring
```

On Fedora:

```bash
sudo dnf install gcc gcc-c++ fontconfig-devel pkgconf-pkg-config \
  libxkbcommon libX11 libXcursor libXi libXrandr dbus-daemon gnome-keyring
```

Then:

```bash
git clone https://github.com/BarMy49/ai-gauge-rust.git
cd ai-gauge-rust
cargo run --locked --release -- --demo
```

The executable is `target/release/ai-gauge`. GNOME needs the **AppIndicator and KStatusNotifierItem Support** extension for the tray icon. KDE supports it directly. Without tray support the window stays available. A running, unlocked Secret Service (such as GNOME Keyring) is needed to remember API keys.

## First real launch

The demo uses temporary sample data and requires no accounts or network access at runtime. To use your own accounts, omit `--demo`:

```bash
cargo run --locked --release
```

Select a service, open **Konto i ustawienia**, sign in or enter the appropriate key, enable monitoring, and save. No `.env` file or configuration editing is required.

### Required CLI tools

The subscription trackers read usage through each provider's own command-line tool. **Install the CLI and sign in with it once** on the same computer and user account as AI Gauge:

| Tracker | CLI to install | Sign in once with |
| --- | --- | --- |
| ChatGPT / Codex | [Codex CLI](https://github.com/openai/codex) (`codex` on `PATH`) | `codex login` (or the app's own sign-in button) |
| Claude Code / Claude | [Claude Code](https://docs.claude.com/en/docs/claude-code) | `claude`, then `/login` |
| Gemini | [Gemini CLI](https://github.com/google-gemini/gemini-cli): `npm install -g @google/gemini-cli` | `gemini`, then choose "Login with Google" |

AI Gauge only reads these sign-ins (`~/.codex`, `~/.claude/.credentials.json`, `~/.gemini/oauth_creds.json`) and never changes them. If Claude Code's sign-in has expired, run `claude` briefly and refresh. Without Claude Code or Gemini CLI the app falls back to the browser sign-in, which claude.ai currently blocks for background reads.

To test every tracker from a terminal, run `ai-gauge --check`. It refreshes each service once and prints the readings or the error (no keys or tokens).

| Service | Required access |
| --- | --- |
| ChatGPT / Codex | ChatGPT sign-in; displays the account's Codex limits |
| OpenAI API | Organization admin API key for cost reports |
| DeepSeek API | Standard API key for balance reads |
| Claude | Claude Code signed in on this computer (preferred), or browser sign-in to the usage panel |
| Anthropic API | Organization admin API key for cost reports |
| Gemini | Gemini CLI signed in on this computer (preferred), or browser sign-in |

If Claude Code is signed in on this computer (`claude`, then `/login`), Claude limits are read through its sign-in, which is read-only and never changed. claude.ai blocks background browser reads with a Cloudflare check, so the browser route is only a fallback. Sign-in for Claude and Gemini opens an ordinary browser window without automation, so "Continue with Google" works. Under **Ustawienia**, a separate option shows how much of the reference amount is left for the DeepSeek balance and the OpenAI/Anthropic budgets.

Browser sign-in needs a system installation of Edge, Chrome, or Chromium; if the browser chosen in settings is not installed, any installed one is used. They use separate browser profiles; Flatpak browsers are not detected automatically. ChatGPT sign-in uses Codex CLI from `PATH`, or downloads the official authentication helper on demand. Initial download and sign-in require internet access. The app does not send generation prompts. Provider page changes can break browser integrations.

Closing the window hides it in the tray; use the tray menu or the application's exit button to quit. Windows may put the icon in the hidden-icons area.

Settings and browser sessions are stored in the OS application data directory. API keys are stored in Windows Credential Manager or Linux Secret Service when requested. To choose a data directory:

```bash
cargo run --locked --release -- --data-dir ./local-data
```

Do not commit or share that directory: browser and Codex profiles contain login sessions. See the [Polish guide](README.pl.md) for logout, budget calculations, and authentication details.

## Checks and builds

```bash
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked --release
cargo run --locked --release -- --smoke-test
```

The smoke test opens the GUI with offline sample data and exits after three seconds. On headless Linux, install `xvfb` and `xauth`, then run:

```bash
dbus-run-session -- xvfb-run -a target/release/ai-gauge --smoke-test
```

[GitHub Actions](.github/workflows/build.yml) runs formatting, tests, Clippy, release builds, and GUI startup checks on Windows and Ubuntu for pushes and pull requests. Successful runs upload executable artifacts. The UI, icons, and page adapter are embedded, so source assets need not accompany the executable. Linux executables still require the desktop libraries listed above.

## Troubleshooting

- **Cargo not found:** restart the terminal after installing rustup and check that its Cargo bin directory is on `PATH`.
- **Missing link.exe or Windows SDK:** install the C++ Build Tools components above and use the MSVC Rust toolchain.
- **Fontconfig or linker errors on Linux:** install the build prerequisites above, including `pkg-config` and Fontconfig development headers.
- **Window cannot open:** use a graphical desktop session, or the headless smoke-test command.
- **Key storage fails:** unlock/start your desktop keyring, or enter the key for the current session without enabling storage.
- **App already running:** restore the existing window from its tray icon or quit that instance first.
- **Build runs out of memory:** retry with `cargo build --locked --release -j 2`.

The [validation notes](VALIDATION.md) describe previous checks and integration limitations. Offline tests do not verify live provider accounts.

## License

Application source: [MIT](LICENSE). Slint and other dependencies retain their own licenses; see [THIRD-PARTY.md](THIRD-PARTY.md), the [Slint royalty-free license](licenses/LicenseRef-Slint-Royalty-free-2.0.md), the [Slint software license](licenses/LicenseRef-Slint-Software-3.0.md), and [GPL-3.0](licenses/GPL-3.0-only.txt).
