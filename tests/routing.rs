use web_server;
use pheidippides_messenger::messenger;
use pheidippides_web::routing;
use mock_db;

#[tokio::test]
async fn returns_bad_request_for_wrong_url() {
    let reader = tokio_test::io::Builder::new()
        .read(b"GET /random_url/aaa/bbbbb HTTP/1.1\r\n")
        .read(b"\r\n")
        .build();
    let mut request = web_server::Request::try_from_stream(reader).await.unwrap();
    let db_access = mock_db::Db::new().await;
    let app = messenger::Messenger::new(db_access);
    let response = routing::handle_request(&mut request, app).await.unwrap();
    let is_bad_request = if let web_server::Response::BadRequest = response {true} else {false};
    assert!(is_bad_request);
}