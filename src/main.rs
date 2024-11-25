use std::{fs, path::PathBuf, str::FromStr, time::Duration};

use proc_heim::{CmdBuilder, ProcessManager};
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let working_dir = PathBuf::from_str("./tmp").unwrap();
    fs::create_dir_all(&working_dir).unwrap();
    let handle = ProcessManager::new(working_dir);
    let cmd = CmdBuilder::default()
        .cmd("bash".into())
        .args(vec!["-C".into(), "./scripts/echo.sh".into()].into())
        .build()
        .unwrap();
    let process_id = handle.spawn(cmd).await?;
    tokio::time::sleep(Duration::from_secs(1)).await;

    handle.write_message(process_id, b"q\n").await?;

    let handle2 = handle.clone();
    let task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(3)).await;
        dbg!("Here23");
        handle2.write_message(process_id, b"s\n").await.unwrap();
        dbg!("Here");
    });

    let handle3 = handle.clone();
    let reader = tokio::spawn(async move {
        let mut stream = handle3.subscribe_message_stream(process_id).await.unwrap();
        let mut counter = 0;
        while let Some(msg) = stream.next().await {
            counter += 1;
            dbg!(format!("Get: {}", String::from_utf8(msg).unwrap()));
            if counter >= 3 {
                dbg!("Call kill");
                handle3.kill(process_id).await.unwrap();
            }
        }
    });

    task.await?;

    handle.write_message(process_id, b"ooo\n").await?;

    reader.await?;
    Ok(())
}
