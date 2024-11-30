use proc_heim::Cmd;
use tokio_stream::StreamExt as _;

use crate::common::create_process_manager;

pub async fn should_read_message<F: Fn(&str) -> Cmd>(cmd_with_message: F) {
    let (_dir, handle) = create_process_manager();
    let msg = "example message";
    let process_id = handle.spawn(cmd_with_message(&msg)).await.unwrap();

    let mut stream = handle.subscribe_message_stream(process_id).await.unwrap();
    assert_eq!(msg.as_bytes(), stream.next().await.unwrap());
}

pub async fn should_spawn_process_then_communicate_with_it_then_kill(cmd: Cmd) {
    let (_dir, handle) = create_process_manager();
    let process_id = handle.spawn(cmd).await.unwrap();

    handle.write_message(process_id, b"msg1").await.unwrap();
    handle.write_message(process_id, b"msg2").await.unwrap();

    let handle2 = handle.clone();
    let reader = tokio::spawn(async move {
        let mut stream = handle2.subscribe_message_stream(process_id).await.unwrap();
        let mut counter = 0;
        while let Some(msg) = stream.next().await {
            match counter {
                0 => assert_eq!(b"Echo: msg1", &msg[..]),
                1 => assert_eq!(b"Echo: msg2", &msg[..]),
                2 => {
                    assert_eq!(b"Echo: msg3", &msg[..]);
                    handle2.kill(process_id).await.unwrap();
                }
                _ => panic!("Stream should end after killing process"),
            };
            counter += 1;
        }
    });

    handle.write_message(process_id, b"msg3").await.unwrap();
    reader.await.unwrap();
}
