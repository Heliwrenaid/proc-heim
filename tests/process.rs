mod common;

use proc_heim::CmdBuilder;
use tokio_stream::StreamExt;

use crate::common::{create_process_manager, echo_script};

#[tokio::test]
async fn should_spawn_process_communicate_with_it_and_kill() {
    let (_dir, handle) = create_process_manager();
    let cmd = CmdBuilder::default()
        .cmd("bash".into())
        .args(vec!["-C".into(), echo_script()].into())
        .build()
        .unwrap();
    let process_id = handle.spawn(cmd).await.unwrap();

    handle.write_message(process_id, b"msg1").await.unwrap();
    handle.write_message(process_id, b"msg2").await.unwrap();

    let handle2: proc_heim::ProcessManagerHandle = handle.clone();
    let reader = tokio::spawn(async move {
        let mut stream = handle2.subscribe_message_stream(process_id).await.unwrap();
        let mut counter = 0;
        while let Some(msg) = stream.next().await {
            dbg!(&msg);
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
