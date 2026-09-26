// c22_e05_boxed_stream_type_alias.rs
//
// CONCEPT: when different code paths produce different stream TYPES, erase
//          the type: `Pin<Box<dyn Stream<Item = ...> + Send + 'static>>`.
// GUIDE:   chapters/22-streams/guide.md  (section "Pin<Box<dyn Stream>>: one type for many streams")
// DATABROKER: databroker/src/grpc/kuksa_val_v2/val.rs —
//             `type SubscribeStream = Pin<Box<dyn Stream<Item = Result<
//             proto::SubscribeResponse, tonic::Status>> + Send + Sync + 'static>>;`
//             and `Ok(tonic::Response::new(Box::pin(stream)))`
// EXPECT:  compile error E0308 (`match` arms have incompatible types)
//
// `impl Stream` means "ONE concrete type the caller can't name". Each branch
// below builds a *different* concrete type, so `impl Stream` can't describe
// them all. A `Box<dyn Stream>` can hold any of them; `Pin` is required
// because a stream may contain self-references once it has been polled
// (see the guide — in practice: `Box::pin(x)` and move on).
// tonic's generated `Val` trait asks for exactly such an associated type.
//
// YOUR TASK
//   Make `subscribe` return `SubscribeStream`, boxing each arm with `Box::pin`.
//   Do not edit the tests.

// I AM NOT DONE

use std::pin::Pin;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::{Stream, StreamExt};

type SubscribeStream = Pin<Box<dyn Stream<Item = Result<String, tonic::Status>> + Send + 'static>>;

enum Source {
    /// Values known up front (e.g. an initial notification).
    Fixed(Vec<String>),
    /// Values arriving later from a task.
    Live(mpsc::Receiver<String>),
    /// The client asked for nothing.
    Nothing,
}

fn subscribe(source: Source) -> impl Stream<Item = Result<String, tonic::Status>> {
    match source {
        Source::Fixed(values) => tokio_stream::iter(values).map(Ok),
        Source::Live(rx) => ReceiverStream::new(rx).map(Ok),
        Source::Nothing => tokio_stream::empty(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_send<T: Send>(_: &T) {}

    #[tokio::test]
    async fn fixed_values() {
        let stream: SubscribeStream = subscribe(Source::Fixed(vec!["a".into(), "b".into()]));
        is_send(&stream);
        let out: Vec<String> = stream.map(|r| r.unwrap()).collect().await;
        assert_eq!(out, vec!["a", "b"]);
    }

    #[tokio::test]
    async fn live_values_from_a_task() {
        let (tx, rx) = mpsc::channel(4);
        tokio::spawn(async move {
            tx.send("x".to_string()).await.unwrap();
        });
        // Tonic moves the stream into its own task, so it must be Send + 'static.
        let stream = subscribe(Source::Live(rx));
        let out = tokio::spawn(async move { stream.collect::<Vec<_>>().await })
            .await
            .unwrap();
        assert_eq!(out.len(), 1);
    }

    #[tokio::test]
    async fn nothing() {
        let stream: SubscribeStream = subscribe(Source::Nothing);
        assert_eq!(stream.collect::<Vec<_>>().await.len(), 0);
    }
}
