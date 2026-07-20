# AGENTS.md

Rules for any agent (Claude Code or other) working in this monorepo.

## The codebase is English-only

Comments, doc comments, docstrings, log messages, and user-facing UI
text (labels, toasts, error messages) are all in English, no exceptions
- this was a deliberate one-time migration (everything used to be in
Portuguese). Don't reintroduce Portuguese in new code, even though
conversations with the maintainer happen in Portuguese.

## Frontend calls to external services never block or throw

Any TypeScript code that calls an external API (SteamGridDB, the Steam
CDN, etc., see `gameArtwork.ts`) must be best-effort: a failed fetch, a
missing match, or the API itself rejecting must never throw past that
call site nor stop whatever loop/sync it's part of (see
`gameArtwork.ts`'s `applyArtwork`/`applyAll`, and `gameSync.ts`'s
`applyArtworkSafely` wrapping it again just in case). Report failures via
`logFrontendError` (`api.ts` -> `main.py:log_frontend_error` ->
`decky.logger`), not `console.error` - the frontend's console only
reaches the Steam WebHelper's own devtools, never the plugin log the
"Logs" tab in Settings actually reads.

## The Runner (Rust) controls everything that touches the host

Any control over the HOST operating system (screens/monitors via
kscreen-doctor, cursor, processes, whatever else comes up) goes through
the MoonProfile Runner (Rust, `moon_profile_runner/`), never through
Apollo (which only connects and runs the `cmd`, no prep-cmd at all, see
`moon_profile_decky/py_modules/moonprofile_core.py`) nor a loose script
somewhere else. The Deck sends WHAT to do (already-resolved commands,
e.g. `build_display_commands`/`build_restore_commands` in
`moonprofile_core.py`); the Runner is the one that actually RUNS it on
the host. This centralizes host control in one place, testable and
logged, instead of spread across Apollo/Deck/loose scripts.

## Every new feature needs an automated test

When creating or changing a feature, add (or update) the tests that
cover the new/changed behavior **in the same work session**, not later.

Prefer testing real behavior over mocking: we've already found real bugs
this way (see `moon_profile_runner/src-tauri/src/server.rs`, tests that
spawn a real process instead of simulating `sysinfo`, which caught both
a process-refresh bug and a shared-prefix match bug, neither of which a
mock would have caught).

## moon_profile_runner/ (Tauri/Rust)

- Tests physically live in `src-tauri/src/tests/<module>.rs` (e.g.
  `src/tests/server.rs` tests `src/server.rs`), but stay logically
  inside the module under test: each production file just has one
  declaration, `#[cfg(test)] #[path = "tests/<module>.rs"] mod tests;`,
  at the end. The `#[path]` attribute only changes WHERE the file lives,
  not its position in the module tree, so `use super::*;` inside the
  test file still sees the parent module's private items normally. Goal:
  separate production code from test code without needing Cargo's
  integration `tests/` directory (which only sees the public API, and
  most tests here exercise private functions).
- The shared test helper (`FakeGameProcess`) lives in
  `src/tests/support.rs`, declared in `lib.rs` as `#[cfg(test)] #[path =
  "tests/support.rs"] mod test_support;` (same pattern).
- Run with `cargo test` (inside `moon_profile_runner/src-tauri/`).
- For HTTP endpoints: use `tower::ServiceExt::oneshot` directly on the
  `Router` (no need to open a real TCP port, fast and no port conflicts
  between runs).
- For logic that depends on the OS (process detection, etc.): prefer
  spawning a real test process/resource instead of mocking the OS API,
  that's how the real bugs here were found.
- Every "pure" helper function (no I/O) also gets its own fast unit test
  separate from the integration test (e.g. `cmd_arg_matches_app_id_cases`).
- `install.sh` (manual/local install) and `packaging/PKGBUILD` (AUR) both
  independently install the same things (binary, applications menu
  entry, systemd `--user` unit) - there's no shared source of truth
  between them. Whenever the install/autostart mechanism changes, update
  BOTH (plus `packaging/moon-profile-runner-git.install`'s message if
  relevant) in the same change, not just the one you happen to be
  testing - a real bug shipped this session (stale autostart file, then
  a missing systemd unit install) came from exactly this drift.

## moon_profile_decky/ (Decky plugin - TypeScript + Python)

### Frontend (TypeScript)

- Harness: `vitest`. Run with `npm run test` (or `npx vitest run`) inside
  `moon_profile_decky/`.
- Tests live in `tests/*.test.ts`, plain Node environment (not jsdom, the
  code under test only touches the surface of globals that Steam injects
  into `window`, e.g. `SteamClient`, `appStore`, `collectionStore`; it
  never renders real DOM). `tests/setup.ts` ensures `window` exists in
  Node before any test runs.
- `SteamClient`/`appStore`/`collectionStore` aren't typed by `@decky/ui`
  (undocumented API), mock them directly on `window` with `vi.fn()` per
  test, without depending on a real Steam client.
- Modules that call `callable(...)` from `@decky/api` (see `src/api.ts`)
  mock the whole module via `vi.mock("../src/api", ...)` in the test (see
  `tests/gameCollection.test.ts` for the pattern).
- `npm run build` (`tsc` via rollup) remains the real type check,
  running `tsc --noEmit` on its own at the root fails because of
  pre-existing project configs (react-router/JSX namespace), it's not
  informative on its own.

### Backend (Python)

- Harness: `pytest` (+ `pytest-asyncio`, `asyncio_mode = auto` via
  `pytest.ini`). Isolated environment in `.venv/` (not versioned), create
  it with `python3 -m venv .venv && .venv/bin/pip install -r
  requirements-dev.txt`, run with `.venv/bin/python -m pytest tests/`.
- `main.py` only exists inside the real Decky Loader runtime (which
  injects `py_modules/` into `sys.path` and a global `decky` module with
  directories/logger), `tests/conftest.py` recreates this artificially
  (the `plugin_module` fixture) by importing `main.py` with a fake
  `decky` module pointing to a temporary folder isolated per test, never
  the user's real config.
- `moonprofile_core.py` (logic shared with `runner.py`) is tested
  directly, without needing the fake `decky`, it's pure/stdlib code.
- Functions that depend on the OS (e.g. `detect_context`, which reads
  `/sys/class/drm`) take the path as a parameter (defaulting to the real
  path) specifically to allow testing against a fixture instead of the
  real hardware of the machine running the test.
