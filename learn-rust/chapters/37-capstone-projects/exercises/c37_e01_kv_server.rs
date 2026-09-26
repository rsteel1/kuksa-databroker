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

// I AM NOT DONE

#![allow(unused_imports)]
use std::future::Future;
use tokio::net::{TcpListener, TcpStream};

// Design it: shared state, a way to publish changes, a way to tell every
// connection to stop, and one task per connection.

pub async fn run(
    listener: TcpListener,
    shutdown: impl Future<Output = ()> + Send,
) -> anyhow::Result<()> {
    let _ = (listener, shutdown);
    todo!()
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
