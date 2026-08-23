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
    /// GitHub computes and serves a SHA-256 digest for every release asset
    /// itself (the same one shown on the splash site's "Verify your
    /// download" panel) — `None` only if GitHub's response is ever
    /// missing it, in which case `download_installer` skips verification
    /// rather than failing every update.
    pub expected_sha256: Option<String>,
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
    #[serde(default)]
    digest: Option<String>,
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
        expected_sha256: msi
            .digest
            .as_deref()
            .and_then(|d| d.strip_prefix("sha256:"))
            .map(str::to_owned),
    })
}

/// Lowercase hex SHA-256 of `bytes` — matches the format GitHub's own
/// `digest` field uses, so the two can be compared directly.
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
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

/// Downloads the update's installer to a temp file, verifies it against
/// `expected_sha256` (GitHub's own digest for that asset) when available,
/// and returns its path. This isn't a substitute for code signing (the
/// binaries are still unsigned — see docs/ROADMAP.md and SECURITY.md) —
/// it can't prove the release itself is legitimate, only that the bytes
/// on disk are exactly the bytes GitHub says it served. It does catch a
/// corrupted download or tampering in transit/at rest, for free, using a
/// value we already fetch. A mismatch deletes the file and fails the
/// update rather than launching an installer that doesn't match what was
/// promised.
pub fn download_installer(update: &AvailableUpdate) -> Result<std::path::PathBuf, String> {
    let path = std::env::temp_dir().join(format!("neo_win_pipes_update_{}.msi", update.version));
    let response = ureq::get(&update.msi_download_url)
        .set("User-Agent", USER_AGENT)
        .timeout(std::time::Duration::from_secs(120))
        .call()
        .map_err(|err| err.to_string())?;
    let mut bytes = Vec::new();
    std::io::copy(&mut response.into_reader(), &mut bytes).map_err(|err| err.to_string())?;

    verify_checksum(&bytes, update.expected_sha256.as_deref())?;

    std::fs::write(&path, &bytes).map_err(|err| err.to_string())?;
    Ok(path)
}

/// Pure (no I/O) so it's directly unit-testable, unlike the network call
/// around it. `expected: None` (GitHub's response was ever missing a
/// digest) passes through without checking anything.
fn verify_checksum(bytes: &[u8], expected: Option<&str>) -> Result<(), String> {
    let Some(expected) = expected else {
        return Ok(());
    };
    let actual = sha256_hex(bytes);
    if actual != expected {
        return Err(format!(
            "downloaded installer's checksum doesn't match GitHub's ({actual} != {expected}) - refusing to install it"
        ));
    }
    Ok(())
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

    fn sample_release_with_digest(tag: &str, asset_name: &str, digest: &str) -> String {
        format!(
            r#"{{"tag_name": "{tag}", "html_url": "https://github.com/brando5393/neo_win_pipes/releases/tag/{tag}", "assets": [{{"name": "{asset_name}", "browser_download_url": "https://example.com/{asset_name}", "digest": "{digest}"}}]}}"#
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

    #[test]
    fn digest_is_extracted_with_sha256_prefix_stripped() {
        let current = Version::parse("0.1.0").unwrap();
        let body = sample_release_with_digest("v0.2.0", "neo_win_pipes.msi", "sha256:abc123");
        let update = parse_update(&current, &body).unwrap();
        assert_eq!(update.expected_sha256.as_deref(), Some("abc123"));
    }

    #[test]
    fn missing_digest_yields_none_not_an_error() {
        let current = Version::parse("0.1.0").unwrap();
        let body = sample_release("v0.2.0", "neo_win_pipes.msi");
        let update = parse_update(&current, &body).unwrap();
        assert_eq!(update.expected_sha256, None);
    }

    #[test]
    fn sha256_hex_matches_known_vector() {
        // The well-known SHA-256 of the empty input.
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn verify_checksum_passes_when_no_expected_digest() {
        assert!(verify_checksum(b"anything", None).is_ok());
    }

    #[test]
    fn verify_checksum_passes_on_a_match() {
        let hash = sha256_hex(b"hello");
        assert!(verify_checksum(b"hello", Some(&hash)).is_ok());
    }

    #[test]
    fn verify_checksum_fails_on_a_mismatch() {
        let err = verify_checksum(b"tampered bytes", Some("0000000000000000")).unwrap_err();
        assert!(err.contains("doesn't match"));
    }
}
