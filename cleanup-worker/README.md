# wordflow-cleanup

The helper program behind AI cleanup's built-in models. WordFlow starts it when cleanup is on, sends it each transcript, and pastes the reply. It runs a GGUF language model with [llama.cpp](https://github.com/ggml-org/llama.cpp).

## Why it's a separate program

The app already contains whisper.cpp for speech. whisper.cpp and llama.cpp each bundle their own copy of ggml, the library underneath both, and the copies are different versions. Linked into one binary, the linker keeps one copy's symbols and drops the other's without an error, so one of the two engines would run against code it wasn't built for. In a separate process each has its own ggml.

It has other benefits:

- If the model crashes or hangs, dictation carries on: the app stops the helper and pastes the transcript as it was.
- The model's memory is given back when cleanup isn't used: the helper quits after 10 minutes idle.

## How the app talks to it

One JSON object per line, on stdin and stdout:

```
→ {"model": "/path/model.gguf", "messages": [{"role": "system", "content": "…"}, {"role": "user", "content": "…"}], "maxTokens": 128}
← {"text": "…"}
← {"error": "…"}
```

A request without `messages` only loads the model. The prompt, the checks on the reply, starting and stopping the helper, and timeouts are all in `src-tauri/src/cleanup.rs`. This program only runs the model.

Set `WORDFLOW_CLEANUP_LOGS=1` to see llama.cpp's own logs on stderr.

## Building

```sh
cargo build --release
```

While developing, `npm run app` finds the helper in `target/release` (or `target/debug`). For installers, `npm run worker:build` copies it to `src-tauri/binaries/` under the name Tauri expects, and `src-tauri/tauri.cleanup.conf.json` adds it to the bundle, next to the app's own executable. CI does the same before building each installer.

On Apple Silicon it uses Metal. Elsewhere it runs on the CPU.
