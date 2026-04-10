#![cfg(feature = "http")]

use ::mockito::Matcher;
use sparseio::{Reader as _, sources::http::Reader};
use sparseio::utils::tracing;

/// This test covers the simplest discovery path so HTTP length probing
/// keeps using HEAD metadata when it is present and valid.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn head_content_length_discovers_length() {
    tracing::init();

    let mut server = mockito::Server::new_async().await;
    let _m = server
        .mock("HEAD", "/asset")
        .with_status(200)
        .with_header("content-length", "12")
        .create_async()
        .await;

    let reader = Reader::with_client(reqwest::Client::new(), server.url() + "/asset");
    assert_eq!(reader.len().await.expect("len should succeed"), 12);
}

/// This test covers the range-based fallback used when HEAD metadata is
/// incomplete but the server still honors ranged GET requests.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn content_range_fallback_discovers_length() {
    tracing::init();

    let mut server = mockito::Server::new_async().await;
    let _head = server
        .mock("HEAD", "/asset")
        .with_status(200)
        .create_async()
        .await;
    let _range = server
        .mock("GET", "/asset")
        .match_header("range", "bytes=0-0")
        .with_status(206)
        .with_header("content-range", "bytes 0-0/99")
        .with_body("a")
        .create_async()
        .await;

    let reader = Reader::with_client(reqwest::Client::new(), server.url() + "/asset");
    assert_eq!(reader.len().await.expect("len should succeed"), 99);
}

/// This test covers servers that return a full 200 response for the range
/// probe and only expose the payload length through the body itself.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ok_body_length_fallback_discovers_length() {
    tracing::init();

    let mut server = mockito::Server::new_async().await;
    let _head = server
        .mock("HEAD", "/asset")
        .with_status(405)
        .create_async()
        .await;
    let _range = server
        .mock("GET", "/asset")
        .match_header("range", "bytes=0-0")
        .with_status(200)
        .with_body("hello")
        .create_async()
        .await;

    let reader = Reader::with_client(reqwest::Client::new(), server.url() + "/asset");
    assert_eq!(reader.len().await.expect("len should succeed"), 5);
}

/// This test pins the emitted Range header so the HTTP reader keeps asking
/// for the exact byte window the caller requested.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn range_header_shape_matches_offset_and_buffer_length() {
    tracing::init();

    let mut server = mockito::Server::new_async().await;
    let _range = server
        .mock("GET", "/asset")
        .match_header("range", Matcher::Exact("bytes=4-7".to_string()))
        .with_status(206)
        .with_header("content-range", "bytes 4-7/8")
        .with_body("efgh")
        .create_async()
        .await;
    let reader = Reader::with_client(reqwest::Client::new(), server.url() + "/asset");

    let mut buf = [0u8; 4];
    let read = reader.read_at(4, &mut buf).await.expect("read should succeed");
    assert_eq!(read, 4);
    assert_eq!(&buf, b"efgh");
}

/// This test keeps the EOF mapping explicit for sparse readers that probe
/// past the end of the remote object.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn range_not_satisfiable_maps_to_eof() {
    tracing::init();

    let mut server = mockito::Server::new_async().await;
    let _range = server
        .mock("GET", "/asset")
        .match_header("range", "bytes=8-11")
        .with_status(416)
        .create_async()
        .await;
    let reader = Reader::with_client(reqwest::Client::new(), server.url() + "/asset");

    let mut buf = [0u8; 4];
    let read = reader.read_at(8, &mut buf).await.expect("EOF should not fail");
    assert_eq!(read, 0);
}

/// This test ensures callers can override unreliable length metadata
/// without consulting the network path at all.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn len_override_takes_precedence_over_network_metadata() {
    tracing::init();

    let server = mockito::Server::new_async().await;
    let reader = Reader::with_client(reqwest::Client::new(), server.url() + "/asset").with_len_override(1234);
    assert_eq!(reader.len().await.expect("override should win"), 1234);
}

/// This test keeps the parser strict so obviously malformed length headers
/// do not silently produce the wrong object size.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn malformed_headers_cause_length_discovery_to_fail() {
    tracing::init();

    let mut server = mockito::Server::new_async().await;
    let _head = server
        .mock("HEAD", "/asset")
        .with_status(200)
        .with_header("content-length", "not-a-number")
        .create_async()
        .await;
    let _range = server
        .mock("GET", "/asset")
        .match_header("range", "bytes=0-0")
        .with_status(206)
        .with_header("content-range", "bytes 0-0/*")
        .with_body("a")
        .create_async()
        .await;

    let reader = Reader::with_client(reqwest::Client::new(), server.url() + "/asset");
    assert!(reader.len().await.is_err(), "malformed headers should not be accepted");
}

/// This test covers the case where a server ignores a non-zero Range
/// request and returns an unbounded 200 response instead.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ignored_range_errors_at_non_zero_offsets_are_rejected() {
    tracing::init();

    let mut server = mockito::Server::new_async().await;
    let _range = server
        .mock("GET", "/asset")
        .match_header("range", Matcher::Exact("bytes=5-8".to_string()))
        .with_status(200)
        .with_body("abcdefgh")
        .create_async()
        .await;
    let reader = Reader::with_client(reqwest::Client::new(), server.url() + "/asset");

    let mut buf = [0u8; 4];
    assert!(reader.read_at(5, &mut buf).await.is_err(), "non-zero offsets require a range-aware response");
}
