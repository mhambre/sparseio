//! HTTP range-based source implementation.
//!
//! This module provides [`Reader`], a [`crate::Reader`] trait implementation that
//! fetches byte ranges from an HTTP endpoint using [`reqwest`].

use std::io;
use std::time::Duration;

use reqwest::StatusCode;

const IDLE_POOL_TIMEOUT: Duration = Duration::from_secs(300);
const IDLE_POOL_MAX_SIZE: usize = 32;
const KEEPALIVE: Duration = Duration::from_secs(30);

/// Reads byte ranges from an HTTP resource.
///
/// `Reader` issues `GET` requests with a `Range` header for `read_at`, and
/// attempts to determine object length from `HEAD` `Content-Length` in `len`.
#[derive(Clone, Debug)]
pub struct Reader {
    client: reqwest::Client,
    url: String,
    len_override: Option<usize>,
}

impl Reader {
    /// Creates a new HTTP reader for `url` with a default [`reqwest::Client`].
    pub fn new(url: impl Into<String>) -> Self {
        // Build a long-lived client so sequential range reads can reuse pooled
        // connections instead of re-handshaking after short idle periods.
        let client = reqwest::Client::builder()
            .pool_idle_timeout(IDLE_POOL_TIMEOUT)
            .pool_max_idle_per_host(IDLE_POOL_MAX_SIZE)
            .tcp_keepalive(KEEPALIVE)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            client,
            url: url.into(),
            len_override: None,
        }
    }

    /// Creates a new HTTP reader using a caller-provided [`reqwest::Client`].
    pub fn with_client(client: reqwest::Client, url: impl Into<String>) -> Self {
        Self {
            client,
            url: url.into(),
            len_override: None,
        }
    }

    /// Returns a copy of this reader configured with a caller-provided object length.
    ///
    /// Use this when the caller already knows the object size but the server
    /// does not provide reliable `Content-Length` / `Content-Range` headers.
    pub fn with_len_override(mut self, len: usize) -> Self {
        self.len_override = Some(len);
        self
    }

    /// Sets or clears the caller-provided object length override.
    pub fn set_len_override(&mut self, len: Option<usize>) {
        self.len_override = len;
    }

    /// Returns the currently configured object length override.
    pub fn len_override(&self) -> Option<usize> {
        self.len_override
    }

    /// Returns the source URL used by this reader.
    pub fn url(&self) -> &str {
        &self.url
    }
}

impl crate::Reader for Reader {
    async fn read_at(&self, offset: usize, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }

        let end = offset
            .checked_add(buffer.len().saturating_sub(1))
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "range end overflow"))?;
        let range_header = format!("bytes={offset}-{end}");

        let mut response = self
            .client
            .get(&self.url)
            .header(reqwest::header::RANGE, range_header)
            .send()
            .await
            .map_err(io::Error::other)?;

        match response.status() {
            StatusCode::PARTIAL_CONTENT => {
                let (start, end, _) = response
                    .headers()
                    .get(reqwest::header::CONTENT_RANGE)
                    .and_then(|value| value.to_str().ok())
                    .and_then(parse_content_range)
                    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing or malformed Content-Range"))?;

                if start != offset {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("HTTP range response started at {start}, expected {offset}"),
                    ));
                }

                let advertised_len = end
                    .checked_sub(start)
                    .and_then(|len| len.checked_add(1))
                    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid Content-Range length"))?;
                if advertised_len > buffer.len() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "HTTP range response advertised {advertised_len} bytes for a {} byte request",
                            buffer.len()
                        ),
                    ));
                }

                let copied = read_response_prefix(&mut response, buffer).await?;
                if copied != advertised_len {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        format!(
                            "HTTP range response body length {copied} did not match advertised length {advertised_len}"
                        ),
                    ));
                }
                Ok(copied)
            },
            StatusCode::OK if offset == 0 => read_response_prefix(&mut response, buffer).await,
            StatusCode::RANGE_NOT_SATISFIABLE => Ok(0),
            _ => Err(io::Error::other(format!(
                "unexpected HTTP status {} for ranged read at offset {offset}",
                response.status()
            ))),
        }
    }

    async fn len(&self) -> io::Result<usize> {
        if let Some(len) = self.len_override {
            return Ok(len);
        }

        match self.client.head(&self.url).send().await {
            Ok(response) if response.status().is_success() => {
                if let Some(len) = response
                    .headers()
                    .get(reqwest::header::CONTENT_LENGTH)
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.parse::<usize>().ok())
                {
                    return Ok(len);
                }
            },
            _ => {},
        }

        // Fall back to a tiny ranged GET for servers that omit Content-Length on HEAD.
        let get = self
            .client
            .get(&self.url)
            .header(reqwest::header::RANGE, "bytes=0-0")
            .send()
            .await;
        if let Ok(response) = get {
            if response.status() == StatusCode::PARTIAL_CONTENT {
                if let Some(total) = response
                    .headers()
                    .get(reqwest::header::CONTENT_RANGE)
                    .and_then(|value| value.to_str().ok())
                    .and_then(parse_content_range)
                    .and_then(|(_, _, total)| total)
                {
                    return Ok(total);
                }
            } else if response.status() == StatusCode::OK {
                if let Some(len) = response
                    .headers()
                    .get(reqwest::header::CONTENT_LENGTH)
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.parse::<usize>().ok())
                {
                    return Ok(len);
                }
                if let Ok(bytes) = response.bytes().await {
                    return Ok(bytes.len());
                }
            }
        }

        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to determine content length for {}", self.url),
        ))
    }
}

async fn read_response_prefix(response: &mut reqwest::Response, buffer: &mut [u8]) -> io::Result<usize> {
    let mut copied = 0usize;

    while copied < buffer.len() {
        let Some(chunk) = response.chunk().await.map_err(io::Error::other)? else {
            break;
        };
        if chunk.is_empty() {
            continue;
        }

        let to_copy = (buffer.len() - copied).min(chunk.len());
        buffer[copied..copied + to_copy].copy_from_slice(&chunk[..to_copy]);
        copied += to_copy;
    }

    Ok(copied)
}

fn parse_content_range(value: &str) -> Option<(usize, usize, Option<usize>)> {
    let value = value.strip_prefix("bytes ")?;
    let (range, total) = value.split_once('/')?;
    let (start, end) = range.split_once('-')?;
    let start = start.parse::<usize>().ok()?;
    let end = end.parse::<usize>().ok()?;
    if end < start {
        return None;
    }

    let total = if total == "*" {
        None
    } else {
        Some(total.parse::<usize>().ok()?)
    };

    Some((start, end, total))
}

#[deprecated(note = "Use sources::http::Reader")]
pub type HttpReader = Reader;

#[cfg(test)]
mod tests {
    use super::{Reader, parse_content_range};
    use crate::Reader as _;

    /// This test keeps the override path isolated from any HTTP probing so
    /// callers can bypass unreliable server metadata completely.
    #[tokio::test]
    async fn len_uses_override_without_network() {
        let reader = Reader::new("http://127.0.0.1:1/unreachable").with_len_override(12345);
        assert_eq!(reader.len().await.expect("len override should be returned"), 12345);
    }

    /// This test pins the plain Content-Range parser so the HTTP reader can
    /// safely interpret a server's range metadata.
    #[test]
    fn parse_content_range_extracts_start_end_and_total_length() {
        assert_eq!(parse_content_range("bytes 0-0/99"), Some((0, 0, Some(99))));
        assert_eq!(parse_content_range("bytes 10-19/2048"), Some((10, 19, Some(2048))));
    }

    /// This test keeps malformed or wildcard totals from being misread as
    /// valid lengths.
    #[test]
    fn parse_content_range_rejects_malformed_ranges_and_keeps_wildcard_totals_optional() {
        assert_eq!(parse_content_range("bytes 0-0/*"), Some((0, 0, None)));
        assert_eq!(parse_content_range("bytes 8-3/12"), None);
        assert_eq!(parse_content_range("not-a-range"), None);
    }
}
