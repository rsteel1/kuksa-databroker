// c24_e07_tower_service.rs
//
// CONCEPT: `tower::Service` (poll_ready + call + an associated Future) and
//          `tower::Layer` middleware — the abstraction tonic, axum and hyper
//          are built on.
// GUIDE:   chapters/24-async-internals/guide.md  (section "tower: Service and Layer")
// DATABROKER: databroker/src/grpc/server.rs — `impl tonic::service::Interceptor
//             for Authorization` (line 51) validates the bearer token and puts
//             `Permissions` into the request; `ValServer::with_interceptor(...)`
//             (line 256) wraps the Val service in tonic's `InterceptedService`,
//             which is a tower `Service` around another `Service`.
// EXPECT:  tests fail: `todo!()` panics
//
// A `Service<Request>` is "an async function with back-pressure":
//     poll_ready(&mut self, cx) -> Poll<Result<(), Error>>   // can you take one more?
//     call(&mut self, req)      -> Self::Future              // start handling it
// A `Layer<S>` turns a service into a bigger service (middleware). Stacking
// layers with `ServiceBuilder` gives you auth, timeouts, logging, limits...
// without the inner service knowing.
//
// YOUR TASK
//   1. `impl Service<Request> for SignalStore`: always ready; `call` returns
//      `Err(Unauthenticated)` if `req.user` is None, the value if the path
//      exists, `Err(NotFound(path))` otherwise. Use `std::future::Ready`.
//   2. `impl<S> Layer<S> for AuthLayer` producing `Auth<S>`.
//   3. `impl Service<Request> for Auth<S>`: `poll_ready` delegates to the
//      inner service; `call` accepts `token == Some("Bearer <token>")` for a
//      known token, sets `req.user` to that token's user and calls the inner
//      service; anything else is rejected with `Unauthenticated` WITHOUT
//      calling the inner service. Use `BoxFuture` because the two branches
//      have different future types.
//   `CountLayer` is a complete example. Do not edit it or the tests.

use futures::future::BoxFuture;
use std::collections::HashMap;
use std::future::{ready, Ready};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use tower::{Layer, Service};

#[derive(Debug, Clone, Default)]
struct Request {
    token: Option<String>,
    path: String,
    /// Set by the auth middleware (like tonic's request extensions).
    user: Option<String>,
}

#[derive(Debug, PartialEq)]
enum Error {
    Unauthenticated(String),
    NotFound(String),
}

// ------------------------------------------------------------- inner service

#[derive(Clone)]
struct SignalStore {
    values: Arc<HashMap<String, i64>>,
}

impl Service<Request> for SignalStore {
    type Response = i64;
    type Error = Error;
    type Future = Ready<Result<i64, Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        // An in-memory store never needs to push back.
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let result = if req.user.is_none() {
            Err(Error::Unauthenticated("no user".into()))
        } else {
            self.values
                .get(&req.path)
                .copied()
                .ok_or(Error::NotFound(req.path))
        };
        ready(result)
    }
}

// ----------------------------------------------------------- auth middleware

#[derive(Clone)]
struct AuthLayer {
    /// token -> user name
    tokens: Arc<HashMap<String, String>>,
}

#[derive(Clone)]
struct Auth<S> {
    inner: S,
    tokens: Arc<HashMap<String, String>>,
}

impl<S> Layer<S> for AuthLayer {
    type Service = Auth<S>;

    fn layer(&self, inner: S) -> Auth<S> {
        Auth {
            inner,
            tokens: self.tokens.clone(),
        }
    }
}

impl<S> Service<Request> for Auth<S>
where
    S: Service<Request, Error = Error>,
    S::Future: Send + 'static,
    // The error branch builds its own future holding a `Result<S::Response, _>`,
    // so the response type must be sendable too for the box to be `Send`.
    S::Response: Send + 'static,
{
    type Response = S::Response;
    type Error = Error;
    type Future = BoxFuture<'static, Result<S::Response, Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        // WHY: we can only accept a request if the service we'll forward it to can.
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request) -> Self::Future {
        let user = req
            .token
            .as_deref()
            .and_then(|header| header.strip_prefix("Bearer "))
            .and_then(|token| self.tokens.get(token))
            .cloned();
        match user {
            Some(user) => {
                req.user = Some(user);
                Box::pin(self.inner.call(req))
            }
            None => Box::pin(ready(Err(Error::Unauthenticated(
                "invalid or missing token".into(),
            )))),
        }
    }
}

// ------------------------------------------------ complete example: counting

#[derive(Clone)]
struct CountLayer {
    count: Arc<AtomicUsize>,
}

#[derive(Clone)]
struct Counted<S> {
    inner: S,
    count: Arc<AtomicUsize>,
}

impl<S> Layer<S> for CountLayer {
    type Service = Counted<S>;
    fn layer(&self, inner: S) -> Counted<S> {
        Counted {
            inner,
            count: self.count.clone(),
        }
    }
}

impl<S: Service<Request>> Service<Request> for Counted<S> {
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), S::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> S::Future {
        self.count.fetch_add(1, Ordering::SeqCst);
        self.inner.call(req)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower::{ServiceBuilder, ServiceExt};

    fn store() -> SignalStore {
        SignalStore {
            values: Arc::new(HashMap::from([("Vehicle.Speed".to_string(), 42)])),
        }
    }

    fn auth() -> AuthLayer {
        AuthLayer {
            tokens: Arc::new(HashMap::from([("s3cret".to_string(), "alice".to_string())])),
        }
    }

    fn req(token: Option<&str>, path: &str) -> Request {
        Request {
            token: token.map(str::to_string),
            path: path.to_string(),
            user: None,
        }
    }

    #[tokio::test]
    async fn store_alone_requires_a_user() {
        let out = store().oneshot(req(None, "Vehicle.Speed")).await;
        assert!(matches!(out, Err(Error::Unauthenticated(_))));

        let mut with_user = req(None, "Vehicle.Speed");
        with_user.user = Some("bob".into());
        assert_eq!(store().oneshot(with_user).await, Ok(42));
    }

    #[tokio::test]
    async fn auth_layer_sets_user_and_forwards() {
        let svc = ServiceBuilder::new().layer(auth()).service(store());
        assert_eq!(
            svc.clone()
                .oneshot(req(Some("Bearer s3cret"), "Vehicle.Speed"))
                .await,
            Ok(42)
        );
        assert_eq!(
            svc.oneshot(req(Some("Bearer s3cret"), "Vehicle.Nope"))
                .await,
            Err(Error::NotFound("Vehicle.Nope".into()))
        );
    }

    #[tokio::test]
    async fn auth_layer_rejects_bad_tokens() {
        let svc = ServiceBuilder::new().layer(auth()).service(store());
        for bad in [
            None,
            Some("s3cret"),
            Some("Bearer wrong"),
            Some("Basic s3cret"),
        ] {
            let out = svc.clone().oneshot(req(bad, "Vehicle.Speed")).await;
            assert!(
                matches!(out, Err(Error::Unauthenticated(_))),
                "token {bad:?} should be rejected"
            );
        }
    }

    #[tokio::test]
    async fn layer_order_matters() {
        // Outer counter sees every request; inner counter only authenticated ones.
        let outer = Arc::new(AtomicUsize::new(0));
        let inner = Arc::new(AtomicUsize::new(0));
        let svc = ServiceBuilder::new()
            .layer(CountLayer {
                count: outer.clone(),
            })
            .layer(auth())
            .layer(CountLayer {
                count: inner.clone(),
            })
            .service(store());

        let _ = svc
            .clone()
            .oneshot(req(Some("Bearer s3cret"), "Vehicle.Speed"))
            .await;
        let _ = svc
            .clone()
            .oneshot(req(Some("Bearer nope"), "Vehicle.Speed"))
            .await;
        let _ = svc.oneshot(req(None, "Vehicle.Speed")).await;

        assert_eq!(outer.load(Ordering::SeqCst), 3);
        assert_eq!(
            inner.load(Ordering::SeqCst),
            1,
            "rejected requests never reach inner layers"
        );
    }
}
