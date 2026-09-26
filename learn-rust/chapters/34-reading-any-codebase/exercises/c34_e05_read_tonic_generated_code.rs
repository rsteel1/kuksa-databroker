// c34_e05_read_tonic_generated_code.rs
//
// CONCEPT: reading code that no human wrote: the Rust that `tonic-prost-build`
//          generates from a `.proto` file at build time, and rebuilding its
//          shape (a trait you implement + a `tower::Service` that routes
//          requests by path to your trait methods).
// GUIDE:   chapters/34-reading-any-codebase/guide.md  (section "Reading macro-generated and build-generated code")
// DATABROKER: databroker/src/grpc/kuksa_val_v2/val.rs — `impl proto::val_server::Val
//             for broker::DataBroker`; databroker/src/grpc/server.rs — `ValServer::with_interceptor`.
// EXPECT:  tests fail: `todo!()` panics
//
// READ FIRST: the generated file. From learn-rust/ run
//     find target -name kuksa.val.v2.rs -path '*databroker-proto*'
// (several identical copies may exist, one per build configuration). In it,
// jump to `pub mod val_server`. Find: the trait `Val` and the attribute on it;
// the `type ...Stream` associated types; `pub struct ValServer<T>`; the
// `impl<T, B> tonic::codegen::Service<http::Request<B>> for ValServer<T>` with
// its `poll_ready` and `call`; the `match req.uri().path()` routing table; the
// `_ =>` fallback; and `SERVICE_NAME` at the end. Compare it with
// databroker-proto/proto/kuksa/val/v2/val.proto (via the `proto` symlink).
//
// YOUR TASK
//   1. Fill in `answers()` from what you read. The tests check each answer
//      against the generated file itself.
//   2. Implement the mini generated server (`GreeterServer`) with the same
//      shape as `ValServer`. Do not edit the tests.

// I AM NOT DONE

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

// ---- Part 1: questions about the generated code ------------------------------

#[derive(Debug, PartialEq, Eq)]
pub struct Answers {
    /// The value of `SERVICE_NAME` in `val_server`.
    pub service_name: &'static str,
    /// Every route in `ValServer::call`, in the order they appear,
    /// e.g. "/kuksa.val.v2.VAL/GetValue".
    pub routes: Vec<&'static str>,
    /// Methods whose response is a server stream: those with a
    /// `type <Method>Stream` associated type in the `Val` trait (method
    /// names in PascalCase, in order).
    pub server_streaming_methods: Vec<&'static str>,
    /// The attribute macro on `pub trait Val` (without `#[` and `]`).
    pub trait_attribute: &'static str,
    /// `type Error = ...;` in `ValServer`'s `Service` impl, verbatim.
    pub service_error_type: &'static str,
    /// The gRPC status code (as a number) the `_ =>` fallback returns.
    pub unknown_route_code: i32,
}

pub fn answers() -> Answers {
    todo!()
}

// ---- Part 2: a mini generated server -------------------------------------------

/// What arrives from the network (the real code uses `http::Request<B>`).
#[derive(Debug, Clone)]
pub struct RawRequest {
    pub path: String,
    pub body: String,
}

/// What goes back (the real code sets the `grpc-status` header).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawResponse {
    /// 0 = OK, otherwise a `tonic::Code` as i32.
    pub grpc_status: i32,
    /// The response message on success, the status message on error.
    pub body: String,
}

/// Same alias the generated code uses (`tonic::codegen::BoxFuture`).
pub type BoxFuture<T, E> = Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'static>>;

/// The trait users implement, shaped like `val_server::Val`.
#[async_trait::async_trait]
pub trait Greeter: Send + Sync + 'static {
    async fn say_hello(
        &self,
        request: tonic::Request<String>,
    ) -> Result<tonic::Response<String>, tonic::Status>;

    async fn say_goodbye(
        &self,
        request: tonic::Request<String>,
    ) -> Result<tonic::Response<String>, tonic::Status>;
}

pub const SERVICE_NAME: &str = "demo.Greeter";

/// Shaped like `ValServer<T>`: holds the implementation behind an `Arc` so
/// every clone of the server (one per connection) shares it.
pub struct GreeterServer<T> {
    inner: Arc<T>,
}

impl<T> GreeterServer<T> {
    pub fn new(inner: T) -> Self {
        Self::from_arc(Arc::new(inner))
    }

    pub fn from_arc(inner: Arc<T>) -> Self {
        GreeterServer { inner }
    }
}

impl<T> Clone for GreeterServer<T> {
    fn clone(&self) -> Self {
        GreeterServer {
            inner: Arc::clone(&self.inner),
        }
    }
}

/// Turns a handler result into a `RawResponse`.
fn to_raw(result: Result<tonic::Response<String>, tonic::Status>) -> RawResponse {
    todo!()
}

impl<T: Greeter> tower::Service<RawRequest> for GreeterServer<T> {
    type Response = RawResponse;
    type Error = Infallible;
    type Future = BoxFuture<RawResponse, Infallible>;

    /// Always ready, like the generated code.
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        todo!()
    }

    /// Routes by path: `/{SERVICE_NAME}/SayHello` and `/{SERVICE_NAME}/SayGoodbye`
    /// call the trait methods; anything else answers Unimplemented with body
    /// `"unknown method <path>"`.
    fn call(&mut self, request: RawRequest) -> Self::Future {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tower::ServiceExt;

    // ---- locating and slicing the generated file ----

    fn find_generated(dir: &Path, depth: usize) -> Option<PathBuf> {
        let entries = std::fs::read_dir(dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.ends_with("kuksa.val.v2.rs")
                && path.to_string_lossy().contains("databroker-proto")
            {
                return Some(path);
            }
            if depth > 0 && path.is_dir() {
                if let Some(found) = find_generated(&path, depth - 1) {
                    return Some(found);
                }
            }
        }
        None
    }

    fn generated_server_code() -> String {
        let target = std::env::var_os("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"));
        // target/<profile>/build/databroker-proto-<hash>/out/kuksa.val.v2.rs
        let file = find_generated(&target, 5).unwrap_or_else(|| {
            panic!(
                "generated kuksa.val.v2.rs not found under {}; run `cargo build -p databroker-proto` first",
                target.display()
            )
        });
        let text = std::fs::read_to_string(&file).expect("readable generated file");
        let start = text
            .find("pub mod val_server")
            .expect("generated file has a val_server module");
        text[start..].to_string()
    }

    /// Every `"<prefix>..."` string literal in `text`, in order.
    fn quoted_with_prefix<'a>(text: &'a str, prefix: &str) -> Vec<&'a str> {
        text.split('"')
            .filter(|piece| piece.starts_with(prefix))
            .collect()
    }

    // ---- Part 1 ----

    #[test]
    fn service_name_matches() {
        let code = generated_server_code();
        let a = answers();
        assert!(
            code.contains(&format!(
                "pub const SERVICE_NAME: &str = \"{}\";",
                a.service_name
            )),
            "wrong service_name"
        );
    }

    #[test]
    fn routes_match_in_order() {
        let code = generated_server_code();
        assert_eq!(
            answers().routes,
            quoted_with_prefix(&code, "/kuksa.val.v2.VAL/")
        );
    }

    #[test]
    fn server_streaming_methods_match() {
        let code = generated_server_code();
        let trait_body = &code[..code.find("pub struct ValServer").expect("ValServer")];
        let streams: Vec<&str> = trait_body
            .lines()
            .filter_map(|line| line.trim().strip_prefix("type "))
            .filter_map(|rest| rest.split(':').next()?.strip_suffix("Stream"))
            .collect();
        assert_eq!(answers().server_streaming_methods, streams);
    }

    #[test]
    fn trait_attribute_error_type_and_fallback_code() {
        let code = generated_server_code();
        let a = answers();
        let before_trait = &code[..code.find("pub trait Val").expect("trait Val")];
        let attribute = before_trait
            .lines()
            .rev()
            .map(str::trim)
            .find(|line| line.starts_with("#["))
            .expect("an attribute above the trait");
        assert_eq!(attribute, format!("#[{}]", a.trait_attribute));
        assert!(
            code.contains(&format!("type Error = {};", a.service_error_type)),
            "wrong service_error_type"
        );
        assert!(code.contains("tonic::Code::Unimplemented"));
        assert_eq!(a.unknown_route_code, tonic::Code::Unimplemented as i32);
    }

    // ---- Part 2 ----

    #[derive(Default)]
    struct Polite {
        calls: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl Greeter for Polite {
        async fn say_hello(
            &self,
            request: tonic::Request<String>,
        ) -> Result<tonic::Response<String>, tonic::Status> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let name = request.into_inner();
            if name.is_empty() {
                return Err(tonic::Status::invalid_argument("name must not be empty"));
            }
            Ok(tonic::Response::new(format!("hello {name}")))
        }

        async fn say_goodbye(
            &self,
            request: tonic::Request<String>,
        ) -> Result<tonic::Response<String>, tonic::Status> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(tonic::Response::new(format!(
                "bye {}",
                request.into_inner()
            )))
        }
    }

    fn req(path: &str, body: &str) -> RawRequest {
        RawRequest {
            path: path.to_string(),
            body: body.to_string(),
        }
    }

    #[tokio::test]
    async fn routes_to_the_trait_methods() {
        let server = GreeterServer::new(Polite::default());
        let hello = server
            .clone()
            .oneshot(req("/demo.Greeter/SayHello", "vehicle"))
            .await
            .expect("infallible");
        assert_eq!(
            hello,
            RawResponse {
                grpc_status: 0,
                body: "hello vehicle".to_string()
            }
        );
        let bye = server
            .oneshot(req("/demo.Greeter/SayGoodbye", "vehicle"))
            .await
            .expect("infallible");
        assert_eq!(bye.body, "bye vehicle");
    }

    #[tokio::test]
    async fn errors_travel_inside_the_response() {
        let server = GreeterServer::new(Polite::default());
        let response = server
            .oneshot(req("/demo.Greeter/SayHello", ""))
            .await
            .expect("infallible");
        assert_eq!(response.grpc_status, tonic::Code::InvalidArgument as i32);
        assert_eq!(response.body, "name must not be empty");
    }

    #[tokio::test]
    async fn unknown_paths_are_unimplemented() {
        let server = GreeterServer::new(Polite::default());
        let response = server
            .oneshot(req("/demo.Greeter/Teleport", "x"))
            .await
            .expect("infallible");
        assert_eq!(
            response,
            RawResponse {
                grpc_status: 12,
                body: "unknown method /demo.Greeter/Teleport".to_string()
            }
        );
    }

    #[tokio::test]
    async fn clones_share_one_implementation() {
        let shared = Arc::new(Polite::default());
        let server = GreeterServer::from_arc(Arc::clone(&shared));
        for _ in 0..3 {
            server
                .clone()
                .oneshot(req("/demo.Greeter/SayGoodbye", "x"))
                .await
                .expect("infallible");
        }
        assert_eq!(shared.calls.load(Ordering::SeqCst), 3);
    }
}
