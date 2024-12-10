use std::{
    io::{self},
    path::PathBuf,
};

use tokio::{
    fs::File,
    io::{AsyncBufReadExt as _, BufReader},
    sync::{mpsc, oneshot},
};
use tokio_stream::{wrappers::LinesStream, StreamExt as TokioStreamExt};

#[derive(Debug, Default)]
pub struct LogsQuery {
    offset: usize,
    limit: Option<usize>,
}

impl LogsQuery {
    pub fn new(offset: Option<usize>, limit: Option<usize>) -> Self {
        Self {
            offset: offset.unwrap_or(0),
            limit,
        }
    }
}

#[derive(Debug)]
pub(crate) enum LogsQueryType {
    Stdout,
    Stderr,
}

impl ToString for LogsQueryType {
    fn to_string(&self) -> String {
        match self {
            LogsQueryType::Stdout => "STDOUT",
            LogsQueryType::Stderr => "STDERR",
        }
        .into()
    }
}

#[allow(dead_code)] // used in spawner tests
pub(crate) enum LogSettingsQuery {
    Stdout,
    Stderr,
    Merged,
}

enum LogReaderCommand {
    GetLogs {
        logs_query_type: LogsQueryType,
        query: LogsQuery,
        responder: oneshot::Sender<Result<Vec<String>, LogReaderError>>,
    },
    Abort,
    CheckSettings {
        query: LogSettingsQuery,
        responder: oneshot::Sender<bool>,
    },
}

pub struct LogReader {
    logs_stdout: Option<PathBuf>,
    logs_stderr: Option<PathBuf>,
    logs_merged: Option<PathBuf>,
    receiver: mpsc::Receiver<LogReaderCommand>,
}

impl LogReader {
    pub fn new(
        logs_stdout: Option<PathBuf>,
        logs_stderr: Option<PathBuf>,
        logs_merged: Option<PathBuf>,
    ) -> LogReaderHandle {
        let (sender, receiver) = mpsc::channel(32);
        let mut reader = Self {
            logs_stdout,
            logs_stderr,
            logs_merged,
            receiver,
        };
        tokio::spawn(async move { reader.run().await });
        LogReaderHandle::new(sender)
    }

    async fn run(&mut self) {
        while let Some(command) = self.receiver.recv().await {
            match command {
                LogReaderCommand::GetLogs {
                    logs_query_type,
                    query,
                    responder,
                } => {
                    let result = self.get_logs(logs_query_type, query).await;
                    let _ = responder.send(result);
                }
                LogReaderCommand::CheckSettings { query, responder } => {
                    let response = self.is_log_type_set(&query);
                    let _ = responder.send(response);
                }
                LogReaderCommand::Abort => break,
            }
        }
    }

    async fn get_logs(
        &self,
        logs_query_type: LogsQueryType,
        query: LogsQuery,
    ) -> Result<Vec<String>, LogReaderError> {
        let log_file_path = self
            .get_log_file_path(&logs_query_type)
            .ok_or(LogReaderError::LogTypeWasNotConfigured(logs_query_type))?;

        let file = File::open(log_file_path).await?;
        let reader = BufReader::new(file);

        if let Some(limit) = query.limit {
            Self::read_logs(reader, query.offset, limit).await
        } else {
            Self::read_logs_to_end(reader, query.offset).await
        }
        .map_err(Into::into)
    }

    fn get_log_file_path(&self, logs_query_type: &LogsQueryType) -> Option<&PathBuf> {
        match logs_query_type {
            LogsQueryType::Stdout => self.logs_stdout.as_ref(),
            LogsQueryType::Stderr => self.logs_stderr.as_ref(),
        }
        .or(self.logs_merged.as_ref())
    }

    async fn read_logs_to_end(reader: BufReader<File>, offset: usize) -> io::Result<Vec<String>> {
        let logs = LinesStream::new(reader.lines())
            .skip(offset)
            .then(|result| async move { result.ok() })
            .filter_map(|line_opt| line_opt)
            .collect()
            .await;
        Ok(logs)
    }

    async fn read_logs(
        reader: BufReader<File>,
        offset: usize,
        limit: usize,
    ) -> io::Result<Vec<String>> {
        let logs = LinesStream::new(reader.lines())
            .skip(offset)
            .take(limit)
            .then(|result| async move { result.ok() })
            .filter_map(|line_opt| line_opt)
            .collect()
            .await;
        Ok(logs)
    }

    fn is_log_type_set(&self, query: &LogSettingsQuery) -> bool {
        match query {
            LogSettingsQuery::Stdout => self.logs_stdout.as_ref(),
            LogSettingsQuery::Stderr => self.logs_stderr.as_ref(),
            LogSettingsQuery::Merged => self.logs_merged.as_ref(),
        }
        .is_some()
    }
}

#[derive(Debug)]
pub(crate) struct LogReaderHandle {
    sender: mpsc::Sender<LogReaderCommand>,
}

impl LogReaderHandle {
    fn new(sender: mpsc::Sender<LogReaderCommand>) -> Self {
        Self { sender }
    }

    pub async fn read_logs(
        &self,
        logs_query_type: LogsQueryType,
        query: LogsQuery,
    ) -> Result<Vec<String>, LogReaderError> {
        let (responder, receiver) = oneshot::channel();
        let cmd = LogReaderCommand::GetLogs {
            logs_query_type,
            query,
            responder,
        };
        let _ = self.sender.send(cmd).await;
        match receiver.await {
            Ok(result) => result,
            Err(_) => Ok(vec![]), // ignore error, LogReader is killed
        }
    }

    pub async fn abort(&self) {
        let _ = self.sender.send(LogReaderCommand::Abort).await;
    }

    #[allow(dead_code)] // used in spawner tests
    pub(crate) async fn check_logs_settings(
        &self,
        query: LogSettingsQuery,
    ) -> Result<bool, oneshot::error::RecvError> {
        let (responder, receiver) = oneshot::channel();
        let cmd: LogReaderCommand = LogReaderCommand::CheckSettings { query, responder };
        let _ = self.sender.send(cmd).await;
        receiver.await
    }
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum LogReaderError {
    #[error("Log type was not configured for process with id: {0:?}")]
    LogTypeWasNotConfigured(LogsQueryType),
    #[error(transparent)]
    UnExpectedIoError(#[from] io::Error),
}
