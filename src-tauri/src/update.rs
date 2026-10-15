//! Finds a newer Wordfall on GitHub and installs it in place.
//!
//! Wordfall 2 ships as pre-releases while SpeakType 1 is still GitHub's
//! "Latest" release, so this lists releases rather than asking for the latest
//! one. Pre-release builds move to any newer release; stable builds only to
//! stable ones. CI attaches a signed `latest.json` to each release, which the
//! updater plugin uses to download, verify and install the new version.

use std::{sync::Mutex, time::Duration};

use semver::Version;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::LockExt;

const RELEASES_URL: &str = "https://api.github.com/repos/Duke242/wordfall/releases?per_page=30";
/// Keeps a stalled connection from leaving the check pending forever.
const CHECK_TIMEOUT: Duration = Duration::from_secs(15);
/// The update manifest CI attaches to each release.
const MANIFEST: &str = "latest.json";

/// The update found by the last check, ready for `install_update`.
#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<Update>>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    available: bool,
    current_version: String,
    latest_version: String,
    notes: String,
    /// The release page, for updates that can't be installed in place.
    url: String,
    /// A signed build for this system is ready, so `install_update` can run.
    installable: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    downloaded: u64,
    total: Option<u64>,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    body: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

#[tauri::command]
pub async fn check_for_update(
    app: AppHandle,
    pending: State<'_, PendingUpdate>,
) -> Result<UpdateInfo, String> {
    let releases: Vec<Release> = reqwest::Client::new()
        .get(RELEASES_URL)
        .header("User-Agent", "Wordfall")
        .header("Accept", "application/vnd.github+json")
        .timeout(CHECK_TIMEOUT)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("Couldn't check for updates: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Couldn't read the update info: {e}"))?;

    let current = app.package_info().version.clone();
    let Some((release, latest)) = newest(&releases, &current) else {
        *pending.0.lock_unpoisoned() = None;
        return Ok(UpdateInfo {
            available: false,
            current_version: current.to_string(),
            latest_version: current.to_string(),
            notes: String::new(),
            url: String::new(),
            installable: false,
        });
    };

    let manifest = release.assets.iter().find(|a| a.name == MANIFEST);
    let update = match manifest {
        Some(asset) => signed_update(&app, &asset.browser_download_url).await,
        None => None,
    };
    let installable = update.is_some();
    *pending.0.lock_unpoisoned() = update;
    Ok(UpdateInfo {
        available: true,
        current_version: current.to_string(),
        latest_version: latest.to_string(),
        notes: release.body.clone(),
        url: release.html_url.clone(),
        installable,
    })
}

/// Downloads and installs the update the last check found, then restarts.
/// Progress arrives as `update-progress` events. On Windows the installer
/// closes the app itself and opens the new version.
#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    pending: State<'_, PendingUpdate>,
) -> Result<(), String> {
    let update = pending
        .0
        .lock_unpoisoned()
        .clone()
        .ok_or("Check for updates again, then install.")?;

    let mut downloaded = 0;
    let mut reported = 0;
    update
        .download_and_install(
            |chunk, total| {
                downloaded += chunk as u64;
                // About a hundred events for the whole download, not one per chunk.
                let step = total.map_or(1 << 20, |total| (total / 100).max(1));
                if downloaded - reported >= step || Some(downloaded) == total {
                    reported = downloaded;
                    let _ = app.emit("update-progress", Progress { downloaded, total });
                }
            },
            || {},
        )
        .await
        .map_err(|e| format!("Couldn't install the update: {e}"))?;
    app.restart()
}

/// The newest release this build should move to, and its version.
fn newest<'a>(releases: &'a [Release], current: &Version) -> Option<(&'a Release, Version)> {
    let stable_only = current.pre.is_empty();
    releases
        .iter()
        .filter(|release| !release.draft)
        .filter_map(|release| {
            let version = Version::parse(release.tag_name.strip_prefix('v')?).ok()?;
            Some((release, version))
        })
        .filter(|(_, version)| version > current && (version.pre.is_empty() || !stable_only))
        .max_by(|(_, a), (_, b)| a.cmp(b))
}

/// The release's signed build for this system. None while CI is still
/// building it, or if the release has none for this system, in which case
/// the update is offered as a download instead.
async fn signed_update(app: &AppHandle, manifest_url: &str) -> Option<Update> {
    let check = async {
        app.updater_builder()
            .endpoints(vec![manifest_url.parse()?])?
            // The release's tag already showed it's newer; the manifest only
            // supplies the files.
            .version_comparator(|_, _| true)
            .timeout(CHECK_TIMEOUT)
            .build()?
            .check()
            .await
    };
    check
        .await
        .unwrap_or_else(|e: tauri_plugin_updater::Error| {
            eprintln!("[update] no in-place update: {e}");
            None
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str) -> Release {
        Release {
            tag_name: tag.into(),
            body: String::new(),
            html_url: String::new(),
            draft: false,
            assets: Vec::new(),
        }
    }

    fn newest_tag(tags: &[&str], current: &str) -> Option<String> {
        let releases: Vec<_> = tags.iter().map(|tag| release(tag)).collect();
        newest(&releases, &Version::parse(current).unwrap()).map(|(r, _)| r.tag_name.clone())
    }

    #[test]
    fn pre_releases_move_to_the_newest_release() {
        let tags = [
            "v2.0.0-alpha.9",
            "v2.0.0-alpha.10",
            "v1.0.21",
            "v2.0.0-alpha.8",
        ];
        assert_eq!(
            newest_tag(&tags, "2.0.0-alpha.8").as_deref(),
            Some("v2.0.0-alpha.10")
        );
        assert_eq!(newest_tag(&tags, "2.0.0-alpha.10"), None);
        assert_eq!(
            newest_tag(&["v2.0.0-beta.1", "v2.0.0"], "2.0.0-alpha.9").as_deref(),
            Some("v2.0.0")
        );
    }

    #[test]
    fn stable_builds_skip_pre_releases() {
        let tags = ["v2.0.1-beta.1", "v2.0.0-alpha.9"];
        assert_eq!(newest_tag(&tags, "2.0.0"), None);
        assert_eq!(
            newest_tag(&["v2.0.1", "v2.0.1-beta.1"], "2.0.0").as_deref(),
            Some("v2.0.1")
        );
    }

    #[test]
    fn ignores_drafts_and_unreadable_tags() {
        let mut draft = release("v2.0.0-alpha.9");
        draft.draft = true;
        let releases = [draft, release("nightly"), release("v2.x")];
        assert!(newest(&releases, &Version::parse("2.0.0-alpha.8").unwrap()).is_none());
    }
}
