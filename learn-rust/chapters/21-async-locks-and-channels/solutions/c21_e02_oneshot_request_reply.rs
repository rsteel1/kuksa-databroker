// c21_e02_oneshot_request_reply.rs
//
// CONCEPT: request/reply over channels — an "actor" task owns its state and
//          answers requests through a `oneshot` reply channel.
// GUIDE:   chapters/21-async-locks-and-channels/guide.md  (section "oneshot: exactly one reply")
// DATABROKER: databroker/src/grpc/kuksa_val_v2/val.rs — the broker asks a
//             provider for values by sending `GetProviderValueRequest` down one
//             channel and waiting for the matching response on another
// EXPECT:  tests fail: `todo!()` panics
//
// Instead of sharing a HashMap behind a lock, give it to ONE task and talk to
// that task with messages. Each "get" request carries its own
// `oneshot::Sender` so the actor can reply to exactly that caller.
//
// YOUR TASK
//   Implement `run_store` (the actor loop) and `get` (the client helper).
//   Do not edit the tests.

use std::collections::HashMap;
use tokio::sync::{mpsc, oneshot};

enum Command {
    Set {
        path: String,
        value: i32,
    },
    Get {
        path: String,
        reply: oneshot::Sender<Option<i32>>,
    },
}

/// Owns the values. Runs until every `Sender<Command>` is dropped.
async fn run_store(mut commands: mpsc::Receiver<Command>) {
    let mut values: HashMap<String, i32> = HashMap::new();
    while let Some(command) = commands.recv().await {
        match command {
            Command::Set { path, value } => {
                values.insert(path, value);
            }
            Command::Get { path, reply } => {
                // WHY `let _ =`: if the caller gave up (dropped its receiver)
                // there is nobody to answer — that's not the actor's problem.
                let _ = reply.send(values.get(&path).copied());
            }
        }
    }
}

/// Ask the store for `path`. Returns `None` if missing OR if the store is gone.
async fn get(store: &mpsc::Sender<Command>, path: &str) -> Option<i32> {
    let (reply, response) = oneshot::channel();
    store
        .send(Command::Get {
            path: path.to_string(),
            reply,
        })
        .await
        .ok()?;
    response.await.ok().flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn set_then_get() {
        let (tx, rx) = mpsc::channel(8);
        let actor = tokio::spawn(run_store(rx));

        tx.send(Command::Set {
            path: "Vehicle.Speed".into(),
            value: 88,
        })
        .await
        .unwrap();
        assert_eq!(get(&tx, "Vehicle.Speed").await, Some(88));
        assert_eq!(get(&tx, "Vehicle.Unknown").await, None);

        drop(tx);
        actor.await.unwrap(); // the actor stops once all senders are gone
    }

    #[tokio::test]
    async fn store_gone_means_none() {
        let (tx, rx) = mpsc::channel(8);
        drop(rx);
        assert_eq!(get(&tx, "Vehicle.Speed").await, None);
    }
}
