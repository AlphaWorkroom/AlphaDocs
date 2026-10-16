//! Public release metadata only. No document content, identity, cookies or
//! credentials are read or transmitted; downloaded programs are never executed.
use serde::Deserialize;
use std::time::Duration;
pub const RELEASES: &str = "https://github.com/AlphaWorkroom/AlphaDocs/releases/latest";
const API: &str = "https://api.github.com/repos/AlphaWorkroom/AlphaDocs/releases/latest";
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Available {
    pub version: String,
    /// Verified update asset metadata for this OS/architecture, if the release provides one.
    pub asset: Option<ReleaseAsset>,
    /// A user-facing reason automatic installation is unavailable.
    pub install_error: Option<String>,
    /// Canonical public release page, suitable for manual download.
    pub url: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseAsset {
    pub name: String,
    pub url: String,
    pub size: u64,
    pub sha256: String,
}
#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    // Releases can contain unrelated assets uploaded by older tooling that
    // lacks one of these fields. Require both only for our selected installer.
    size: Option<u64>,
    digest: Option<String>,
}
#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    draft: bool,
    prerelease: bool,
    #[serde(default)]
    assets: Vec<Asset>,
}
pub fn installer(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        ("windows", "x86_64") => Some("AlphaDocs-Windows-x86_64-Setup.exe"),
        ("linux", "x86_64") => Some("AlphaDocs-Linux-x86_64.deb"),
        ("macos", "aarch64") => Some("AlphaDocs-macOS-aarch64.dmg"),
        ("macos", "x86_64") => Some("AlphaDocs-macOS-x86_64.dmg"),
        _ => None,
    }
}
pub fn due(now: u64, previous: u64) -> bool {
    previous == 0 || previous > now || now.saturating_sub(previous) >= 24 * 60 * 60
}
fn parse(bytes: &[u8], current: &str, os: &str, arch: &str) -> Result<Option<Available>, String> {
    let release: Release = serde_json::from_slice(bytes)
        .map_err(|_| "GitHub returned invalid release metadata".to_string())?;
    if release.draft || release.prerelease {
        return Ok(None);
    }
    let version = release
        .tag_name
        .strip_prefix('v')
        .unwrap_or(&release.tag_name);
    let next = semver::Version::parse(version).map_err(|_| "Invalid release version")?;
    let current = semver::Version::parse(current).map_err(|_| "Invalid installed version")?;
    if !next.pre.is_empty() || !next.build.is_empty() || next <= current {
        return Ok(None);
    }
    // Only canonical numeric version tags and this exact repository may open.
    if release.tag_name != format!("v{next}") {
        return Err("Non-canonical release tag".into());
    }
    let page = format!(
        "https://github.com/AlphaWorkroom/AlphaDocs/releases/tag/{}",
        release.tag_name
    );
    if release.html_url != page {
        return Err("Unexpected release location".into());
    }
    let (asset, install_error) = if let Some(name) = installer(os, arch) {
        let expected_url = format!(
            "https://github.com/AlphaWorkroom/AlphaDocs/releases/download/{}/{name}",
            release.tag_name
        );
        let mut matching = release.assets.iter().filter(|a| a.name == name);
        if let Some(found) = matching.next() {
            if matching.next().is_some() {
                return Err("Release contains duplicate installer assets".into());
            }
            if found.browser_download_url != expected_url {
                return Err("Unexpected update asset location".into());
            }
            let size = found
                .size
                .filter(|size| *size > 0 && *size <= MAX_ASSET_BYTES)
                .ok_or_else(|| {
                    "Update asset size is missing or exceeds the 100 MB limit".to_owned()
                })?;
            let digest = found
                .digest
                .as_deref()
                .and_then(|d| d.strip_prefix("sha256:"))
                .filter(|d| is_sha256(d))
                .ok_or_else(|| {
                    "GitHub did not provide a valid SHA-256 digest for the update".to_owned()
                })?;
            (
                Some(ReleaseAsset {
                    name: name.into(),
                    url: expected_url,
                    size,
                    sha256: digest.to_ascii_lowercase(),
                }),
                None,
            )
        } else {
            (
                None,
                Some("This release does not contain an installer for this platform.".into()),
            )
        }
    } else {
        (
            None,
            Some(format!(
                "Automatic updates are not available for {os}/{arch}. Download the release manually."
            )),
        )
    };
    Ok(Some(Available {
        version: next.to_string(),
        url: page,
        asset,
        install_error,
    }))
}
pub const MAX_ASSET_BYTES: u64 = 100_000_000;
pub fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}
pub fn check() -> Result<Option<Available>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .https_only(true)
        .max_redirects(0)
        .timeout_global(Some(Duration::from_secs(10)))
        .build()
        .into();
    let response = agent
        .get(API)
        .header("Accept", "application/vnd.github+json")
        .header(
            "User-Agent",
            concat!("AlphaDocs/", env!("CARGO_PKG_VERSION")),
        )
        .call();
    let mut response = match response {
        Err(ureq::Error::StatusCode(404)) => return Ok(None),
        Err(error) => return Err(format!("Could not check GitHub: {error}")),
        Ok(response) => response,
    };
    let bytes = response
        .body_mut()
        .with_config()
        .limit(1024 * 1024)
        .read_to_vec()
        .map_err(|e| e.to_string())?;
    parse(
        &bytes,
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mac_updates_choose_only_the_correct_architecture_dmg() {
        for arch in ["aarch64", "x86_64"] {
            let name = format!("AlphaDocs-macOS-{arch}.dmg");
            let page = "https://github.com/AlphaWorkroom/AlphaDocs/releases/tag/v1.0.0";
            let url = format!(
                "https://github.com/AlphaWorkroom/AlphaDocs/releases/download/v1.0.0/{name}"
            );
            let bytes = serde_json::to_vec(&serde_json::json!({"tag_name":"v1.0.0","html_url":page,"draft":false,"prerelease":false,"assets":[{"name":name,"browser_download_url":url,"size":100,"digest":format!("sha256:{}", "a".repeat(64))}]})).unwrap();
            assert_eq!(
                parse(&bytes, "0.1.1", "macos", arch)
                    .unwrap()
                    .unwrap()
                    .asset
                    .unwrap()
                    .url,
                url
            );
        }
    }
    fn release(tag: &str, draft: bool, prerelease: bool, url: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({"tag_name":tag,"html_url":url,"draft":draft,"prerelease":prerelease,"assets":[{"name":"AlphaDocs-Windows-x86_64-Setup.exe","browser_download_url":format!("https://github.com/AlphaWorkroom/AlphaDocs/releases/download/{tag}/AlphaDocs-Windows-x86_64-Setup.exe"),"size":100,"digest":format!("sha256:{}", "a".repeat(64))}]})).unwrap()
    }
    #[test]
    fn newer_stable_versions_choose_matching_assets_and_never_downgrade() {
        let bytes = release(
            "v0.2.0",
            false,
            false,
            "https://github.com/AlphaWorkroom/AlphaDocs/releases/tag/v0.2.0",
        );
        assert!(
            parse(&bytes, "0.1.10", "windows", "x86_64")
                .unwrap()
                .unwrap()
                .asset
                .unwrap()
                .url
                .ends_with("Setup.exe")
        );
        assert!(
            parse(&bytes, "0.2.0", "windows", "x86_64")
                .unwrap()
                .is_none()
        );
        assert!(
            parse(&bytes, "1.0.0", "windows", "x86_64")
                .unwrap()
                .is_none()
        );
        assert!(
            parse(&bytes, "0.1.0", "linux", "x86_64")
                .unwrap()
                .unwrap()
                .url
                .ends_with("/tag/v0.2.0")
        );
    }
    #[test]
    fn drafts_prereleases_malformed_and_foreign_urls_are_not_updates() {
        let page = "https://github.com/AlphaWorkroom/AlphaDocs/releases/tag/v2.0.0";
        for (draft, pre) in [(true, false), (false, true)] {
            assert!(
                parse(
                    &release("v2.0.0", draft, pre, page),
                    "0.1.0",
                    "windows",
                    "x86_64"
                )
                .unwrap()
                .is_none()
            );
        }
        assert!(
            parse(
                &release("v2.0.0", false, false, "https://evil.example/installer"),
                "0.1.0",
                "windows",
                "x86_64"
            )
            .is_err()
        );
        assert!(parse(b"invalid", "0.1.0", "windows", "x86_64").is_err());
        assert!(!due(100_000, 99_999));
        assert!(due(100_000, 0));
        assert!(due(100_000, 1));
        assert!(due(10, 100));
    }

    #[test]
    fn selected_asset_requires_bounded_size_and_valid_sha256_but_ignores_other_assets() {
        let tag = "v2.0.0";
        let page = "https://github.com/AlphaWorkroom/AlphaDocs/releases/tag/v2.0.0";
        let name = "AlphaDocs-Windows-x86_64-Setup.exe";
        let url =
            format!("https://github.com/AlphaWorkroom/AlphaDocs/releases/download/{tag}/{name}");
        let metadata = |fields: serde_json::Value| {
            serde_json::to_vec(&serde_json::json!({
                "tag_name": tag,
                "html_url": page,
                "draft": false,
                "prerelease": false,
                "assets": fields,
            }))
            .unwrap()
        };
        // Irrelevant assets should not make the selected installer unusable.
        let valid = metadata(serde_json::json!([
            {"name":"symbols.zip","browser_download_url":"https://example.invalid/symbols.zip"},
            {"name":name,"browser_download_url":url,"size":100,"digest":format!("sha256:{}", "A".repeat(64))}
        ]));
        let result = parse(&valid, "1.0.0", "windows", "x86_64")
            .unwrap()
            .unwrap();
        let selected = result.asset.unwrap();
        assert_eq!(selected.size, 100);
        assert_eq!(selected.sha256, "a".repeat(64));

        for (size, digest) in [
            (
                serde_json::json!(null),
                serde_json::json!(format!("sha256:{}", "a".repeat(64))),
            ),
            (
                serde_json::json!(0),
                serde_json::json!(format!("sha256:{}", "a".repeat(64))),
            ),
            (
                serde_json::json!(MAX_ASSET_BYTES + 1),
                serde_json::json!(format!("sha256:{}", "a".repeat(64))),
            ),
            (serde_json::json!(100), serde_json::json!(null)),
            (serde_json::json!(100), serde_json::json!("sha256:bad")),
        ] {
            let bytes = metadata(serde_json::json!([{
                "name":name,
                "browser_download_url":url,
                "size":size,
                "digest":digest
            }]));
            assert!(parse(&bytes, "1.0.0", "windows", "x86_64").is_err());
        }
        let bad_location = metadata(serde_json::json!([{
            "name":name,
            "browser_download_url":"https://evil.example/installer.exe",
            "size":100,
            "digest":format!("sha256:{}", "a".repeat(64))
        }]));
        assert!(parse(&bad_location, "1.0.0", "windows", "x86_64").is_err());
        let duplicate = metadata(serde_json::json!([
            {"name":name,"browser_download_url":url,"size":100,"digest":format!("sha256:{}", "a".repeat(64))},
            {"name":name,"browser_download_url":url,"size":100,"digest":format!("sha256:{}", "b".repeat(64))}
        ]));
        assert!(parse(&duplicate, "1.0.0", "windows", "x86_64").is_err());
    }

    #[test]
    fn unsupported_platform_has_manual_fallback_without_an_install_asset() {
        let bytes = release(
            "v2.0.0",
            false,
            false,
            "https://github.com/AlphaWorkroom/AlphaDocs/releases/tag/v2.0.0",
        );
        let available = parse(&bytes, "1.0.0", "freebsd", "x86_64")
            .unwrap()
            .unwrap();
        assert!(available.asset.is_none());
        assert!(available.install_error.unwrap().contains("freebsd/x86_64"));
        assert!(available.url.ends_with("/tag/v2.0.0"));
    }
}
