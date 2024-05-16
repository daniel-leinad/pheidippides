#![feature(assert_matches)]
use std::assert_matches::assert_matches;

use pheidippides_messenger::Message;
use pheidippides_messenger::messenger::Messenger;
use mock_db;
use mock_db::Db;

#[tokio::test]
async fn subscribes_to_new_messages_without_starting_point() {
    let app = make_app().await;
    let user_id_1 = app.create_user("TestUser_1", "12345".into()).await.unwrap().unwrap();
    let user_id_2 = app.create_user("TestUser_2", "12345".into()).await.unwrap().unwrap();
    let user_id_3 = app.create_user("TestUser_3", "12345".into()).await.unwrap().unwrap();
    app.send_message("Message 1".into(), user_id_1, user_id_2).await.unwrap();
    app.send_message("Message 2".into(), user_id_2, user_id_1).await.unwrap();
    let mut subscription = app.subscribe_to_new_messages(user_id_1, None).await.unwrap();
    app.send_message("Message 3".into(), user_id_2, user_id_1).await.unwrap();
    app.send_message("Message 4".into(), user_id_1, user_id_2).await.unwrap();
    app.send_message("Message 5".into(), user_id_2, user_id_3).await.unwrap();
    app.send_message("Message 6".into(), user_id_3, user_id_1).await.unwrap();

    assert_matches!(subscription.recv().await.unwrap(), 
        Message{ from, to, message, ..} if from == user_id_2 && to == user_id_1 && &message == "Message 3");

    assert_matches!(subscription.recv().await.unwrap(), 
        Message{ from, to, message, ..} if from == user_id_1 && to == user_id_2 && &message == "Message 4");

    assert_matches!(subscription.recv().await.unwrap(), 
        Message{ from, to, message, ..} if from == user_id_3 && to == user_id_1 && &message == "Message 6");
}

#[tokio::test]
async fn subscribtions_to_new_messages_without_starting_point_dont_conflict() {
    let app = make_app().await;
    let user_id_1 = app.create_user("TestUser_1", "12345".into()).await.unwrap().unwrap();
    let user_id_2 = app.create_user("TestUser_2", "12345".into()).await.unwrap().unwrap();
    let user_id_3 = app.create_user("TestUser_3", "12345".into()).await.unwrap().unwrap();
    app.send_message("Message 1".into(), user_id_1, user_id_2).await.unwrap();
    app.send_message("Message 2".into(), user_id_2, user_id_1).await.unwrap();
    let mut subscription1 = app.subscribe_to_new_messages(user_id_1, None).await.unwrap();
    app.send_message("Message 3".into(), user_id_2, user_id_1).await.unwrap();
    app.send_message("Message 4".into(), user_id_1, user_id_2).await.unwrap();
    app.send_message("Message 5".into(), user_id_2, user_id_3).await.unwrap();
    let mut subscription2 = app.subscribe_to_new_messages(user_id_1, None).await.unwrap();
    app.send_message("Message 6".into(), user_id_3, user_id_1).await.unwrap();

    assert_matches!(subscription1.recv().await.unwrap(), 
        Message{ from, to, message, ..} if from == user_id_2 && to == user_id_1 && &message == "Message 3");

    assert_matches!(subscription1.recv().await.unwrap(), 
        Message{ from, to, message, ..} if from == user_id_1 && to == user_id_2 && &message == "Message 4");

    assert_matches!(subscription1.recv().await.unwrap(), 
        Message{ from, to, message, ..} if from == user_id_3 && to == user_id_1 && &message == "Message 6");

    assert_matches!(subscription2.recv().await.unwrap(), 
        Message{ from, to, message, ..} if from == user_id_3 && to == user_id_1 && &message == "Message 6");
}

#[tokio::test]
async fn subscribes_to_new_messages_with_starting_point() {
    let app = make_app().await;
    let user_id_1 = app.create_user("TestUser_1", "12345".into()).await.unwrap().unwrap();
    let user_id_2 = app.create_user("TestUser_2", "12345".into()).await.unwrap().unwrap();
    let user_id_3 = app.create_user("TestUser_3", "12345".into()).await.unwrap().unwrap();
    app.send_message("Message 1".into(), user_id_1, user_id_2).await.unwrap();
    app.send_message("Message 2".into(), user_id_2, user_id_1).await.unwrap();
    let starting_point = app.send_message("Message 3".into(), user_id_2, user_id_1).await.unwrap();
    app.send_message("Message 4".into(), user_id_1, user_id_2).await.unwrap();
    app.send_message("Message 5".into(), user_id_2, user_id_3).await.unwrap();
    app.send_message("Message 6".into(), user_id_3, user_id_1).await.unwrap();
    let mut subscription = app.subscribe_to_new_messages(user_id_1, Some(starting_point)).await.unwrap();

    assert_matches!(subscription.recv().await.unwrap(), 
        Message{ from, to, message, ..} if from == user_id_1 && to == user_id_2 && &message == "Message 4");
    assert_matches!(subscription.recv().await.unwrap(), 
        Message{ from, to, message, ..} if from == user_id_3 && to == user_id_1 && &message == "Message 6");
    
    app.send_message("Message 7".into(), user_id_2, user_id_1).await.unwrap();
    app.send_message("Message 8".into(), user_id_1, user_id_2).await.unwrap();
    app.send_message("Message 9".into(), user_id_2, user_id_3).await.unwrap();
    app.send_message("Message 10".into(), user_id_3, user_id_1).await.unwrap();

    assert_matches!(subscription.recv().await.unwrap(), 
        Message{ from, to, message, ..} if from == user_id_2 && to == user_id_1 && &message == "Message 7");

    assert_matches!(subscription.recv().await.unwrap(), 
        Message{ from, to, message, ..} if from == user_id_1 && to == user_id_2 && &message == "Message 8");

    assert_matches!(subscription.recv().await.unwrap(), 
        Message{ from, to, message, ..} if from == user_id_3 && to == user_id_1 && &message == "Message 10");
}

#[tokio::test]
async fn authorization_works_correctly() {
    let app = make_app().await;

    assert!(app.verify_user("User1", "User1".to_owned()).await.unwrap().is_none());
    assert!(app.verify_user("User2", "12345".to_owned()).await.unwrap().is_none());
    assert!(app.verify_user("__invalid_user", "12345".to_owned()).await.unwrap().is_none());

    app.create_user("User1", "User1".to_owned()).await.unwrap();

    assert!(app.verify_user("User1", "User1".to_owned()).await.unwrap().is_some());
    assert!(app.verify_user("User2", "12345".to_owned()).await.unwrap().is_none());
    assert!(app.verify_user("__invalid_user", "12345".to_owned()).await.unwrap().is_none());

    app.create_user("User2", "User2".to_owned()).await.unwrap();

    assert!(app.verify_user("User1", "User1".to_owned()).await.unwrap().is_some());
    assert!(app.verify_user("User2", "12345".to_owned()).await.unwrap().is_none());
    assert!(app.verify_user("__invalid_user", "12345".to_owned()).await.unwrap().is_none());
}

#[tokio::test]
async fn authorization_ignores_case() {
    let app = make_app().await;

    assert!(app.verify_user("User1", "User1".to_owned()).await.unwrap().is_none());
    assert!(app.verify_user("User1", "user1".to_owned()).await.unwrap().is_none());
    assert!(app.verify_user("user1", "User1".to_owned()).await.unwrap().is_none());
    assert!(app.verify_user("user1", "user1".to_owned()).await.unwrap().is_none());

    app.create_user("User1", "User1".to_owned()).await.unwrap();

    assert!(app.verify_user("User1", "User1".to_owned()).await.unwrap().is_some());
    assert!(app.verify_user("User1", "user1".to_owned()).await.unwrap().is_none());
    assert!(app.verify_user("user1", "User1".to_owned()).await.unwrap().is_some());
    assert!(app.verify_user("user1", "user1".to_owned()).await.unwrap().is_none());
}

async fn make_app() -> Messenger<Db, Db> {
    let db_access = Db::empty();
    let app = Messenger::new(db_access.clone(), db_access);
    app
}