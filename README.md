# Wordfall

Wordfall (formerly SpeakType 2) is the cross-platform dictation app for macOS, Windows and Linux. It's developed alongside SpeakType 1, the macOS-only Swift app in [`macos-v1/`](macos-v1), and replaces it once everything in [ROADMAP.md](ROADMAP.md) is done.

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

Dictation matches the app it goes into (Settings → Tone that fits the app). The focused app is identified when recording starts, by bundle id on macOS, executable on Windows and window class on X11 (native Wayland apps can't be identified), and sorted into chat, email, terminal, code editor or other, with the user's own list checked before the built-in one in `src-tauri/src/tone.rs`. With AI cleanup on, the category adds a line to the model's instructions; without it, chat drops the final period and terminals lose the capital letter and trailing punctuation. In code editors and terminals, `src-tauri/src/coding.rs` writes spoken code as code: "main dot rs" → `main.rs`, "camel case get user name" → `getUserName`, "dash dash verbose" → `--verbose` (terminals only). The app is looked up on a thread of its own while recording, so a slow answer (macOS Accessibility can take up to half a second) never holds up the hotkey.

Command mode edits selected text by voice. Set a shortcut in Settings → Shortcuts, select text in any app, hold the shortcut and say what to change ("make this more formal", "translate to German"). Wordfall copies the selection, sends it with the instruction to the AI cleanup model, pastes the result over the selection and puts the clipboard back. It needs AI cleanup set up, and has its own shortcut rather than guessing from what was said, so a dictation that starts with "make" or "fix" never replaces the text around it. Single modifier keys aren't offered for it: the macOS listener for those holds one key, the dictation hotkey. Each shortcut only ends the recordings it started, so pressing one while the other is held never stops the other's recording (in Toggle mode either one stops a recording with a second press).

Copying the selection takes care with the app it goes to (`src-tauri/src/command.rs`, `src-tauri/src/paste.rs`):

- Terminals get Ctrl+Shift+C on Linux and Ctrl+Insert on Windows, as Ctrl+C would stop the running program; macOS uses ⌘C everywhere. Native Wayland sessions don't say which app has focus, so a terminal can't be told apart there, and command mode says so in the pill rather than risk Ctrl+C.
- With nothing selected, VS Code and JetBrains IDEs copy the whole current line. In code editors, a copy ending in a line break is taken as no selection ("Select some text first"), which also turns away selections of whole lines down to the next one; select up to the end of the last line instead.
- Text and images on the clipboard are put back afterwards. Anything else, like copied files, can't be, so it isn't cleared before copying: copying with nothing selected leaves it in place, while copying a selection replaces it, as Ctrl+C would.

Settings → Commands and formatting turns on rules for English dictation, all on by default: "scratch that", "new line" and "new paragraph" as commands, keeping only the correction in "meet at 3, actually 4", and writing spoken lists, numbers, dates, times, prices and percentages as you'd type them. They live in `src-tauri/src/spoken/` and leave the text alone when unsure. Voice commands apply to dictation pasted when you finish, not to live typing, where "new line" would press Return.

## Meetings

The Meetings screen records a call and shows its transcript as people talk. It records the microphone and, where the system allows, what the computer plays, as two tracks: the transcript labels them "You" and "Others" (a cheap speaker split, not voice recognition). Each track is written to a WAV file as it's recorded and transcribed in 15–30 second windows cut at pauses, so memory stays flat in long meetings. When a meeting ends, the tracks are mixed into one recording beside the transcript, in `meetings/<id>/` next to History. If AI cleanup is set up (built-in model or local server), it writes a summary, action items and open questions, in parts for long meetings. Notes and transcript copy or save as Markdown.

Recording what the computer plays depends on the system:

- **Windows:** WASAPI loopback of any output, through cpal. Nothing extra to install.
- **macOS 14.2 or later:** a Core Audio process tap on an output, through cpal. macOS asks for permission to record system audio the first time. An output that's also a microphone (AirPods, most headsets) can't be tapped this way; choose the Mac's speakers or a virtual device. On macOS 13, route the call through a virtual device such as [BlackHole](https://github.com/ExistentialAudio/BlackHole) and choose it as the source.
- **Linux:** the output's PulseAudio or PipeWire monitor, recorded with `parec` (in `pulseaudio-utils`; PipeWire serves it through `pipewire-pulse`). Any monitor listed by `pactl` can be chosen. Without `parec`, choose an input device that carries the output, or record the microphone only.

Without system audio, the meeting is recorded from the microphone alone and the transcript isn't split by speaker. Through speakers rather than headphones, the microphone also hears the other side; microphone audio quieter than the system audio at the same moment is dropped so it isn't transcribed twice, which can also drop your words when you talk over someone.

To preview the screens in the browser, open http://localhost:1420/?meeting=recording for a meeting in progress, or add `?notes=0` to see Meetings without a cleanup model.

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
