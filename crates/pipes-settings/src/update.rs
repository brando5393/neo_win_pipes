//! Checks GitHub Releases for a newer version, and can fetch + launch the
//! new installer. This exists so updates "flow down" to installed copies
//! without needing a background service, a paid update host, or anything
//! beyond GitHub's free Releases hosting — see docs/ARCHITECTURE.md for
//! why a fully silent (zero-prompt) auto-updater isn't realistic here:
//! the screensaver lives in `System32`, so any update touching it still
//! needs one UAC prompt, same as the original install.
//!
//! The network call itself isn't unit-tested (that would be a flaky,
//! slow test hitting a real external API on every `cargo test`) — but
//! the parsing and version-comparison logic that actually decides "is
//! this newer" is pure and tested against sample GitHub API responses.

use semver::Version;
use serde::Deserialize;

const REPO: &str = "brando5393/neo_win_pipes";
const USER_AGENT: &str = "neo_win_pipes-pipes-settings";

#[derive(Debug, Clone, PartialEq)]
pub struct AvailableUpdate {
    pub version: Version,
    pub msi_download_url: String,
    pub release_page_url: String,
}

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    assets: Vec<GitHubAsset>,
}

#[derive(Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

/// Parses a GitHub "latest release" API response and returns an update
/// iff its version is strictly newer than `current` and it has a `.msi`
/// asset. Pure and unit-tested — `check_for_update` is a thin, untested
/// wrapper that just fetches this body over HTTP.
fn parse_update(current: &Version, body: &str) -> Option<AvailableUpdate> {
    let release: GitHubRelease = serde_json::from_str(body).ok()?;
    let tag = release.tag_name.trim_start_matches('v');
    let remote_version = Version::parse(tag).ok()?;
    if remote_version <= *current {
        return None;
    }
    let msi = release.assets.iter().find(|a| a.name.ends_with(".msi"))?;
    Some(AvailableUpdate {
        version: remote_version,
        msi_download_url: msi.browser_download_url.clone(),
        release_page_url: release.html_url,
    })
}

/// Checks GitHub for a newer release. Never panics; any network/parse
/// failure (offline, GitHub down, rate-limited, no releases published
/// yet) just yields `None` — a background version check failing
/// silently is correct behavior here, not something that should ever
/// interrupt using the app.
pub fn check_for_update(current: &Version) -> Option<AvailableUpdate> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let body = ureq::get(&url)
        .set("User-Agent", USER_AGENT)
        .timeout(std::time::Duration::from_secs(5))
        .call()
        .ok()?
        .into_string()
        .ok()?;
    parse_update(current, &body)
}

/// Downloads the update's installer to a temp file and returns its path.
pub fn download_installer(update: &AvailableUpdate) -> Result<std::path::PathBuf, String> {
    let path = std::env::temp_dir().join(format!("neo_win_pipes_update_{}.msi", update.version));
    let response = ureq::get(&update.msi_download_url)
        .set("User-Agent", USER_AGENT)
        .timeout(std::time::Duration::from_secs(120))
        .call()
        .map_err(|err| err.to_string())?;
    let mut file = std::fs::File::create(&path).map_err(|err| err.to_string())?;
    std::io::copy(&mut response.into_reader(), &mut file).map_err(|err| err.to_string())?;
    Ok(path)
}

/// Launches the downloaded installer in "passive" mode (progress UI only,
/// no wizard click-through, since the user already went through that on
/// first install) — still shows the one UAC prompt Windows requires for
/// anything touching `System32`; that can't be skipped, and pretending
/// otherwise would be dishonest about what's actually possible here.
pub fn launch_installer(path: &std::path::Path) -> std::io::Result<std::process::Child> {
    std::process::Command::new("msiexec")
        .args(["/i", &path.to_string_lossy(), "/passive", "/norestart"])
        .spawn()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_release(tag: &str, asset_name: &str) -> String {
        format!(
            r#"{{"tag_name": "{tag}", "html_url": "https://github.com/brando5393/neo_win_pipes/releases/tag/{tag}", "assets": [{{"name": "{asset_name}", "browser_download_url": "https://example.com/{asset_name}"}}]}}"#
        )
    }

    #[test]
    fn newer_version_is_detected() {
        let current = Version::parse("0.1.0").unwrap();
        let body = sample_release("v0.2.0", "neo_win_pipes.msi");
        let update = parse_update(&current, &body).expect("should detect an update");
        assert_eq!(update.version, Version::parse("0.2.0").unwrap());
        assert_eq!(
            update.msi_download_url,
            "https://example.com/neo_win_pipes.msi"
        );
    }

    #[test]
    fn same_version_is_not_an_update() {
        let current = Version::parse("0.1.0").unwrap();
        let body = sample_release("v0.1.0", "neo_win_pipes.msi");
        assert!(parse_update(&current, &body).is_none());
    }

    #[test]
    fn older_version_is_not_an_update() {
        let current = Version::parse("1.0.0").unwrap();
        let body = sample_release("v0.9.0", "neo_win_pipes.msi");
        assert!(parse_update(&current, &body).is_none());
    }

    #[test]
    fn missing_msi_asset_yields_no_update() {
        let current = Version::parse("0.1.0").unwrap();
        let body = sample_release("v0.2.0", "neo_win_pipes.tar.gz");
        assert!(parse_update(&current, &body).is_none());
    }

    #[test]
    fn malformed_json_yields_no_update_not_a_panic() {
        let current = Version::parse("0.1.0").unwrap();
        assert!(parse_update(&current, "not json").is_none());
    }

    #[test]
    fn tag_without_v_prefix_still_parses() {
        let current = Version::parse("0.1.0").unwrap();
        let body = sample_release("0.2.0", "neo_win_pipes.msi");
        assert!(parse_update(&current, &body).is_some());
    }
}
