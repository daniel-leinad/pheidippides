use std::time::Duration;
use anyhow::Context;

use tokio::net::TcpStream;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::io::{AsyncWriteExt, AsyncBufReadExt, BufReader, BufWriter};

use crate::http_response::HttpResponseBuilder;

const KEEP_ALIVE_CHECK_INTERVAL: Duration = Duration::from_secs(3600);

#[derive(Debug)]
pub struct EventSourceEvent {
    pub data: String,
    pub id: String,
    pub event: Option<String>,
}

pub async fn handle_event_stream(mut tcp_stream: TcpStream, retry: Option<i32>, event_stream: &mut UnboundedReceiver<EventSourceEvent>) -> anyhow::Result<()> {
    let http_response = HttpResponseBuilder::new()
        .content_event_stream()
        .build();

    let (reader, writer) = tcp_stream.split();
    let mut reader = BufReader::new(reader);
    let mut writer = BufWriter::new(writer);

    writer.write_all(&http_response.into_bytes()).await?;

    if let Some(retry_value) = retry {
        writer.write_all(&format!("retry: {retry_value}\n").as_bytes()).await?;
    };

    writer.flush().await?;

    loop {
        let mut read_buf = String::new();
        tokio::select! {
            _ = tokio::time::sleep(KEEP_ALIVE_CHECK_INTERVAL) => {
                writer.write_all(b": keep-alive\n").await.context("Keep-alive failed")?;
                writer.flush().await.context("Keep-alive failed")?
            },

            _ = reader.read_line(&mut read_buf) => {
                // Either client disconnected or something went wrong, either way stop the subscription
                break;
            },

            next_event = event_stream.recv() => {
                match next_event {
                    Some(event) => {
                        send_event_to_event_source_stream(&mut writer, event).await.context("Send event failed")?;
                    },
                    None => {
                        break
                    },
                }
            },
        }

    }

    Ok(())
}

async fn send_event_to_event_source_stream<T: AsyncWriteExt + Unpin>(writer: &mut BufWriter<T>, event: EventSourceEvent) -> anyhow::Result<()> {
    let mut response_str = String::new();
    if let Some(event_type) = event.event {
        response_str.push_str(&format!("event: {}\n", event_type));
    }
    for line in event.data.lines() {
        response_str.push_str(&format!("data: {line}\n"))
    };
    response_str.push_str(&format!("id: {}\n", event.id));
    response_str.push_str("\n");

    writer.write_all(response_str.as_bytes()).await?;
    writer.flush().await?;

    Ok(())
}
