use std::collections::HashMap;

#[tokio::test]
async fn cannot_make_request_of_gibberish() {
    let reader = tokio_test::io::Builder::new().read(b"sdksdjlkdj").build();
    let request_res = http_server::request::Request::try_from_stream(reader).await;
    assert!(request_res.is_err())
}

#[tokio::test]
async fn makes_request_with_headers() {
    let reader = tokio_test::io::Builder::new()
        .read(b"GET /resource HTTP/1.1\r\n")
        .read(b"Header-1: 0\r\n")
        .read(b"Header-2: 1\r\n")
        .read(b"\r\n")
        .build();
    let request = http_server::request::Request::try_from_stream(reader)
        .await
        .unwrap();
    assert_eq!(request.url(), "/resource");
    assert_eq!(request.method(), http_server::method::Method::Get);
    assert_eq!(
        *request.headers(),
        HashMap::from([
            ("Header-1".into(), String::from("0")),
            ("Header-2".into(), String::from("1")),
        ])
    );
}

#[tokio::test]
async fn cant_read_content_without_content_length() {
    let reader = tokio_test::io::Builder::new()
        .read(b"GET /resource HTTP/1.1\r\n")
        .read(b"Header-1: 0\r\n")
        .read(b"Header-2: 1\r\n")
        .read(b"\r\n")
        .build();
    let mut request = http_server::request::Request::try_from_stream(reader)
        .await
        .unwrap();
    assert!(request.content().await.is_err());
}

#[tokio::test]
async fn reads_content() {
    let reader = tokio_test::io::Builder::new()
        .read(b"GET /resource HTTP/1.1\r\n")
        .read(b"Content-Length: 10\r\n")
        .read(b"\r\n")
        .read(b"1223334444")
        .build();
    let mut request = http_server::request::Request::try_from_stream(reader)
        .await
        .unwrap();
    assert_eq!(request.content().await.unwrap(), "1223334444");
}
