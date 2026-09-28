// c37_e04_review_kv_server.rs
//
// CONCEPT: code review — this file COMPILES and mostly works, but has 3
//          planted bugs typical of a first pass at a networked, stateful
//          service: a command parser that's case-sensitive where the spec
//          says it shouldn't be, state that isn't actually shared between
//          connections, and a shutdown that stops accepting new
//          connections but never tells already-open ones to close.
// GUIDE:   chapters/37-capstone-projects/guide.md  (section "Project 1: a
//          key-value server")
// DATABROKER: `SET`/`GET` mirror `PublishValue`/`GetValue`; the shared
//             state mirrors `DataBroker`'s `Arc<RwLock<Database>>`
//             (databroker/src/broker.rs) — cheap to clone, one copy per
//             connection task, all pointing at the same data; shutdown
//             mirrors `grpc/server.rs:95-103` (`shutdown` sends the
//             broker's trigger once) and `main.rs`'s `shutdown_handler`
//             (every long-running task must actually act on that signal,
//             not just the code that decides when to send it).
// EXPECT:  tests fail: `Get k` (mixed case) is reported as an unknown
//          command, a value set by one client is invisible to another,
//          and after shutdown an already-connected client's socket is
//          never closed (the test that checks this times out waiting for
//          end-of-file and fails instead of hanging forever).
//
// THE SPEC (same as the tests)
//   Lines end in `\n`. The command word is case-insensitive; keys and
//   values are case-sensitive. Empty lines are ignored.
//     SET <key> <value>   -> OK       (value = the rest of the line)
//     GET <key>           -> VALUE <value> | NIL
//     QUIT                -> BYE, then the server closes the connection
//     anything else       -> ERR unknown command <WORD>
//   Shutdown: stop accepting new connections, close every open connection,
//   wait for all connection tasks to finish, then return `Ok(())`.
//
// THIS FILE HAS EXACTLY 3 BUGS.
//
// YOUR TASK
//   Find and fix all 3 bugs. For each one, add a one-line comment right
//   above the fix: `// BUG 1: <what was wrong>`, `// BUG 2: ...`,
//   `// BUG 3: ...`.
//   Do not edit the tests.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::broadcast;

type Store = Arc<Mutex<HashMap<String, String>>>;

async fn handle_connection(
    stream: tokio::net::TcpStream,
    store: Store,
    // BUG 3 (part 1 of 2): this connection had no way to learn about
    // shutdown at all, so it could only ever stop when the CLIENT closed
    // the connection or sent `QUIT`. It now holds a `Receiver` and selects
    // on it every time around the loop, exactly like `open_provider_stream`
    // selects on the broker's shutdown trigger.
    mut shutdown: broadcast::Receiver<()>,
) -> anyhow::Result<()> {
    let (read, mut write) = stream.into_split();
    let mut lines = BufReader::new(read).lines();
    loop {
        let line = tokio::select! {
            line = lines.next_line() => line?,
            _ = shutdown.recv() => break,
        };
        let Some(line) = line else { break };
        if line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(2, ' ');
        let command = parts.next().unwrap_or("");
        let rest = parts.next().unwrap_or("");

        // BUG 1: the command word must be case-insensitive ("Get", "get"
        // and "GET" all mean the same thing); only keys and values stay
        // case-sensitive. Upper-case just the command before matching.
        let reply = match command.to_ascii_uppercase().as_str() {
            "SET" => {
                let mut kv = rest.splitn(2, ' ');
                let key = kv.next().unwrap_or("").to_string();
                let value = kv.next().unwrap_or("").to_string();
                store.lock().unwrap().insert(key, value);
                "OK".to_string()
            }
            "GET" => match store.lock().unwrap().get(rest) {
                Some(value) => format!("VALUE {value}"),
                None => "NIL".to_string(),
            },
            "QUIT" => {
                write.write_all(b"BYE\n").await?;
                break;
            }
            _ => format!("ERR unknown command {command}"),
        };
        write.write_all(format!("{reply}\n").as_bytes()).await?;
    }
    Ok(())
}

pub async fn run(
    listener: TcpListener,
    shutdown: impl Future<Output = ()> + Send,
) -> anyhow::Result<()> {
    // BUG 2: this used to be created fresh INSIDE the accept loop, once per
    // connection — every client got its own, empty `HashMap`, so nothing
    // one client set was ever visible to another. One store, shared (via
    // `Arc::clone`) by every connection, the same way every gRPC handler
    // shares one `Arc<RwLock<Database>>`.
    let store: Store = Arc::new(Mutex::new(HashMap::new()));
    let (trigger, _) = broadcast::channel::<()>(1);
    let mut connections = Vec::new();

    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let (stream, _) = accepted?;
                // BUG 3 (part 2 of 2): subscribe BEFORE spawning, same
                // reasoning as chapter 31's `spawn_worker` — a receiver
                // created after the task starts running could miss a
                // shutdown sent in between.
                let rx = trigger.subscribe();
                connections.push(tokio::spawn(handle_connection(stream, Arc::clone(&store), rx)));
            }
            _ = &mut shutdown => break,
        }
    }
    let _ = trigger.send(());
    for connection in connections {
        let _ = connection.await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use std::time::Duration;
    use tokio::io::{BufReader as StdBufReader, Lines};
    use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
    use tokio::net::TcpStream;
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
        lines: Lines<StdBufReader<OwnedReadHalf>>,
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
                lines: StdBufReader::new(read).lines(),
                writer,
            }
        }

        async fn send(&mut self, line: &str) {
            self.writer
                .write_all(format!("{line}\n").as_bytes())
                .await
                .expect("write to server");
        }

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
    async fn commands_are_case_insensitive() {
        let server = start().await;
        let mut c = Client::connect(&server).await;
        assert_eq!(c.ask("set k v").await, "OK");
        assert_eq!(c.ask("Get k").await, "VALUE v");
        assert_eq!(c.ask("get K").await, "NIL", "keys stay case-sensitive");
    }

    #[tokio::test]
    async fn clients_share_state() {
        let server = start().await;
        let mut a = Client::connect(&server).await;
        let mut b = Client::connect(&server).await;
        assert_eq!(a.ask("SET shared 1").await, "OK");
        assert_eq!(
            b.ask("GET shared").await,
            "VALUE 1",
            "a value set by one client must be visible to another"
        );
    }

    #[tokio::test]
    async fn quit_says_bye_and_closes() {
        let server = start().await;
        let mut c = Client::connect(&server).await;
        assert_eq!(c.ask("QUIT").await, "BYE");
        assert_eq!(c.recv().await, None, "expected end-of-file after BYE");
    }

    #[tokio::test]
    async fn shutdown_closes_every_open_connection() {
        let mut server = start().await;
        let mut idle = Client::connect(&server).await;
        assert_eq!(idle.ask("SET k v").await, "OK");

        server
            .stop
            .take()
            .expect("not stopped yet")
            .send(())
            .expect("server still running");

        assert_eq!(
            idle.recv().await,
            None,
            "an already-open connection must be closed on shutdown, not left hanging"
        );
        let result = timeout(PATIENCE, server.task)
            .await
            .expect("run() did not return after shutdown")
            .expect("run() panicked");
        assert!(result.is_ok(), "run() returned {result:?}");
    }
}
