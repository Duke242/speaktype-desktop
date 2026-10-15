//! Sharing the dictionary and snippets between computers, or with a team.
//!
//! There's no server. WordFlow keeps `wordflow-shared.json` in a folder the
//! user picks, usually one that Dropbox, iCloud Drive or a shared drive keeps
//! in step. Every rule and snippet in it carries the time it last changed, and
//! deletions stay as markers for a while, so each copy of WordFlow merges what
//! it finds by keeping the newest version of each entry. Copies that read each
//! other's files end up with the same entries, whatever order they sync in.
//!
//! Rules and snippets are matched by their trigger, ignoring case. The same
//! file format is used to export and import them.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{
        Mutex, OnceLock,
        mpsc::{self, RecvTimeoutError, Sender},
    },
    time::Duration,
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::DialogExt;

use crate::{
    AppState, LockExt, history::now_ms, snippets::Snippet, text::DictionaryEntry, tone::AppCategory,
};

/// Keeps the name from before the rename to WordFlow, so shared folders keep syncing.
pub const FILE_NAME: &str = "wordflow-shared.json";
/// The shared file's name from before the rename to WordFlow.
const OLD_FILE_NAME: &str = "wordfall-shared.json";
/// What was last synced, so changes made here can be told from changes made elsewhere.
const STATE_FILE: &str = "sync-state.json";
const FORMAT_VERSION: u32 = 1;
/// How often the shared file is checked for changes from elsewhere.
const INTERVAL: Duration = Duration::from_secs(60);
/// Deletion markers are dropped after this long. A copy that was offline for
/// longer may bring a deleted entry back.
const KEEP_DELETIONS_MS: u64 = 90 * 86_400_000;

/// A dictionary rule as shared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub trigger: String,
    pub replacement: String,
    #[serde(default = "yes")]
    pub is_enabled: bool,
    #[serde(default = "yes")]
    pub match_whole_word: bool,
    #[serde(default)]
    pub learned: bool,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "crate::text::app_categories"
    )]
    pub only_in: Vec<AppCategory>,
}

/// A snippet as shared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedSnippet {
    pub trigger: String,
    pub expansion: String,
    #[serde(default = "yes")]
    pub is_enabled: bool,
}

fn yes() -> bool {
    true
}

/// One entry and when it last changed. A deleted entry keeps its trigger, so
/// the deletion reaches the other copies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record<T> {
    #[serde(flatten)]
    pub item: T,
    /// Milliseconds since 1970.
    pub modified_at: u64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deleted: bool,
}

/// The contents of `wordflow-shared.json` and of exported files.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Library {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub dictionary: Vec<Record<Rule>>,
    #[serde(default)]
    pub snippets: Vec<Record<SharedSnippet>>,
}

/// Something shared, matched across copies by its key.
trait Shared: Clone + PartialEq + Serialize {
    fn trigger(&self) -> &str;

    /// The trigger in lowercase, with spacing tidied.
    fn key(&self) -> String {
        self.trigger()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    }
}

impl Shared for Rule {
    fn trigger(&self) -> &str {
        &self.trigger
    }
}

impl Shared for SharedSnippet {
    fn trigger(&self) -> &str {
        &self.trigger
    }
}

/// How a local entry is shared.
trait Local: Clone {
    type Item: Shared;
    fn item(&self) -> Self::Item;
    fn update(&mut self, item: &Self::Item);
    fn create(item: &Self::Item) -> Self;
}

impl Local for DictionaryEntry {
    type Item = Rule;

    fn item(&self) -> Rule {
        Rule {
            trigger: self.trigger.clone(),
            replacement: self.replacement.clone(),
            is_enabled: self.is_enabled,
            match_whole_word: self.match_whole_word,
            learned: self.learned,
            only_in: self.only_in.clone(),
        }
    }

    fn update(&mut self, rule: &Rule) {
        self.trigger = rule.trigger.clone();
        self.replacement = rule.replacement.clone();
        self.is_enabled = rule.is_enabled;
        self.match_whole_word = rule.match_whole_word;
        self.learned = rule.learned;
        self.only_in = rule.only_in.clone();
    }

    fn create(rule: &Rule) -> Self {
        let mut entry = DictionaryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            trigger: String::new(),
            replacement: String::new(),
            is_enabled: true,
            match_whole_word: true,
            learned: false,
            only_in: Vec::new(),
        };
        entry.update(rule);
        entry
    }
}

impl Local for Snippet {
    type Item = SharedSnippet;

    fn item(&self) -> SharedSnippet {
        SharedSnippet {
            trigger: self.trigger.clone(),
            expansion: self.expansion.clone(),
            is_enabled: self.is_enabled,
        }
    }

    fn update(&mut self, snippet: &SharedSnippet) {
        self.trigger = snippet.trigger.clone();
        self.expansion = snippet.expansion.clone();
        self.is_enabled = snippet.is_enabled;
    }

    fn create(snippet: &SharedSnippet) -> Self {
        let mut created = Snippet {
            id: uuid::Uuid::new_v4().to_string(),
            trigger: String::new(),
            expansion: String::new(),
            is_enabled: true,
        };
        created.update(snippet);
        created
    }
}

/// Dates the entries here against what was last synced: unchanged ones keep
/// their time, new or edited ones get `now`, and ones that are gone become
/// deletions at `now`. Before the first sync nothing is known about them, so
/// they get time 0 and lose to any version already shared.
fn stamp<T: Shared>(local: &[T], last: Option<&[Record<T>]>, now: u64) -> Vec<Record<T>> {
    let previous: HashMap<String, &Record<T>> = last
        .unwrap_or_default()
        .iter()
        .map(|r| (r.item.key(), r))
        .collect();
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in local {
        let key = item.key();
        if key.is_empty() || !seen.insert(key.clone()) {
            continue;
        }
        let modified_at = match previous.get(&key) {
            Some(r) if !r.deleted && r.item == *item => r.modified_at,
            _ if last.is_none() => 0,
            _ => now,
        };
        out.push(Record {
            item: item.clone(),
            modified_at,
            deleted: false,
        });
    }
    for (key, record) in previous {
        if !seen.contains(&key) {
            out.push(Record {
                deleted: true,
                modified_at: if record.deleted {
                    record.modified_at
                } else {
                    now
                },
                item: record.item.clone(),
            });
        }
    }
    out
}

/// Whether `a` should replace `b`: it's newer, or as new and wins a fixed
/// tie-break, so every copy picks the same one.
fn wins<T: Shared>(a: &Record<T>, b: &Record<T>) -> bool {
    let content = |r: &Record<T>| serde_json::to_string(&r.item).unwrap_or_default();
    (a.modified_at, a.deleted, content(a)) > (b.modified_at, b.deleted, content(b))
}

/// The newest record of each entry, sorted by trigger. Old deletions are dropped.
fn merge<T: Shared>(a: &[Record<T>], b: &[Record<T>], now: u64) -> Vec<Record<T>> {
    let mut newest: BTreeMap<String, &Record<T>> = BTreeMap::new();
    for record in a.iter().chain(b) {
        let key = record.item.key();
        if key.is_empty() {
            continue;
        }
        if newest.get(&key).is_none_or(|current| wins(record, current)) {
            newest.insert(key, record);
        }
    }
    newest
        .into_values()
        .filter(|r| !(r.deleted && now.saturating_sub(r.modified_at) > KEEP_DELETIONS_MS))
        .cloned()
        .collect()
}

/// The local list after a merge: entries keep their place and id, deleted
/// ones go, and new ones are added at the end.
fn apply<L: Local>(local: &[L], merged: &[Record<L::Item>]) -> Vec<L> {
    let live: HashMap<String, &L::Item> = merged
        .iter()
        .filter(|r| !r.deleted)
        .map(|r| (r.item.key(), &r.item))
        .collect();
    let mut placed = HashSet::new();
    let mut out = Vec::new();
    for entry in local {
        let key = entry.item().key();
        if key.is_empty() {
            // Not shared, but kept.
            out.push(entry.clone());
        } else if let Some(item) = live.get(&key) {
            let mut entry = entry.clone();
            // A second entry with the same trigger stays as it is.
            if placed.insert(key) {
                entry.update(item);
            }
            out.push(entry);
        }
    }
    for record in merged {
        if !record.deleted && !placed.contains(&record.item.key()) {
            out.push(L::create(&record.item));
        }
    }
    out
}

fn sync_list<L: Local>(
    local: &[L],
    last: Option<&[Record<L::Item>]>,
    shared: &[Record<L::Item>],
    now: u64,
) -> (Vec<L>, Vec<Record<L::Item>>) {
    let items: Vec<L::Item> = local.iter().map(Local::item).collect();
    let merged = merge(&stamp(&items, last, now), shared, now);
    (apply(local, &merged), merged)
}

/// The result of merging this copy's entries with the shared file.
#[derive(Debug)]
struct Merged {
    dictionary: Vec<DictionaryEntry>,
    snippets: Vec<Snippet>,
    library: Library,
}

/// Merges local entries with the shared library. `last` is the library as it
/// was after the previous sync with this folder, if there was one.
fn reconcile(
    dictionary: &[DictionaryEntry],
    snippets: &[Snippet],
    last: Option<&Library>,
    shared: &Library,
    now: u64,
) -> Merged {
    let (dictionary, rules) = sync_list(
        dictionary,
        last.map(|l| l.dictionary.as_slice()),
        &shared.dictionary,
        now,
    );
    let (snippets, shared_snippets) = sync_list(
        snippets,
        last.map(|l| l.snippets.as_slice()),
        &shared.snippets,
        now,
    );
    Merged {
        dictionary,
        snippets,
        library: Library {
            version: FORMAT_VERSION,
            dictionary: rules,
            snippets: shared_snippets,
        },
    }
}

/// Entries to write to an export file.
fn export_library(dictionary: &[DictionaryEntry], snippets: &[Snippet], now: u64) -> Library {
    Library {
        version: FORMAT_VERSION,
        dictionary: current(dictionary, now),
        snippets: current(snippets, now),
    }
}

fn current<L: Local>(local: &[L], now: u64) -> Vec<Record<L::Item>> {
    local
        .iter()
        .map(|entry| Record {
            item: entry.item(),
            modified_at: now,
            deleted: false,
        })
        .collect()
}

/// Adds imported entries, or updates the ones with the same trigger. Returns
/// how many were added or changed.
fn import_into<L: Local>(local: &mut Vec<L>, imported: &[Record<L::Item>]) -> usize {
    let mut changed = 0;
    for record in imported.iter().filter(|r| !r.deleted) {
        let key = record.item.key();
        if key.is_empty() {
            continue;
        }
        match local.iter_mut().find(|e| e.item().key() == key) {
            Some(entry) if entry.item() == record.item => {}
            Some(entry) => {
                entry.update(&record.item);
                changed += 1;
            }
            None => {
                local.push(L::create(&record.item));
                changed += 1;
            }
        }
    }
    changed
}

// ---- Files ----

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, String> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| format!("Couldn't read {}: {e}", file_name(path))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("Couldn't open {}: {e}", file_name(path))),
    }
}

/// Writes to a temporary file next to `path` and renames it into place, so a
/// sync service never picks up a half-written file.
fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    let result = fs::write(&tmp, json).and_then(|()| fs::rename(&tmp, path));
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result.map_err(|e| format!("Couldn't save {}: {e}", file_name(path)))
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// What was last synced, and with which folder.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncState {
    folder: String,
    library: Library,
}

// ---- Running ----

/// How sharing went, for Settings.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub last_synced_at: Option<u64>,
    pub error: Option<String>,
}

static STATUS: Mutex<Status> = Mutex::new(Status {
    last_synced_at: None,
    error: None,
});
/// Held for a whole sync, so two never overlap.
static SYNCING: Mutex<()> = Mutex::new(());
static NUDGE: OnceLock<Sender<()>> = OnceLock::new();

pub fn status() -> Status {
    STATUS.lock_unpoisoned().clone()
}

/// Syncs now, then every minute and soon after each change. Does nothing
/// while no folder is chosen.
pub fn start(app: &AppHandle) {
    let (tx, rx) = mpsc::channel();
    if NUDGE.set(tx).is_err() {
        return;
    }
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("sync".into())
        .spawn(move || {
            loop {
                sync_now(&app);
                match rx.recv_timeout(INTERVAL) {
                    // Let a burst of changes settle into one sync.
                    Ok(()) => {
                        std::thread::sleep(Duration::from_millis(500));
                        while rx.try_recv().is_ok() {}
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }
        });
    if let Err(e) = spawned {
        eprintln!("[sync] couldn't start: {e}");
    }
}

/// Asks for a sync soon, after settings change.
pub fn nudge() {
    if let Some(tx) = NUDGE.get() {
        let _ = tx.send(());
    }
}

/// Syncs and records how it went.
pub fn sync_now(app: &AppHandle) -> Status {
    let result = sync(app);
    let mut status = STATUS.lock_unpoisoned();
    match result {
        Ok(true) => {
            status.last_synced_at = Some(now_ms());
            status.error = None;
        }
        Ok(false) => status.error = None,
        Err(e) => {
            eprintln!("[sync] {e}");
            status.error = Some(e);
        }
    }
    status.clone()
}

/// Returns whether there was a folder to sync with.
fn sync(app: &AppHandle) -> Result<bool, String> {
    let _syncing = SYNCING.lock_unpoisoned();
    let state = app.state::<AppState>();
    let settings = state.settings();
    if settings.sync_folder.is_empty() {
        return Ok(false);
    }
    let folder = PathBuf::from(&settings.sync_folder);
    if !folder.is_dir() {
        return Err("Couldn't find the shared folder. Is it still there?".into());
    }
    let shared_path = folder.join(FILE_NAME);
    let old_path = folder.join(OLD_FILE_NAME);
    if !shared_path.exists() && old_path.exists() {
        fs::rename(&old_path, &shared_path).map_err(|e| e.to_string())?;
    }
    // A file that can't be read is left alone rather than overwritten.
    let shared: Library = read_json(&shared_path)?.unwrap_or_default();
    let state_path = app
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?
        .join(STATE_FILE);
    let last = read_json::<SyncState>(&state_path)
        .unwrap_or_default()
        .filter(|s| s.folder == settings.sync_folder)
        .map(|s| s.library);
    let now = now_ms();

    let merged = reconcile(
        &settings.dictionary,
        &settings.snippets,
        last.as_ref(),
        &shared,
        now,
    );
    let changed_here =
        merged.dictionary != settings.dictionary || merged.snippets != settings.snippets;
    let library = if changed_here {
        // Merged again with the latest settings, in case they just changed.
        let merged = state.update_settings(|current| {
            let merged = reconcile(
                &current.dictionary,
                &current.snippets,
                last.as_ref(),
                &shared,
                now,
            );
            current.dictionary = merged.dictionary;
            current.snippets = merged.snippets;
            merged.library
        })?;
        let _ = app.emit("settings-changed", ());
        merged
    } else {
        merged.library
    };

    if library != shared {
        write_json(&shared_path, &library)?;
    }
    if last.as_ref() != Some(&library) {
        let saved = SyncState {
            folder: settings.sync_folder,
            library,
        };
        write_json(&state_path, &saved)?;
    }
    Ok(true)
}

// ---- Choosing files ----

/// Asks for the folder to share through. `None` if the user cancelled.
pub fn pick_folder(app: &AppHandle) -> Option<PathBuf> {
    app.dialog()
        .file()
        .set_title("Choose a folder to share through")
        .blocking_pick_folder()
        .and_then(|p| p.into_path().ok())
}

/// Saves the dictionary and snippets to a file the user picks. Returns its
/// path, or `None` if they cancelled.
pub fn export(app: &AppHandle) -> Result<Option<PathBuf>, String> {
    let Some(path) = app
        .dialog()
        .file()
        .add_filter("WordFlow library", &["json"])
        .set_file_name("wordflow-library.json")
        .blocking_save_file()
        .and_then(|p| p.into_path().ok())
    else {
        return Ok(None);
    };
    let settings = app.state::<AppState>().settings();
    write_json(
        &path,
        &export_library(&settings.dictionary, &settings.snippets, now_ms()),
    )?;
    Ok(Some(path))
}

/// How many rules and snippets an import added or changed.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub rules: usize,
    pub snippets: usize,
}

/// Adds the rules and snippets from a file the user picks. `None` if they cancelled.
pub fn import(app: &AppHandle) -> Result<Option<ImportSummary>, String> {
    let Some(path) = app
        .dialog()
        .file()
        .add_filter("WordFlow library", &["json"])
        .blocking_pick_file()
        .and_then(|p| p.into_path().ok())
    else {
        return Ok(None);
    };
    let library: Library = read_json(&path)?.ok_or("That file is gone")?;
    let summary = app
        .state::<AppState>()
        .update_settings(|settings| ImportSummary {
            rules: import_into(&mut settings.dictionary, &library.dictionary),
            snippets: import_into(&mut settings.snippets, &library.snippets),
        })?;
    let _ = app.emit("settings-changed", ());
    nudge();
    Ok(Some(summary))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(trigger: &str, replacement: &str) -> DictionaryEntry {
        DictionaryEntry {
            id: format!("id-{trigger}"),
            trigger: trigger.into(),
            replacement: replacement.into(),
            is_enabled: true,
            match_whole_word: true,
            learned: false,
            only_in: Vec::new(),
        }
    }

    fn snippet(trigger: &str, expansion: &str) -> Snippet {
        Snippet {
            id: format!("id-{trigger}"),
            trigger: trigger.into(),
            expansion: expansion.into(),
            is_enabled: true,
        }
    }

    /// One computer: its entries and what it last synced.
    #[derive(Default)]
    struct Copy {
        dictionary: Vec<DictionaryEntry>,
        snippets: Vec<Snippet>,
        last: Option<Library>,
    }

    impl Copy {
        fn sync(&mut self, shared: &mut Library, now: u64) {
            let merged = reconcile(
                &self.dictionary,
                &self.snippets,
                self.last.as_ref(),
                shared,
                now,
            );
            self.dictionary = merged.dictionary;
            self.snippets = merged.snippets;
            *shared = merged.library.clone();
            self.last = Some(merged.library);
        }

        fn rules(&self) -> Vec<(&str, &str)> {
            let mut rules: Vec<_> = self
                .dictionary
                .iter()
                .map(|e| (e.trigger.as_str(), e.replacement.as_str()))
                .collect();
            rules.sort();
            rules
        }
    }

    #[test]
    fn first_sync_shares_everything_and_keeps_ids() {
        let mut shared = Library::default();
        let mut laptop = Copy {
            dictionary: vec![entry("word flow", "WordFlow")],
            snippets: vec![snippet("my email", "me@example.com")],
            ..Copy::default()
        };
        laptop.sync(&mut shared, 1_000);
        assert_eq!(shared.dictionary.len(), 1);
        assert_eq!(shared.snippets[0].item.expansion, "me@example.com");
        assert_eq!(laptop.dictionary[0].id, "id-word flow");

        let mut desktop = Copy::default();
        desktop.sync(&mut shared, 2_000);
        assert_eq!(desktop.rules(), [("word flow", "WordFlow")]);
        assert_eq!(desktop.snippets[0].trigger, "my email");
    }

    #[test]
    fn edits_and_deletions_reach_the_other_copy() {
        let mut shared = Library::default();
        let mut laptop = Copy {
            dictionary: vec![entry("teh", "the"), entry("k8s", "Kubernetes")],
            ..Copy::default()
        };
        laptop.sync(&mut shared, 1_000);
        let mut desktop = Copy::default();
        desktop.sync(&mut shared, 1_500);

        // The laptop edits one rule and deletes the other.
        laptop.dictionary.retain(|e| e.trigger != "teh");
        laptop.dictionary[0].replacement = "K8s".into();
        laptop.sync(&mut shared, 2_000);
        assert!(
            shared
                .dictionary
                .iter()
                .any(|r| r.deleted && r.item.trigger == "teh")
        );

        desktop.sync(&mut shared, 3_000);
        assert_eq!(desktop.rules(), [("k8s", "K8s")]);
    }

    #[test]
    fn the_newest_edit_wins_whichever_syncs_first() {
        let mut shared = Library::default();
        let mut a = Copy {
            snippets: vec![snippet("sig", "Best, K")],
            ..Copy::default()
        };
        a.sync(&mut shared, 1_000);
        let mut b = Copy::default();
        b.sync(&mut shared, 1_100);

        // Both edit offline; b's edit is later but a syncs last.
        a.snippets[0].expansion = "Cheers, K".into();
        b.snippets[0].expansion = "Thanks, K".into();
        let b_merged = reconcile(&b.dictionary, &b.snippets, b.last.as_ref(), &shared, 3_000);
        shared = b_merged.library.clone();
        b.snippets = b_merged.snippets;
        b.last = Some(b_merged.library);
        a.sync(&mut shared, 2_000);
        b.sync(&mut shared, 4_000);

        assert_eq!(a.snippets[0].expansion, "Thanks, K");
        assert_eq!(b.snippets[0].expansion, "Thanks, K");
    }

    #[test]
    fn merging_converges_in_any_order() {
        let a = vec![
            Record {
                item: SharedSnippet {
                    trigger: "x".into(),
                    expansion: "1".into(),
                    is_enabled: true,
                },
                modified_at: 5,
                deleted: false,
            },
            Record {
                item: SharedSnippet {
                    trigger: "y".into(),
                    expansion: "2".into(),
                    is_enabled: true,
                },
                modified_at: 5,
                deleted: false,
            },
        ];
        let b = vec![
            Record {
                item: SharedSnippet {
                    trigger: "X".into(),
                    expansion: "3".into(),
                    is_enabled: true,
                },
                modified_at: 5,
                deleted: false,
            },
            Record {
                item: SharedSnippet {
                    trigger: "y".into(),
                    expansion: "2".into(),
                    is_enabled: true,
                },
                modified_at: 9,
                deleted: true,
            },
        ];
        let ab = merge(&a, &b, 10);
        assert_eq!(ab, merge(&b, &a, 10));
        assert_eq!(merge(&ab, &a, 10), ab, "merging again changes nothing");
        assert!(ab.iter().any(|r| r.item.key() == "y" && r.deleted));
        assert_eq!(ab.len(), 2);
    }

    #[test]
    fn a_new_copy_adopts_shared_versions_and_adds_its_own() {
        let mut shared = Library::default();
        let mut team = Copy {
            dictionary: vec![entry("acme", "ACME")],
            ..Copy::default()
        };
        team.sync(&mut shared, 1_000);
        // Edit it so it isn't time 0 any more.
        team.dictionary[0].replacement = "Acme Inc.".into();
        team.sync(&mut shared, 2_000);

        let mut newcomer = Copy {
            dictionary: vec![entry("ACME", "Acme"), entry("brb", "be right back")],
            ..Copy::default()
        };
        newcomer.sync(&mut shared, 3_000);
        assert_eq!(
            newcomer.rules(),
            [("acme", "Acme Inc."), ("brb", "be right back")]
        );
        team.sync(&mut shared, 4_000);
        assert_eq!(
            team.rules(),
            [("acme", "Acme Inc."), ("brb", "be right back")]
        );
    }

    #[test]
    fn deleting_then_adding_again_brings_it_back() {
        let mut shared = Library::default();
        let mut copy = Copy {
            dictionary: vec![entry("a", "1")],
            ..Copy::default()
        };
        copy.sync(&mut shared, 1_000);
        copy.dictionary.clear();
        copy.sync(&mut shared, 2_000);
        assert!(copy.dictionary.is_empty());
        copy.dictionary.push(entry("a", "2"));
        copy.sync(&mut shared, 3_000);
        assert_eq!(copy.rules(), [("a", "2")]);
        assert!(!shared.dictionary[0].deleted);
    }

    #[test]
    fn old_deletions_are_forgotten() {
        let mut shared = Library::default();
        let mut copy = Copy {
            dictionary: vec![entry("a", "1")],
            ..Copy::default()
        };
        copy.sync(&mut shared, 1_000);
        copy.dictionary.clear();
        copy.sync(&mut shared, 2_000);
        assert_eq!(shared.dictionary.len(), 1);
        copy.sync(&mut shared, 2_000 + KEEP_DELETIONS_MS + 1);
        assert!(shared.dictionary.is_empty());
    }

    #[test]
    fn untitled_and_duplicate_entries_stay_put() {
        let mut shared = Library::default();
        let mut copy = Copy {
            dictionary: vec![entry("  ", "x"), entry("a", "1"), entry("A", "2")],
            ..Copy::default()
        };
        copy.sync(&mut shared, 1_000);
        assert_eq!(copy.dictionary.len(), 3);
        assert_eq!(shared.dictionary.len(), 1);
        assert_eq!(shared.dictionary[0].item.replacement, "1");
    }

    #[test]
    fn imports_add_new_entries_and_update_matching_ones() {
        let library = export_library(
            &[entry("A", "new"), entry("b", "2")],
            &[snippet("sig", "Best")],
            1,
        );
        let mut dictionary = vec![entry("a", "old"), entry("c", "3")];
        assert_eq!(import_into(&mut dictionary, &library.dictionary), 2);
        let pairs: Vec<_> = dictionary
            .iter()
            .map(|e| (e.trigger.as_str(), e.replacement.as_str()))
            .collect();
        assert_eq!(pairs, [("A", "new"), ("c", "3"), ("b", "2")]);
        assert_eq!(
            import_into(&mut dictionary, &library.dictionary),
            0,
            "nothing new the second time"
        );

        let mut snippets: Vec<Snippet> = Vec::new();
        assert_eq!(import_into(&mut snippets, &library.snippets), 1);
    }

    #[test]
    fn the_file_format_is_stable() {
        let library = export_library(&[entry("teh", "the")], &[snippet("sig", "Best")], 42);
        let json = serde_json::to_value(&library).unwrap();
        assert_eq!(json["version"], 1);
        assert_eq!(json["dictionary"][0]["trigger"], "teh");
        assert_eq!(json["dictionary"][0]["modifiedAt"], 42);
        assert!(json["dictionary"][0].get("deleted").is_none());
        assert_eq!(json["snippets"][0]["expansion"], "Best");

        let read: Library = serde_json::from_str(
            r#"{"dictionary": [{"trigger": "x", "replacement": "y", "modifiedAt": 1, "deleted": true}]}"#,
        )
        .unwrap();
        assert!(read.dictionary[0].deleted && read.dictionary[0].item.is_enabled);
        assert!(read.snippets.is_empty());
    }

    #[test]
    fn json_files_are_written_whole_and_read_back() {
        let dir = std::env::temp_dir().join(format!("wordflow-sync-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(FILE_NAME);
        assert_eq!(read_json::<Library>(&path).unwrap(), None);
        let library = export_library(&[entry("a", "b")], &[], 7);
        write_json(&path, &library).unwrap();
        assert_eq!(read_json::<Library>(&path).unwrap(), Some(library));
        assert!(!dir.join("wordflow-shared.json.tmp").exists());
        fs::write(&path, "{not json").unwrap();
        assert!(read_json::<Library>(&path).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}
