# Changelog

Every release on GitHub uses the section for its version from this file as its
release notes. Add a section here before tagging a new version.

## 4.0.1 — 2026-11-28

### Fixed
- **ChatGPT / Codex sign-in on ARM computers.** Without Codex CLI installed, the app
  downloaded OpenAI's x86_64 sign-in helper even on arm64 Linux, where it cannot run.
  It now downloads the helper built for the computer's processor: aarch64 or x86_64 on
  Linux, and arm64 or x64 on Windows.
- A wrong-processor helper saved by an older version is no longer used. Click
  **Zaloguj się z ChatGPT** once and the right one is downloaded.

## 4.0.0 — 2026-11-28

### Changed
- **New look for the whole window.** The app has its own light and dark color theme
  that matches the tray icon (mint accent on deep navy), instead of the plain gray
  system style.
  - **Przegląd** shows each service as a card with a colored badge, the value in large
    type, a usage bar and a status dot (green: fresh data, amber: stale, gray: none).
    Cards sit in two columns when the window is wide enough and in one column otherwise.
  - The sidebar has icons, a highlighted current page, the app version and
    **Schowaj do tray** at the bottom. Narrow windows keep the compact top bar.
  - Service pages switch between **Limity** and **Konto i ustawienia** with tabs.
    Each limit is a card; the pinned one carries a "Na ikonie tray" label.
    Sign-out and key removal sit in their own section in red.
  - **Ustawienia** is split into cards. Theme and text size are picked with
    segmented buttons, on/off options use switches with their explanation underneath.
  - **Dostosuj usługi** uses compact rows with a drag handle, a visibility switch
    and arrow buttons.
  - **O aplikacji** shows the app icon, version, license and the small Slint badge.
  - Status messages appear in a bar at the bottom of the window instead of between
    the page content.
- Every feature, option and keyboard shortcut works as before.

### Fixed
- Moving a service reported "pozycja N z 6" although there are 5 services. It now
  counts the real number of services.

## 0.3.2 — 2026-11-27

### Fixed
- **Claude Code no longer runs into "HTTP 429: limit zapytań".** Claude limits were
  requested at every refresh (every 60 seconds with a short interval), which the
  Claude usage service rejects as too frequent. The app now asks at most once every
  5 minutes, whatever the refresh interval is.
- After a 429 the app waits as long as Claude asks (`Retry-After`), or 15 minutes if
  Claude does not say, instead of retrying every refresh.

### Changed
- The last successful Claude reading is saved in the app's data folder
  (`claude-usage.json`) and shown while the app waits, together with the time it
  was read. It survives restarts, so the Claude tile no longer drops to
  "Brak odczytanych limitów" after a rate limit.
- **O aplikacji** shows the real version of the build (taken from `Cargo.toml`)
  instead of a fixed "0.3".
- The #MadeWithSlint badge on **O aplikacji** is now small instead of filling the page.

### Upgrade notes
- Close any older copy of AI Gauge before starting this one. Older copies keep asking
  Claude every refresh and prolong the rate limit for the whole account.

## 0.3.1 — 2026-11-27

### Added
- Trackers read usage through the providers' own command-line tools:
  - **ChatGPT / Codex** through Codex CLI (`codex login`).
  - **Claude Code / Claude** through Claude Code's sign-in (`claude`, then `/login`).
    The sign-in is only read, never changed.
- `ai-gauge --check` refreshes every tracker once and prints each reading or error
  (no keys or tokens). On Windows the report appears in the terminal that started it.
- README section **Required CLI tools** explains what to install and sign in with.
- Fedora releases are **.rpm packages** (`ai-gauge-<version>-1.fc43.<arch>.rpm`) for
  x86_64 and aarch64. Install with `sudo dnf install ./ai-gauge-*.rpm`; dnf pulls in
  the desktop libraries, and the app appears in the app menu with its icon.

### Changed
- Codex limit windows read naturally ("5 h", "7 dni", "30 dni") instead of minutes
  ("43200 min").
- ChatGPT's in-app help describes the Codex CLI route.
- Fedora aarch64 builds run on GitHub's native ARM runner instead of emulation.
- Rebuilding a release replaces its files instead of mixing old and new ones.

### Removed
- **Gemini** is removed completely: tracker, browser sign-in, settings and docs.
  Settings files from older versions that still mention Gemini load normally.

## 0.3.0 — 2026-11-27

### Fixed
- **Browser sign-in for Claude and Google works again.** The sign-in window was
  started with Chrome's remote debugging enabled, so Chrome reported itself as
  automated and Google refused the sign-in (including "Continue with Google" on
  claude.ai). Sign-in now opens an ordinary browser window.
- If the browser chosen in settings (for example Edge) is not installed, any
  installed Chrome, Chromium or Edge is used instead of failing.
- When claude.ai answers background reads with a Cloudflare check, the app says so
  instead of "Brak liczników".

### Added
- Claude limits can be read through Claude Code's sign-in, which avoids the
  Cloudflare check on claude.ai.
- New option in **Ustawienia**: show how many percent of the reference amount is
  **left** for currency-metered APIs (DeepSeek balance, OpenAI and Anthropic
  budgets), separate from the option for subscription limits. Over budget shows 0%.
- Release workflow building a Windows .exe, Fedora x86_64 and aarch64 packages and
  an x86_64 AppImage, published with SHA256 checksums.
