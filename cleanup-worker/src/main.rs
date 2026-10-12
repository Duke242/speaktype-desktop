//! SpeakType's cleanup helper: runs a local language model through llama.cpp.
//!
//! The app starts this program when transcript cleanup is on, and talks to it
//! over stdin and stdout, one JSON object per line:
//!
//! - request: `{"model": "/path/model.gguf", "messages": [{"role": "system", "content": "…"}, …], "maxTokens": 512}`
//! - reply: `{"text": "…"}` or `{"error": "…"}`
//!
//! A request without messages only loads the model, so the first dictation
//! doesn't wait for it. The model stays loaded until another one is asked for or
//! stdin closes. Logs go to stderr.

use std::{
    io::{self, BufRead, Write},
    num::NonZeroU32,
    path::{Path, PathBuf},
};

use llama_cpp_2::{
    context::params::LlamaContextParams,
    llama_backend::LlamaBackend,
    llama_batch::LlamaBatch,
    model::{LlamaChatMessage, LlamaChatTemplate, LlamaModel, params::LlamaModelParams},
    sampling::LlamaSampler,
};
use serde::{Deserialize, Serialize};

/// Room for the prompt plus the reply. Dictations are short; longer ones are
/// refused by the app before they get here.
const MAX_CONTEXT: u32 = 8192;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Request {
    model: PathBuf,
    #[serde(default)]
    messages: Vec<Message>,
    #[serde(default = "default_max_tokens")]
    max_tokens: u32,
}

fn default_max_tokens() -> u32 {
    512
}

#[derive(Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Reply {
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

struct Loaded {
    path: PathBuf,
    model: LlamaModel,
}

fn main() {
    let mut backend = match LlamaBackend::init() {
        Ok(backend) => backend,
        Err(e) => {
            eprintln!("[cleanup] couldn't start llama.cpp: {e}");
            std::process::exit(1);
        }
    };
    if std::env::var_os("SPEAKTYPE_CLEANUP_LOGS").is_none() {
        backend.void_logs();
    }

    let mut loaded: Option<Loaded> = None;
    let mut stdout = io::stdout().lock();
    for line in io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Request>(&line) {
            Ok(request) => handle(&backend, &mut loaded, &request),
            Err(e) => Err(format!("Unreadable request: {e}")),
        };
        let reply = match reply {
            Ok(text) => Reply {
                text: Some(text),
                error: None,
            },
            Err(error) => Reply {
                text: None,
                error: Some(error),
            },
        };
        let written = serde_json::to_writer(&mut stdout, &reply)
            .map_err(io::Error::other)
            .and_then(|()| stdout.write_all(b"\n"))
            .and_then(|()| stdout.flush());
        if written.is_err() {
            // The app has gone away.
            break;
        }
    }
}

fn handle(
    backend: &LlamaBackend,
    loaded: &mut Option<Loaded>,
    request: &Request,
) -> Result<String, String> {
    let model = load(backend, loaded, &request.model)?;
    if request.messages.is_empty() {
        return Ok(String::new());
    }
    generate(backend, model, &request.messages, request.max_tokens)
}

/// The model at `path`, loading it unless it's the one already in memory.
fn load<'a>(
    backend: &LlamaBackend,
    loaded: &'a mut Option<Loaded>,
    path: &Path,
) -> Result<&'a LlamaModel, String> {
    if loaded.as_ref().is_none_or(|l| l.path != path) {
        // Free the old model before loading the new one, so both are never in memory.
        *loaded = None;
        if !path.is_file() {
            return Err(format!("The cleanup model is missing: {}", path.display()));
        }
        let model = LlamaModel::load_from_file(backend, path, &LlamaModelParams::default())
            .map_err(|e| format!("Couldn't load the cleanup model: {e}"))?;
        *loaded = Some(Loaded {
            path: path.to_path_buf(),
            model,
        });
    }
    Ok(&loaded.as_ref().expect("loaded above").model)
}

fn generate(
    backend: &LlamaBackend,
    model: &LlamaModel,
    messages: &[Message],
    max_tokens: u32,
) -> Result<String, String> {
    let chat = messages
        .iter()
        .map(|m| LlamaChatMessage::new(m.role.clone(), m.content.clone()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    // Every catalog model has its template built in; ChatML is the common fallback.
    let template = model.chat_template(None).or_else(|_| {
        LlamaChatTemplate::new("chatml").map_err(|e| format!("Bad chat template: {e}"))
    })?;
    let prompt = model
        .apply_chat_template(&template, &chat, true)
        .map_err(|e| format!("Couldn't build the prompt: {e}"))?;

    let vocab = model.vocab();
    let mut tokens = vocab.tokenize(prompt.as_bytes(), true, true);
    // Templates that write the start token themselves would otherwise get two.
    if tokens.len() > 1 && tokens[0] == vocab.bos() && tokens[1] == vocab.bos() {
        tokens.remove(0);
    }
    let prompt_len = u32::try_from(tokens.len()).map_err(|e| e.to_string())?;
    let n_ctx = (prompt_len + max_tokens)
        .min(MAX_CONTEXT)
        .min(model.n_ctx_train());
    if prompt_len >= n_ctx {
        return Err("The transcript is too long to clean up".into());
    }

    let threads = thread_count();
    let params = LlamaContextParams::default()
        .with_n_ctx(NonZeroU32::new(n_ctx))
        .with_n_batch(n_ctx)
        .with_n_threads(threads)
        .with_n_threads_batch(threads);
    let mut ctx = model
        .new_context(backend, params)
        .map_err(|e| format!("Couldn't prepare the model: {e}"))?;

    let mut batch = LlamaBatch::new(tokens.len().max(1), 1);
    let last = tokens.len() - 1;
    for (i, token) in tokens.iter().enumerate() {
        batch
            .add(*token, i as i32, &[0], i == last)
            .map_err(|e| e.to_string())?;
    }
    ctx.decode(&mut batch).map_err(|e| e.to_string())?;

    // Greedy: cleanup should be the same every time, not creative.
    let mut sampler = LlamaSampler::greedy();
    let mut output = Vec::new();
    let mut position = prompt_len as i32;
    while (position as u32) < n_ctx {
        let token = sampler.sample(&ctx, batch.n_tokens() - 1);
        if vocab.is_eog(token) {
            break;
        }
        vocab.token_to_piece_into(token, &mut output, false, None);
        batch.clear();
        batch
            .add(token, position, &[0], true)
            .map_err(|e| e.to_string())?;
        ctx.decode(&mut batch).map_err(|e| e.to_string())?;
        position += 1;
    }
    Ok(String::from_utf8_lossy(&output).into_owned())
}

/// llama.cpp gains little past 8 threads, and more would starve the rest of the system.
fn thread_count() -> i32 {
    std::thread::available_parallelism()
        .map(|n| n.get().min(8) as i32)
        .unwrap_or(4)
}
