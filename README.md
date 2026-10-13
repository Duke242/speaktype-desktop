# SpeakType 2

The cross-platform SpeakType app for macOS, Windows and Linux. It's developed alongside SpeakType 1, the macOS-only Swift app, and replaces it once everything in [ROADMAP.md](ROADMAP.md) is done.

## Develop

Requires Node 22+, a current stable Rust toolchain and CMake. On Linux, also install the system libraries listed in `.github/actions/setup-desktop-build/action.yml`.

```sh
npm install
npm run app        # run with live reload
npm run app:build  # build installers into src-tauri/target/release/bundle, with the cleanup helper
```

To design screens without the native app, run `npm run dev` and open http://localhost:1420. The UI runs with sample data (add `?theme=dark` or `?onboarding=1` to the URL).
Open http://localhost:1420/pill.html to preview the recorder with a simulated speech waveform, including pauses.

Live typing is enabled in Settings → Behavior. It inserts short phrases during recording, at pauses or roughly every four seconds of continuous speech (plus model processing time). Use Fn in Hold mode on macOS, or Toggle mode with other shortcuts. Native Wayland sessions retain insertion on release. Escape stops further insertion and keeps the transcript in History; text already typed remains. Switching the active app pauses live insertion for that session. Live typing leaves the clipboard untouched.

## Check

```sh
npm run typecheck && npm test                 # UI
cd src-tauri && cargo fmt --check \
  && cargo clippy --all-targets -- -D warnings \
  && cargo test                               # app core
```

Benchmark a downloaded model on an audio file:

```sh
cd src-tauri
cargo run --release --example transcribe_wav -- parakeet-tdt-v3 "<models folder>" clip.wav
```

AI cleanup's built-in models run in a separate helper, [`cleanup-worker/`](cleanup-worker/README.md). `npm run app` finds it once you've run `cargo build --release` in that folder; without it, cleanup works through a local server only.

## Release

```sh
npm run release          # tags the next alpha, e.g. v2.0.0-alpha.5, and pushes it
npm run release -- beta  # or the next beta
```

Releasing needs the [GitHub CLI](https://cli.github.com), signed in with `gh auth login`. The script creates the release on GitHub, and CI attaches the installers to it.

Every push to `main` and every pull request runs `.github/workflows/desktop.yml`:
- type checks, tests and a production build of the UI
- format, lint and tests of the app core on macOS, Windows and Linux
- a dependency audit
- installers for all three systems, uploaded as workflow artifacts

Pushing a `v2.*` tag builds the installers straight away, since the tagged commit has already been checked, and publishes them as a GitHub pre-release. SpeakType 1 stays the "Latest" release, so its update check and download links are unaffected. Every tag is a rollback point.

## Layout

- `src/` UI. Colors, radii, shadows and type styles all come from `src/styles/globals.css`.
- `src-tauri/src/` app core: audio, transcription engines, dictation, paste, history, models, cleanup.
- `cleanup-worker/` the helper program that runs built-in cleanup models with llama.cpp.
- `src-tauri/src/platform/{macos,windows,linux}/` everything OS-specific, one folder per OS with the same functions.
