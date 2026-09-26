// c37_e01_kv_server.rs
//
// CONCEPT: PROJECT, no scaffolding. A networked, concurrent service built
//          from scratch: tokio TCP, per-connection tasks, shared state,
//          publish/subscribe, graceful shutdown.
// GUIDE:   chapters/37-capstone-projects/guide.md  (section "Project 1: a key-value server")
// DATABROKER: the whole databroker in miniature. `SET` = PublishValue,
//             `SUBSCRIBE` = Subscribe (broker.rs `subscribe` + broadcast),
//             shutdown = grpc/server.rs `shutdown` + the broker's shutdown trigger.
// EXPECT:  tests fail: `todo!()` panics
//
// THE SPEC (the tests pin every line of it)
//   `run(listener, shutdown)` serves clients on `listener` until `shutdown`
//   completes. Each client sends commands as lines ending in `\n`. The
//   command word is case-insensitive; keys and values are case-sensitive.
//   Empty lines are ignored. Every reply is exactly one line.
//
//     SET <key> <value>   -> OK            value = the rest of the line (may contain spaces)
//     GET <key>           -> VALUE <value> | NIL
//     SUBSCRIBE <key>     -> OK, and from then on the connection also receives
//                            `UPDATE <key> <value>` for every SET of that key by
//                            ANY client (itself included), in SET order.
//                            Only SETs made after the OK are reported.
//     QUIT                -> BYE, then the server closes the connection
//     anything else       -> ERR unknown command <WORD>   (WORD as sent)
//     wrong arguments     -> ERR usage: SET <key> <value>
//                            ERR usage: GET <key>
//                            ERR usage: SUBSCRIBE <key>
//
//   Shutdown: when `shutdown` completes, stop accepting connections, close
//   every open connection (clients read end-of-file), wait for all
//   connection tasks to finish, and return `Ok(())`.
//
// YOUR TASK
//   Design and implement it. Only `run`'s signature is fixed: add any
//   types, helpers and tasks you want. Do not edit the tests.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, watch};
use tokio::task::JoinSet;

/// One published change. `seq` increases with every SET, so a connection can
/// ignore changes that happened before it subscribed.
#[derive(Clone, Debug)]
struct Change {
    seq: u64,
    key: String,
    value: String,
}

#[derive(Default)]
struct Store {
    values: HashMap<String, String>,
    seq: u64,
}

#[derive(Clone)]
struct Shared {
    store: Arc<Mutex<Store>>,
    changes: broadcast::Sender<Change>,
}

impl Shared {
    fn set(&self, key: &str, value: &str) {
        // WHY: bump `seq`, store and publish under one lock, so the order of
        // UPDATEs matches the order of SETs, and a SUBSCRIBE that reads `seq`
        // under the same lock gets a clean "before/after" cut. A std Mutex
        // is fine: it's never held across an `.await`.
        let mut store = self.store.lock().expect("store poisoned");
        store.seq += 1;
        store.values.insert(key.to_string(), value.to_string());
        let _ = self.changes.send(Change {
            seq: store.seq,
            key: key.to_string(),
            value: value.to_string(),
        });
    }

    fn get(&self, key: &str) -> Option<String> {
        self.store
            .lock()
            .expect("store poisoned")
            .values
            .get(key)
            .cloned()
    }

    fn current_seq(&self) -> u64 {
        self.store.lock().expect("store poisoned").seq
    }
}

enum Reply {
    Line(String),
    Close(String),
}

pub async fn run(
    listener: TcpListener,
    shutdown: impl Future<Output = ()> + Send,
) -> anyhow::Result<()> {
    let (changes, _) = broadcast::channel(1024);
    let shared = Shared {
        store: Arc::default(),
        changes,
    };
    let (stop_tx, stop_rx) = watch::channel(false);
    let mut connections = JoinSet::new();

    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            () = &mut shutdown => break,
            accepted = listener.accept() => {
                let (socket, _) = accepted?;
                connections.spawn(serve(socket, shared.clone(), stop_rx.clone()));
            }
        }
    }

    drop(listener);
    let _ = stop_tx.send(true);
    while connections.join_next().await.is_some() {}
    Ok(())
}

async fn serve(socket: TcpStream, shared: Shared, mut stop: watch::Receiver<bool>) {
    let (read_half, mut write_half) = socket.into_split();
    let mut lines = BufReader::new(read_half).lines();
    // Subscribe to the change feed up front; `subscribed` decides which
    // changes this connection actually reports (key -> seq at SUBSCRIBE time).
    let mut feed = shared.changes.subscribe();
    let mut subscribed: HashMap<String, u64> = HashMap::new();

    loop {
        let reply = tokio::select! {
            _ = stop.changed() => return,
            line = lines.next_line() => match line {
                Ok(Some(line)) => handle(&line, &shared, &mut subscribed),
                Ok(None) | Err(_) => return,
            },
            change = feed.recv() => match change {
                Ok(change) => match subscribed.get(&change.key) {
                    Some(&since) if change.seq > since => {
                        Some(Reply::Line(format!("UPDATE {} {}", change.key, change.value)))
                    }
                    _ => None,
                },
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    Some(Reply::Line(format!("ERR lagged {n}")))
                }
                Err(broadcast::error::RecvError::Closed) => return,
            },
        };
        match reply {
            None => {}
            Some(Reply::Line(text)) => {
                if write_line(&mut write_half, &text).await.is_err() {
                    return;
                }
            }
            Some(Reply::Close(text)) => {
                let _ = write_line(&mut write_half, &text).await;
                return;
            }
        }
    }
}

async fn write_line(
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    text: &str,
) -> std::io::Result<()> {
    writer.write_all(text.as_bytes()).await?;
    writer.write_all(b"\n").await
}

/// Pure command handling: no I/O, so it's easy to reason about (and to test).
fn handle(line: &str, shared: &Shared, subscribed: &mut HashMap<String, u64>) -> Option<Reply> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let (word, rest) = match line.split_once(char::is_whitespace) {
        Some((word, rest)) => (word, rest.trim_start()),
        None => (line, ""),
    };
    let reply = match word.to_ascii_uppercase().as_str() {
        "SET" => match rest.split_once(char::is_whitespace) {
            Some((key, value)) if !value.trim().is_empty() => {
                shared.set(key, value.trim_start());
                "OK".to_string()
            }
            _ => "ERR usage: SET <key> <value>".to_string(),
        },
        "GET" => match single_arg(rest) {
            Some(key) => shared
                .get(key)
                .map_or_else(|| "NIL".to_string(), |value| format!("VALUE {value}")),
            None => "ERR usage: GET <key>".to_string(),
        },
        "SUBSCRIBE" => match single_arg(rest) {
            Some(key) => {
                subscribed.insert(key.to_string(), shared.current_seq());
                "OK".to_string()
            }
            None => "ERR usage: SUBSCRIBE <key>".to_string(),
        },
        "QUIT" => return Some(Reply::Close("BYE".to_string())),
        _ => format!("ERR unknown command {word}"),
    };
    Some(Reply::Line(reply))
}

fn single_arg(rest: &str) -> Option<&str> {
    let mut parts = rest.split_whitespace();
    match (parts.next(), parts.next()) {
        (Some(arg), None) => Some(arg),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use std::time::Duration;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
    use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
    use tokio::sync::oneshot;
    use tokio::task::JoinHandle;
    use tokio::time::timeout;

    const PATIENCE: Duration = Duration::from_secs(5);

    struct Server {
        addr: SocketAddr,
        stop: Option<oneshot::Sender<()>>,
        task: JoinHandle<anyhow::Result<()>>,
    }

    async fn start() -> Server {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind loopback");
        let addr = listener.local_addr().expect("local addr");
        let (stop, stopped) = oneshot::channel::<()>();
        let task = tokio::spawn(run(listener, async move {
            let _ = stopped.await;
        }));
        Server {
            addr,
            stop: Some(stop),
            task,
        }
    }

    struct Client {
        lines: Lines<BufReader<OwnedReadHalf>>,
        writer: OwnedWriteHalf,
    }

    impl Client {
        async fn connect(server: &Server) -> Client {
            let stream = timeout(PATIENCE, TcpStream::connect(server.addr))
                .await
                .expect("connect timed out")
                .expect("connect");
            let (read, writer) = stream.into_split();
            Client {
                lines: BufReader::new(read).lines(),
                writer,
            }
        }

        async fn send(&mut self, line: &str) {
            self.writer
                .write_all(format!("{line}\n").as_bytes())
                .await
                .expect("write to server");
        }

        /// Next line from the server, or None on end-of-file.
        async fn recv(&mut self) -> Option<String> {
            timeout(PATIENCE, self.lines.next_line())
                .await
                .expect("timed out waiting for the server")
                .expect("read from server")
        }

        async fn ask(&mut self, line: &str) -> String {
            self.send(line).await;
            self.recv().await.expect("server closed the connection")
        }
    }

    #[tokio::test]
    async fn set_then_get() {
        let server = start().await;
        let mut c = Client::connect(&server).await;
        assert_eq!(c.ask("SET Vehicle.Speed 42").await, "OK");
        assert_eq!(c.ask("GET Vehicle.Speed").await, "VALUE 42");
        assert_eq!(c.ask("GET Vehicle.Missing").await, "NIL");
        assert_eq!(c.ask("SET Vehicle.Name my  little car").await, "OK");
        assert_eq!(c.ask("GET Vehicle.Name").await, "VALUE my  little car");
    }

    #[tokio::test]
    async fn commands_are_case_insensitive_and_errors_are_explained() {
        let server = start().await;
        let mut c = Client::connect(&server).await;
        assert_eq!(c.ask("set k v").await, "OK");
        assert_eq!(c.ask("Get k").await, "VALUE v");
        assert_eq!(c.ask("FROB k").await, "ERR unknown command FROB");
        assert_eq!(c.ask("GET").await, "ERR usage: GET <key>");
        assert_eq!(c.ask("GET a b").await, "ERR usage: GET <key>");
        assert_eq!(c.ask("SET k").await, "ERR usage: SET <key> <value>");
        assert_eq!(c.ask("SUBSCRIBE").await, "ERR usage: SUBSCRIBE <key>");
        c.send("").await; // ignored: no reply
        assert_eq!(c.ask("GET k").await, "VALUE v");
    }

    #[tokio::test]
    async fn clients_share_state() {
        let server = start().await;
        let mut a = Client::connect(&server).await;
        let mut b = Client::connect(&server).await;
        assert_eq!(a.ask("SET shared 1").await, "OK");
        assert_eq!(b.ask("GET shared").await, "VALUE 1");
    }

    #[tokio::test]
    async fn subscribers_get_later_updates_for_their_keys_only() {
        let server = start().await;
        let mut publisher = Client::connect(&server).await;
        let mut subscriber = Client::connect(&server).await;

        assert_eq!(publisher.ask("SET speed 10").await, "OK"); // before SUBSCRIBE
        assert_eq!(subscriber.ask("SUBSCRIBE speed").await, "OK");
        assert_eq!(publisher.ask("SET other 1").await, "OK"); // different key
        assert_eq!(publisher.ask("SET speed 20").await, "OK");
        assert_eq!(publisher.ask("SET speed 30").await, "OK");

        assert_eq!(subscriber.recv().await.as_deref(), Some("UPDATE speed 20"));
        assert_eq!(subscriber.recv().await.as_deref(), Some("UPDATE speed 30"));
        // A subscriber's own SET is reported to it too, after the OK.
        assert_eq!(subscriber.ask("SET speed 40").await, "OK");
        assert_eq!(subscriber.recv().await.as_deref(), Some("UPDATE speed 40"));
    }

    #[tokio::test]
    async fn quit_says_bye_and_closes() {
        let server = start().await;
        let mut c = Client::connect(&server).await;
        assert_eq!(c.ask("QUIT").await, "BYE");
        assert_eq!(c.recv().await, None, "expected end-of-file after BYE");
    }

    #[tokio::test]
    async fn many_clients_at_once() {
        let server = start().await;
        let mut tasks = Vec::new();
        for i in 0..20 {
            let mut c = Client::connect(&server).await;
            tasks.push(tokio::spawn(async move {
                assert_eq!(c.ask(&format!("SET key{i} {i}")).await, "OK");
            }));
        }
        for task in tasks {
            task.await.expect("client task");
        }
        let mut c = Client::connect(&server).await;
        for i in 0..20 {
            assert_eq!(c.ask(&format!("GET key{i}")).await, format!("VALUE {i}"));
        }
    }

    #[tokio::test]
    async fn shutdown_closes_every_connection_and_returns_ok() {
        let mut server = start().await;
        let mut idle = Client::connect(&server).await;
        let mut subscribed = Client::connect(&server).await;
        assert_eq!(idle.ask("SET k v").await, "OK");
        assert_eq!(subscribed.ask("SUBSCRIBE k").await, "OK");

        server
            .stop
            .take()
            .expect("not stopped yet")
            .send(())
            .expect("server still running");

        assert_eq!(
            idle.recv().await,
            None,
            "idle client not closed on shutdown"
        );
        assert_eq!(
            subscribed.recv().await,
            None,
            "subscriber not closed on shutdown"
        );
        let result = timeout(PATIENCE, server.task)
            .await
            .expect("run() did not return after shutdown")
            .expect("run() panicked");
        assert!(result.is_ok(), "run() returned {result:?}");
    }
}
