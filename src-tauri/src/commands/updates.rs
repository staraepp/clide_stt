//! GitHub release awareness.
//!
//! This intentionally checks and links rather than downloading or executing a
//! package. A secure self-updater needs Tauri update-signing keys plus a
//! Developer ID/notarized public release; silently installing an unsigned DMG
//! would weaken the platform protections Clide depends on.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::database::{kv, now_ms};
use crate::state::AppState;

const RELEASE_API: &str = "https://api.github.com/repos/staraepp/clide_stt/releases/latest";
const RELEASES_URL: &str = "https://github.com/staraepp/clide_stt/releases/latest";
const CACHE_KEY: &str = "updates.latest_release";
const CHECK_INTERVAL_MS: i64 = 24 * 60 * 60 * 1_000;

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CachedRelease {
    version: String,
    url: String,
    checked_at: i64,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    current_version: String,
    latest_version: Option<String>,
    update_available: bool,
    release_url: String,
    checked_at: Option<i64>,
}

fn normalize_version(value: &str) -> &str {
    value
        .trim()
        .strip_prefix('v')
        .or_else(|| value.trim().strip_prefix('V'))
        .unwrap_or(value.trim())
}

fn is_newer(current: &str, latest: &str) -> bool {
    let Ok(current) = semver::Version::parse(normalize_version(current)) else {
        return false;
    };
    let Ok(latest) = semver::Version::parse(normalize_version(latest)) else {
        return false;
    };
    latest > current
}

fn status(current: &str, cached: Option<&CachedRelease>) -> UpdateStatus {
    let latest_version = cached.map(|release| normalize_version(&release.version).to_string());
    UpdateStatus {
        current_version: current.to_string(),
        update_available: latest_version
            .as_deref()
            .map(|latest| is_newer(current, latest))
            .unwrap_or(false),
        latest_version,
        release_url: cached
            .map(|release| release.url.clone())
            .unwrap_or_else(|| RELEASES_URL.to_string()),
        checked_at: cached.map(|release| release.checked_at),
    }
}

/// Check at most once per 24 hours unless the user presses Check now.
#[tauri::command]
pub async fn check_for_updates(app: AppHandle, force: bool) -> Result<UpdateStatus, String> {
    let state = app.state::<AppState>();
    let current = env!("CARGO_PKG_VERSION");
    let cached =
        kv::get::<CachedRelease>(&state.db.lock(), CACHE_KEY).map_err(|error| error.to_string())?;

    if !force
        && cached
            .as_ref()
            .is_some_and(|release| now_ms().saturating_sub(release.checked_at) < CHECK_INTERVAL_MS)
    {
        return Ok(status(current, cached.as_ref()));
    }

    let response = state
        .http
        .get(RELEASE_API)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|error| format!("Could not reach GitHub: {error}"))?
        .error_for_status()
        .map_err(|error| format!("GitHub could not provide the latest release: {error}"))?
        .json::<GitHubRelease>()
        .await
        .map_err(|error| format!("GitHub returned an unreadable release: {error}"))?;

    let release = CachedRelease {
        version: normalize_version(&response.tag_name).to_string(),
        url: response.html_url,
        checked_at: now_ms(),
    };

    // Never replace a valid cache with an unparsable tag.
    semver::Version::parse(&release.version).map_err(|_| {
        format!(
            "GitHub's latest tag is not a version: {}",
            response.tag_name
        )
    })?;
    kv::set(&state.db.lock(), CACHE_KEY, &release).map_err(|error| error.to_string())?;

    Ok(status(current, Some(&release)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comparison_handles_release_tags() {
        assert!(is_newer("0.1.0", "v0.1.1"));
        assert!(is_newer("1.9.9", "2.0.0"));
        assert!(!is_newer("0.1.1", "v0.1.1"));
        assert!(!is_newer("0.2.0", "v0.1.9"));
    }

    #[test]
    fn cached_release_becomes_a_frontend_status() {
        let cached = CachedRelease {
            version: "v0.2.0".into(),
            url: "https://example.com/release".into(),
            checked_at: 42,
        };
        let result = status("0.1.0", Some(&cached));
        assert!(result.update_available);
        assert_eq!(result.latest_version.as_deref(), Some("0.2.0"));
        assert_eq!(result.checked_at, Some(42));
    }
}
