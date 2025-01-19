use std::fmt::Debug;

use proc_heim::{
    manager::{MessageStreamExt, TryMessageStreamExt},
    model::command::Cmd,
};
use serde::{Deserialize, Serialize};
use tokio_stream::StreamExt as _;

use crate::common::create_process_manager;

#[cfg(any(feature = "json", feature = "message-pack"))]
use proc_heim::manager::{serde::MessageFormat, Message};

#[allow(dead_code)]
pub async fn should_read_message<F: Fn(&str) -> Cmd>(cmd_with_message: F) {
    let (_dir, handle) = create_process_manager();
    let msg = "example message";
    let process_id = handle.spawn(cmd_with_message(msg)).await.unwrap();

    let mut stream = handle
        .subscribe_message_stream(process_id)
        .await
        .unwrap()
        .into_string_stream();
    assert_eq!(msg, stream.next().await.unwrap().unwrap());
}

#[allow(dead_code)]
pub async fn should_spawn_process_then_communicate_with_it_then_kill(cmd: Cmd) {
    let (_dir, handle) = create_process_manager();
    let process_id = handle.spawn(cmd).await.unwrap();

    handle.send_message(process_id, b"msg1").await.unwrap();
    handle.send_message(process_id, b"msg2").await.unwrap();

    let msg3 = b"\x95\xa0\x90\xca\xc2"; // extended ASCII codes

    let handle2 = handle.clone();
    let reader = tokio::spawn(async move {
        let mut stream = handle2
            .subscribe_message_stream(process_id)
            .await
            .unwrap()
            .ignore_lost_messages()
            .into_bytes_stream();
        let mut counter = 0;
        while let Some(msg) = stream.next().await {
            match counter {
                0 => assert_eq!(b"msg1", &msg[..]),
                1 => assert_eq!(b"msg2", &msg[..]),
                2 => {
                    assert_eq!(msg3, &msg[..]);
                    handle2.kill(process_id).await.unwrap();
                }
                _ => panic!("Stream should end after killing process"),
            };
            counter += 1;
        }
    });

    handle.send_message(process_id, msg3).await.unwrap();
    reader.await.unwrap();
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Default)]
pub struct ExampleMessage {
    pub data1: String,
    pub data2: Vec<u8>,
    pub data3: i32,
    pub data4: f32,
    pub data5: bool,
}

impl TryFrom<Vec<u8>> for ExampleMessage {
    type Error = serde_json::error::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value)
    }
}

impl TryInto<Vec<u8>> for ExampleMessage {
    type Error = serde_json::error::Error;

    fn try_into(self) -> Result<Vec<u8>, Self::Error> {
        serde_json::to_vec(&self)
    }
}

#[cfg(any(feature = "json", feature = "message-pack"))]
#[allow(dead_code)]
pub async fn should_read_message_with_format(
    cmd: Cmd,
    message: ExampleMessage,
    format: MessageFormat,
) {
    let (_dir, handle) = create_process_manager();
    let process_id = handle.spawn(cmd).await.unwrap();

    handle
        .send_message(
            process_id,
            Message::from_serializable(&message, &format).unwrap(),
        )
        .await
        .unwrap();

    let mut stream = handle
        .subscribe_message_stream(process_id)
        .await
        .unwrap()
        .into_deserialized_stream(&format);
    let actual_message = stream.try_next().await.unwrap().unwrap();
    assert_eq!(message, actual_message);
}

#[cfg(any(feature = "json", feature = "message-pack"))]
#[allow(dead_code)]
pub async fn should_write_json_message_and_read_part_of_it<F: Fn(&str) -> Cmd>(cmd_with_arg: F) {
    let (_dir, handle) = create_process_manager();
    let process_id = handle.spawn(cmd_with_arg(".data1")).await.unwrap();

    let data1 = "Ye e  e ee ah";
    let message = ExampleMessage {
        data1: data1.into(),
        ..Default::default()
    };

    handle
        .send_message(
            process_id,
            Message::from_serializable(&message, &MessageFormat::Json).unwrap(),
        )
        .await
        .unwrap();

    let mut stream = handle
        .subscribe_message_stream(process_id)
        .await
        .unwrap()
        .into_string_stream();
    let message = stream.try_next().await.unwrap().unwrap();
    assert_eq!(data1, message);
}
