use std::time::Duration;
use uuid::uuid;

use pheidippides::db::DataAccess;
use pheidippides::Message;

#[macro_export]
macro_rules! db_access_tests {
    ($tester:ident) => {
        $tester!{it_creates_user}
        $tester!{doesnt_fetch_nonexistent_users}
        $tester!{it_creates_message}
        $tester!{fetches_last_messages}
        $tester!{fetches_users_messages_since}
    };
}

pub async fn it_creates_user(db_access: &impl DataAccess) {
    let username = "TestUser1";
    let user_id = db_access.create_user(username).await
        .unwrap()
        .unwrap();
    assert_eq!(db_access.fetch_user(&user_id).await.unwrap().unwrap().username, username);
    assert_eq!(db_access.find_user_by_username(username).await.unwrap().unwrap(), user_id);
}

pub async fn doesnt_fetch_nonexistent_users(db_access: &impl DataAccess) {
    assert!(db_access.fetch_user(&uuid!("4ec09097-45d5-43a0-bdea-614948bce47e")).await.unwrap().is_none());
    assert!(db_access.find_user_by_username("__NonExistentUserOnlyForTesting").await.unwrap().is_none());
}

pub async fn it_creates_message(db_access: &impl DataAccess) {
    let sender_id = db_access.create_user("__Test_Sender").await.unwrap().unwrap();
    let receiver_id = db_access.create_user("__Test_Receiver").await.unwrap().unwrap();
    let message = Message{ 
        id: uuid!("5b438594-81bc-48ce-8694-e2c14dcd45dc"), 
        from: sender_id, 
        to: receiver_id, 
        message: "Test message".to_owned(), 
        timestamp: chrono::Utc::now(), 
    };

    db_access.create_message(&message).await.unwrap();
}

pub async fn fetches_last_messages(db_access: &impl DataAccess) {
    let user_1 = db_access.create_user("__User_1").await.unwrap().unwrap();
    let user_2 = db_access.create_user("__User_2").await.unwrap().unwrap();
    let user_3 = db_access.create_user("__User_3").await.unwrap().unwrap();

    let messages = [
        (uuid!("ae8855d6-1446-4be2-b059-d1f150e7f33b"), user_1, user_2, "Message 1"),
        (uuid!("e24af34e-f1cb-4a82-8916-2072ec6c785d"), user_2, user_1, "Message 2"),
        (uuid!("ce9fe950-c323-4b3b-8a63-1c5497b2f582"), user_1, user_2, "Message 3"), // starting point
        (uuid!("160ee275-2b26-4745-b928-70e0fc0d6733"), user_2, user_1, "Message 4"),
        (uuid!("8c142367-3782-4bde-91ef-dd518a15c75a"), user_1, user_3, "Message 5"),
        (uuid!("29a9b6fd-ae3d-4602-a495-7530c488f6b4"), user_2, user_3, "Message 6"),
        (uuid!("8a75c5c9-d7b6-428e-a1cc-a34ee05c5a14"), user_3, user_2, "Message 7"),
        (uuid!("a82a13cd-7b34-4911-9319-0ae718287197"), user_3, user_1, "Message 8"),
    ];

    //TODO assert that message len fits into buffer

    let starting_id = uuid!("ce9fe950-c323-4b3b-8a63-1c5497b2f582");

    let mut timestamp = chrono::Utc::now();

    for (id, from, to, msg) in messages {
        timestamp = timestamp + Duration::from_secs(1);
        db_access.create_message(&Message { 
            id: id, 
            from, 
            to, 
            message: msg.to_owned(), 
            timestamp,
        }).await.unwrap();
    };

    let mut last_messages = db_access
        .fetch_last_messages_in_chat(&user_1, &user_2, None)
        .await
        .unwrap()
        .into_iter();

    assert_eq!(&last_messages.next().unwrap().message, "Message 4");
    assert_eq!(&last_messages.next().unwrap().message, "Message 3");
    assert_eq!(&last_messages.next().unwrap().message, "Message 2");
    assert_eq!(&last_messages.next().unwrap().message, "Message 1");
    assert_eq!(last_messages.next(), None);

    let mut last_messages = db_access
        .fetch_last_messages_in_chat(&user_3, &user_2, None)
        .await
        .unwrap()
        .into_iter();

    assert_eq!(&last_messages.next().unwrap().message, "Message 7");
    assert_eq!(&last_messages.next().unwrap().message, "Message 6");
    assert_eq!(last_messages.next(), None);

    let mut last_messages = db_access
        .fetch_last_messages_in_chat(&user_1, &user_2, Some(starting_id))
        .await
        .unwrap()
        .into_iter();

    assert_eq!(&last_messages.next().unwrap().message, "Message 2");
    assert_eq!(&last_messages.next().unwrap().message, "Message 1");
    assert_eq!(last_messages.next(), None);

}

pub async fn fetches_users_messages_since(db_access: &impl DataAccess) {
    let user_1 = db_access.create_user("__User_1").await.unwrap().unwrap();
    let user_2 = db_access.create_user("__User_2").await.unwrap().unwrap();
    let user_3 = db_access.create_user("__User_3").await.unwrap().unwrap();

    let messages = [
        (uuid!("cec8c6f5-39a2-4aed-91a7-10c60853f05a"), user_1, user_2, "Message 1"),
        (uuid!("2dbb11b1-7be2-4b56-a99f-c0102189f07e"), user_2, user_1, "Message 2"),
        (uuid!("cc11d4b1-d3dd-49b2-a83a-480d46a13c62"), user_1, user_2, "Message 3"), // starting point
        (uuid!("4b78d736-fdcc-48a3-91b5-2e55ecfd3794"), user_2, user_1, "Message 4"),
        (uuid!("e63e45b4-b8c9-4113-9c66-80e7f429bbd1"), user_1, user_3, "Message 5"),
        (uuid!("fa1a289c-f80a-429f-a509-b2c41b0f8ea4"), user_2, user_3, "Message 6"),
        (uuid!("4e00b39e-c248-45d0-885a-aba31b7471d7"), user_3, user_2, "Message 7"),
        (uuid!("832dc1d0-a74a-4c65-9a5b-e87c850f73dc"), user_3, user_1, "Message 8"),
    ];

    let starting_id = uuid!("cc11d4b1-d3dd-49b2-a83a-480d46a13c62");

    let mut timestamp = chrono::Utc::now();

    for (id, from, to, msg) in messages {
        timestamp = timestamp + Duration::from_secs(1);
        db_access.create_message(&Message { 
            id: id, 
            from, 
            to, 
            message: msg.to_owned(), 
            timestamp,
        }).await.unwrap();
    };

    let mut users_messages_since = db_access
        .fetch_users_messages_since(&user_1, &starting_id)
        .await
        .unwrap()
        .into_iter();
    
    assert_eq!(&users_messages_since.next().unwrap().message, "Message 4");
    assert_eq!(&users_messages_since.next().unwrap().message, "Message 5");
    assert_eq!(&users_messages_since.next().unwrap().message, "Message 8");
    assert_eq!(users_messages_since.next(), None);
}