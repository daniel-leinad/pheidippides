#![feature(assert_matches)]
use std::assert_matches::assert_matches;

use pheidippides::{app, db};

#[tokio::test]
async fn subscribes_to_new_messages_without_starting_point() {
    let db_access = db::mock::Db::new().await;
    let app = app::App::new(db_access);
    let user_id_1 = app.create_user("TestUser_1", "12345".into()).await.unwrap().unwrap();
    let user_id_2 = app.create_user("TestUser_2", "12345".into()).await.unwrap().unwrap();
    let user_id_3 = app.create_user("TestUser_3", "12345".into()).await.unwrap().unwrap();
    app.send_message("Message 1".into(), user_id_1, user_id_2).await.unwrap();
    app.send_message("Message 2".into(), user_id_2, user_id_1).await.unwrap();
    let mut subscription = app.subscribe_new_messages(user_id_1, None).await.unwrap();
    app.send_message("Message 3".into(), user_id_2, user_id_1).await.unwrap();
    app.send_message("Message 4".into(), user_id_1, user_id_2).await.unwrap();
    app.send_message("Message 5".into(), user_id_2, user_id_3).await.unwrap();
    app.send_message("Message 6".into(), user_id_3, user_id_1).await.unwrap();

    assert_matches!(subscription.recv().await.unwrap(), 
        db::Message{ from, to, message, ..} if from == user_id_2 && to == user_id_1 && &message == "Message 3");

    assert_matches!(subscription.recv().await.unwrap(), 
        db::Message{ from, to, message, ..} if from == user_id_1 && to == user_id_2 && &message == "Message 4");

    assert_matches!(subscription.recv().await.unwrap(), 
        db::Message{ from, to, message, ..} if from == user_id_3 && to == user_id_1 && &message == "Message 6");
}

#[tokio::test]
async fn subscribes_to_new_messages_with_starting_point() {
    let db_access = db::mock::Db::new().await;
    let app = app::App::new(db_access);
    let user_id_1 = app.create_user("TestUser_1", "12345".into()).await.unwrap().unwrap();
    let user_id_2 = app.create_user("TestUser_2", "12345".into()).await.unwrap().unwrap();
    let user_id_3 = app.create_user("TestUser_3", "12345".into()).await.unwrap().unwrap();
    app.send_message("Message 1".into(), user_id_1, user_id_2).await.unwrap();
    app.send_message("Message 2".into(), user_id_2, user_id_1).await.unwrap();
    let starting_point = app.send_message("Message 3".into(), user_id_2, user_id_1).await.unwrap();
    app.send_message("Message 4".into(), user_id_1, user_id_2).await.unwrap();
    app.send_message("Message 5".into(), user_id_2, user_id_3).await.unwrap();
    app.send_message("Message 6".into(), user_id_3, user_id_1).await.unwrap();
    let mut subscription = app.subscribe_new_messages(user_id_1, Some(starting_point)).await.unwrap();

    assert_matches!(subscription.recv().await.unwrap(), 
        db::Message{ from, to, message, ..} if from == user_id_1 && to == user_id_2 && &message == "Message 4");
    assert_matches!(subscription.recv().await.unwrap(), 
        db::Message{ from, to, message, ..} if from == user_id_3 && to == user_id_1 && &message == "Message 6");
    
    app.send_message("Message 7".into(), user_id_2, user_id_1).await.unwrap();
    app.send_message("Message 8".into(), user_id_1, user_id_2).await.unwrap();
    app.send_message("Message 9".into(), user_id_2, user_id_3).await.unwrap();
    app.send_message("Message 10".into(), user_id_3, user_id_1).await.unwrap();

    assert_matches!(subscription.recv().await.unwrap(), 
        db::Message{ from, to, message, ..} if from == user_id_2 && to == user_id_1 && &message == "Message 7");

    assert_matches!(subscription.recv().await.unwrap(), 
        db::Message{ from, to, message, ..} if from == user_id_1 && to == user_id_2 && &message == "Message 8");

    assert_matches!(subscription.recv().await.unwrap(), 
        db::Message{ from, to, message, ..} if from == user_id_3 && to == user_id_1 && &message == "Message 10");
}