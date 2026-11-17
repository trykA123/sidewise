//! The one request Sidewise makes: GitHub's release list, with fixed host, headers, limits and timeouts.

use crate::check::CheckError;

/// The GitHub API host that serves the release list.
pub const HOST: &str = "api.github.com";

/// The path and query of the release list request.
pub const PATH: &str = "/repos/trykA123/sidewise/releases?per_page=20";

/// The user agent GitHub sees.
pub const USER_AGENT: &str = "Sidewise";

/// The extra request headers, each line ending in CRLF.
pub const HEADERS: &str = "Accept: application/vnd.github+json\r\nX-GitHub-Api-Version: 2022-11-28";

/// WinHTTP timeouts in milliseconds: resolve, connect, send, receive.
pub const TIMEOUTS_MS: [i32; 4] = [5_000, 5_000, 5_000, 10_000];

/// The largest body chunk read at a time, in bytes.
pub const CHUNK_BYTES: usize = 64 * 1024;

/// Fetches the release feed body.
/// Windows: WinHTTP (U5); elsewhere `Unsupported`.
pub fn fetch_feed() -> Result<Vec<u8>, CheckError> {
    Err(CheckError::Unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::version::REPO;

    #[test]
    fn the_path_names_the_feed_repository() {
        assert_eq!(PATH, format!("/repos/{REPO}/releases?per_page=20"));
    }

    #[test]
    fn the_user_agent_is_a_product_name() {
        assert!(!USER_AGENT.chars().any(|c| c.is_ascii_digit()));
        assert!(!USER_AGENT.contains('/'));
    }

    #[test]
    fn headers_end_lines_in_crlf_and_send_no_credentials() {
        let bytes = HEADERS.as_bytes();
        for pair in bytes.windows(2) {
            if pair[1] == b'\n' {
                assert_eq!(pair[0], b'\r', "\\n must follow \\r");
            }
        }
        let lower = HEADERS.to_ascii_lowercase();
        for secret in ["user-agent", "authorization", "cookie"] {
            assert!(!lower.contains(secret), "{secret} must not be sent");
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn off_windows_the_fetch_is_unsupported() {
        assert_eq!(fetch_feed(), Err(CheckError::Unsupported));
    }

    /// Reads the real GitHub feed; run by hand in the Windows VM (`-- --ignored`).
    #[test]
    #[ignore = "reads the live GitHub feed over the network"]
    fn live_github_feed() -> Result<(), CheckError> {
        let body = fetch_feed()?;
        let releases = crate::feed::parse_feed(&body).map_err(|_| CheckError::BadFeed)?;
        assert!(!releases.is_empty(), "the published feed lists releases");
        let tags: Vec<String> = releases.iter().map(|(v, _)| v.to_string()).collect();
        assert!(
            tags.iter().any(|t| t == "0.1.0-alpha.2"),
            "alpha.2 is published: {tags:?}"
        );
        Ok(())
    }
}
