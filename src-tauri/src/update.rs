//! Finds a newer WordFlow on GitHub and installs it in place.
//!
//! WordFlow ships as pre-releases, which GitHub's "latest release" skips, so
//! this lists releases rather than asking for the latest one. Pre-release builds move to any newer release; stable builds only to
//! stable ones. CI attaches a signed `latest.json` to each release once its
//! installers are built, which the updater plugin uses to download, verify and
//! install the new version. Releases without one aren't offered: they're still
//! building, or predate the updater and so are older than this build.

use std::{sync::Mutex, time::Duration};

use semver::Version;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::{
    AppState, LockExt,
    dictation::DictationState,
    meeting::{MeetingState, Meetings},
};

const RELEASES_URL: &str = "https://api.github.com/repos/Duke242/wordflow/releases?per_page=30";
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
        .header("User-Agent", "WordFlow")
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

    let update = signed_update(&app, release.manifest_url, &latest).await;
    let installable = update.is_some();
    *pending.0.lock_unpoisoned() = update;
    Ok(UpdateInfo {
        available: true,
        current_version: current.to_string(),
        latest_version: latest.to_string(),
        notes: release.body.to_string(),
        url: release.html_url.to_string(),
        installable,
    })
}

/// Downloads and installs `version`, which the last check found, then
/// restarts. Progress arrives as `update-progress` events. On Windows the
/// installer closes the app itself and opens the new version.
#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    version: String,
    pending: State<'_, PendingUpdate>,
    state: State<'_, AppState>,
    meetings: State<'_, Meetings>,
) -> Result<(), String> {
    // Restarting would cut off the recording.
    if !matches!(
        *state.dictation_state.lock_unpoisoned(),
        DictationState::Idle
    ) {
        return Err("Finish dictating, then install the update.".into());
    }
    if !matches!(meetings.state(), MeetingState::Idle) {
        return Err("Stop the meeting recording, then install the update.".into());
    }
    // Another check may have found something else since this one was shown.
    let update = pending
        .0
        .lock_unpoisoned()
        .clone()
        .filter(|update| update.version == version)
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

/// A release with its installers built, ready to update to.
struct Candidate<'a> {
    body: &'a str,
    html_url: &'a str,
    manifest_url: &'a str,
}

/// The newest release this build should move to, and its version.
fn newest<'a>(releases: &'a [Release], current: &Version) -> Option<(Candidate<'a>, Version)> {
    let stable_only = current.pre.is_empty();
    releases
        .iter()
        .filter(|release| !release.draft)
        .filter_map(|release| {
            let version = Version::parse(release.tag_name.strip_prefix('v')?).ok()?;
            let manifest = release.assets.iter().find(|a| a.name == MANIFEST)?;
            let candidate = Candidate {
                body: &release.body,
                html_url: &release.html_url,
                manifest_url: &manifest.browser_download_url,
            };
            Some((candidate, version))
        })
        .filter(|(_, version)| version > current && (version.pre.is_empty() || !stable_only))
        .max_by(|(_, a), (_, b)| a.cmp(b))
}

/// The release's signed build for this system. None if it has none for this
/// system, in which case the update is offered as a download instead.
async fn signed_update(app: &AppHandle, manifest_url: &str, version: &Version) -> Option<Update> {
    let version = version.clone();
    let check = async {
        app.updater_builder()
            .endpoints(vec![manifest_url.parse()?])?
            // The release's tag already showed it's newer. The manifest must be
            // for that same version, so an old manifest can't roll the app back.
            .version_comparator(move |_, release| release.version == version)
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
            assets: vec![Asset {
                name: MANIFEST.into(),
                browser_download_url: format!("https://example.com/{tag}/{MANIFEST}"),
            }],
        }
    }

    fn newest_tag(tags: &[&str], current: &str) -> Option<String> {
        let releases: Vec<_> = tags.iter().map(|tag| release(tag)).collect();
        newest(&releases, &Version::parse(current).unwrap())
            .map(|(_, version)| format!("v{version}"))
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
    fn ignores_drafts_unbuilt_releases_and_unreadable_tags() {
        let mut draft = release("v2.0.0-alpha.9");
        draft.draft = true;
        let mut building = release("v2.0.0-alpha.10");
        building.assets.clear();
        let releases = [draft, building, release("nightly"), release("v2.x")];
        assert!(newest(&releases, &Version::parse("2.0.0-alpha.8").unwrap()).is_none());
    }
}
