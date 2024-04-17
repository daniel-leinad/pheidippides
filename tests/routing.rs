use pheidippides::{http, routing, db, app};

#[tokio::test]
async fn returns_bad_request_for_wrong_url() {
    let reader = tokio_test::io::Builder::new()
        .read(b"GET /random_url/aaa/bbbbb HTTP/1.1\r\n")
        .read(b"\r\n")
        .build();
    let mut request = http::Request::try_from_stream(reader).await.unwrap();
    let db_access = db::mock::Db::new().await;
    let app = app::App::new(db_access);
    let response = routing::handle_request(&mut request, app).await.unwrap();
    let is_bad_request = if let http::Response::BadRequest = response {true} else {false};
    assert!(is_bad_request);
}