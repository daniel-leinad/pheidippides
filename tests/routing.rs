use pheidippides_web::routing;
use mock_db::Db;
use pheidippides_auth::AuthServiceUsingArgon2;
use pheidippides_messenger::messenger::Messenger;

#[tokio::test]
async fn returns_bad_request_for_wrong_url() {
    let app = make_app().await;

    let reader = tokio_test::io::Builder::new()
        .read(b"GET /random_url/aaa/bbbbb HTTP/1.1\r\n")
        .read(b"\r\n")
        .build();
    let mut request = http_server::request::Request::try_from_stream(reader).await.unwrap();

    let response = routing::route(&mut request, app).await.unwrap();
    assert!(response.is_bad_request());
}

async fn make_app() -> Messenger<Db, AuthServiceUsingArgon2<Db>> {
    let db_access = mock_db::Db::new().await;
    let auth_service = AuthServiceUsingArgon2::new(db_access.clone());

    Messenger::new(db_access, auth_service)
}