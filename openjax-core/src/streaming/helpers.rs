use std::future::Future;

use anyhow::Result;
use openjax_protocol::Event;
use tokio::sync::mpsc::UnboundedReceiver;

pub(crate) fn emit_synthetic_response_deltas(
    turn_id: u64,
    message: &str,
    chunk_chars: usize,
) -> Vec<Event> {
    if message.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::new();
    let mut chunk = String::new();
    let mut chunk_len = 0usize;

    for ch in message.chars() {
        chunk.push(ch);
        chunk_len += 1;
        if chunk_len >= chunk_chars {
            out.push(Event::ResponseTextDelta {
                turn_id,
                content_delta: chunk.clone(),
                stream_source: openjax_protocol::StreamSource::Synthetic,
            });
            chunk.clear();
            chunk_len = 0;
        }
    }

    if !chunk.is_empty() {
        out.push(Event::ResponseTextDelta {
            turn_id,
            content_delta: chunk,
            stream_source: openjax_protocol::StreamSource::Synthetic,
        });
    }

    out
}

pub(crate) async fn run_stream_with_delta_handler<T, Fut, F>(
    mut delta_rx: UnboundedReceiver<crate::model::StreamDelta>,
    stream_future: Fut,
    mut on_delta: F,
) -> Result<T>
where
    Fut: Future<Output = Result<T>>,
    F: FnMut(crate::model::StreamDelta),
{
    tokio::pin!(stream_future);

    let stream_result = loop {
        tokio::select! {
            delta = delta_rx.recv() => {
                if let Some(delta) = delta {
                    on_delta(delta);
                }
            }
            result = &mut stream_future => {
                break result;
            }
        }
    };

    while let Ok(delta) = delta_rx.try_recv() {
        on_delta(delta);
    }

    stream_result
}

#[cfg(test)]
mod tests {
    use crate::model::StreamDelta;
    use anyhow::anyhow;
    use openjax_protocol::Event;

    use super::{emit_synthetic_response_deltas, run_stream_with_delta_handler};

    #[test]
    fn synthetic_deltas_return_empty_for_empty_message() {
        let events = emit_synthetic_response_deltas(1, "", 4);
        assert!(events.is_empty());
    }

    #[test]
    fn synthetic_deltas_split_by_chunk_size() {
        let events = emit_synthetic_response_deltas(2, "abcdefg", 3);
        assert_eq!(events.len(), 3);
        assert!(matches!(
            events[0],
            Event::ResponseTextDelta { ref content_delta, .. } if content_delta == "abc"
        ));
        assert!(matches!(
            events[1],
            Event::ResponseTextDelta { ref content_delta, .. } if content_delta == "def"
        ));
        assert!(matches!(
            events[2],
            Event::ResponseTextDelta { ref content_delta, .. } if content_delta == "g"
        ));
    }

    #[tokio::test]
    async fn stream_helper_drains_buffered_deltas_after_stream_finishes() {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        tx.send(StreamDelta::Text("A".to_string())).expect("send A");
        tx.send(StreamDelta::Text("B".to_string())).expect("send B");
        drop(tx);

        let mut observed = String::new();
        let output =
            run_stream_with_delta_handler(rx, async { Ok::<_, anyhow::Error>("ok") }, |d| {
                if let StreamDelta::Text(text) = d {
                    observed.push_str(&text);
                }
            })
            .await
            .expect("stream result");

        assert_eq!(output, "ok");
        assert_eq!(observed, "AB");
    }

    #[tokio::test]
    async fn stream_helper_returns_error_while_preserving_seen_deltas() {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        tx.send(StreamDelta::Text("X".to_string())).expect("send X");
        drop(tx);

        let mut observed = String::new();
        let result =
            run_stream_with_delta_handler(rx, async { Err::<(), _>(anyhow!("boom")) }, |d| {
                if let StreamDelta::Text(text) = d {
                    observed.push_str(&text);
                }
            })
            .await;

        assert!(result.is_err());
        assert_eq!(observed, "X");
    }
}
