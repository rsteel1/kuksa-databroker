// c30_e05_request_extensions_interceptor.rs
//
// CONCEPT: tonic interceptors, request metadata (headers) and extensions (typed per-request data).
// GUIDE:   chapters/30-protobuf-and-grpc/guide.md  (section "Interceptors, metadata and extensions")
// DATABROKER: databroker/src/grpc/server.rs — `impl tonic::service::Interceptor for Authorization`;
//             val.rs `get_value` — `request.extensions().get::<Permissions>()`
// EXPECT:  tests fail: `todo!()` panics
//
// Every gRPC request to the databroker passes through an interceptor BEFORE
// the handler runs. It reads the `authorization: Bearer <jwt>` header
// (gRPC "metadata"), decodes the token into `Permissions`, and stores them in
// the request's *extensions* — a type-keyed map for attaching Rust values to a
// request. The handler later pulls them out with `extensions().get::<Permissions>()`.
// If authorization is disabled, the interceptor inserts ALLOW_ALL instead.
//
// The interceptor sees a `Request<()>`: only headers + extensions, not the
// message body (which may not have arrived yet).
//
// YOUR TASK
//   1. Implement `Interceptor::call` for `Auth`:
//        * Disabled               -> insert `Permissions::AllowAll`, Ok
//        * Enabled, no header     -> Err(unauthenticated("No auth token provided"))
//        * header not "Bearer X"  -> Err(unauthenticated("Invalid auth token"))
//        * token unknown          -> Err(unauthenticated("Invalid auth token"))
//        * token known            -> insert its Permissions (clone), Ok
//   2. Implement `handler`: return the caller's permissions or
//      Err(unauthenticated("Unauthenticated")) if none were attached.
//   Do not edit the tests.

// I AM NOT DONE

use std::collections::HashMap;
use tonic::service::Interceptor;
use tonic::{Request, Status};

#[derive(Debug, Clone, PartialEq)]
enum Permissions {
    AllowAll,
    ReadOnly(Vec<String>),
}

#[derive(Clone)]
enum Auth {
    Disabled,
    /// token -> permissions (stands in for real JWT decoding)
    Enabled {
        tokens: HashMap<String, Permissions>,
    },
}

impl Interceptor for Auth {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        todo!("request.metadata().get(\"authorization\"), .to_str(), strip_prefix(\"Bearer \"), extensions_mut().insert(..)")
    }
}

/// Stand-in for a gRPC handler such as `Val::get_value`.
fn handler<T>(request: &Request<T>) -> Result<Permissions, Status> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tonic::Code;

    fn enabled() -> Auth {
        let mut tokens = HashMap::new();
        tokens.insert("t-admin".to_string(), Permissions::AllowAll);
        tokens.insert(
            "t-reader".to_string(),
            Permissions::ReadOnly(vec!["Vehicle.Speed".into()]),
        );
        Auth::Enabled { tokens }
    }

    fn with_header(value: &str) -> Request<()> {
        let mut req = Request::new(());
        req.metadata_mut()
            .insert("authorization", value.parse().expect("ascii header"));
        req
    }

    #[test]
    fn disabled_allows_everything() {
        let req = Auth::Disabled.call(Request::new(())).expect("passes");
        assert_eq!(handler(&req).unwrap(), Permissions::AllowAll);
    }

    #[test]
    fn valid_token_attaches_its_permissions() {
        let req = enabled()
            .call(with_header("Bearer t-reader"))
            .expect("passes");
        assert_eq!(
            handler(&req).unwrap(),
            Permissions::ReadOnly(vec!["Vehicle.Speed".into()])
        );
    }

    #[test]
    fn missing_header() {
        let err = enabled().call(Request::new(())).unwrap_err();
        assert_eq!(err.code(), Code::Unauthenticated);
        assert_eq!(err.message(), "No auth token provided");
    }

    #[test]
    fn malformed_or_unknown_token() {
        for header in ["t-admin", "Basic t-admin", "Bearer nope"] {
            let err = enabled().call(with_header(header)).unwrap_err();
            assert_eq!(err.code(), Code::Unauthenticated, "header {header:?}");
            assert_eq!(err.message(), "Invalid auth token");
        }
    }

    #[test]
    fn handler_without_interceptor_is_unauthenticated() {
        let req = Request::new(());
        assert_eq!(handler(&req).unwrap_err().code(), Code::Unauthenticated);
    }
}
