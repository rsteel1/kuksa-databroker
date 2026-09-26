# 30 — Protobuf & gRPC with tonic/prost

> **Builds on:** chapters 09 (From/TryFrom), 17 (error mapping), 20–23 (async, streams, async traits).

**Why this matters for the databroker.** The databroker *is* a gRPC server.
Everything a vehicle app or a provider does — read a value, subscribe,
publish, actuate, open a provider stream — is a gRPC call defined in a
`.proto` file and served by Rust code generated from it. Two crates do the
work: **prost** turns protobuf messages into Rust structs, **tonic** turns
protobuf *services* into async Rust traits plus an HTTP/2 server and client.
This chapter teaches you to read the `.proto` files, predict the Rust they
generate, and follow a request from the wire to the broker core and back.

---

## The pipeline: .proto → build.rs → Rust

```text
proto/kuksa/val/v2/val.proto ─┐        (git submodule: eclipse-kuksa/kuksa-proto)
proto/kuksa/val/v2/types.proto┘
            │  databroker-proto/build.rs  (runs at `cargo build`)
            │    - builds `protoc` from source (protobuf-src)
            │    - tonic_prost_build::configure().compile_protos(...)
            ▼
   $OUT_DIR/kuksa.val.v2.rs   (generated Rust: structs, enums, client, server trait)
            │  databroker-proto/src/lib.rs
            │    pub mod kuksa { pub mod val { pub mod v2 { tonic::include_proto!("kuksa.val.v2"); } } }
            ▼
   use databroker_proto::kuksa::val::v2 as proto;   // in the databroker
```

* `build.rs` (`databroker-proto/build.rs:17`) is a Rust program Cargo runs
  *before* compiling the crate. It sets `PROTOC` to a protoc built from
  source by the `protobuf-src` crate — so you don't need protoc installed —
  and writes generated files into `OUT_DIR`.
* `tonic::include_proto!` (`databroker-proto/src/lib.rs:143`) is just
  `include!(concat!(env!("OUT_DIR"), "/kuksa.val.v2.rs"))`.
* It also writes a *file descriptor set* (`build.rs:33`), a binary
  description of the API that the server exposes through **gRPC
  reflection** — that's how tools like `grpcurl` can list services without
  having the `.proto` files.

**Read the generated code.** It's ordinary Rust. After a build:
`learn-rust/target/debug/build/databroker-proto-*/out/kuksa.val.v2.rs`.
Keep it open next to `val.proto` while doing this chapter.

## From .proto to Rust structs

```proto
// proto/kuksa/val/v2/types.proto
message Datapoint {
  google.protobuf.Timestamp timestamp = 1;
  Value value                         = 2;
}
```

becomes

```rust
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Datapoint {
    #[prost(message, optional, tag = "1")]
    pub timestamp: Option<prost_types::Timestamp>,
    #[prost(message, optional, tag = "2")]
    pub value: Option<Value>,
}
```

The mapping rules:

| Protobuf | Rust (prost) | Notes |
|---|---|---|
| `message M` | `pub struct M` | `Clone`, `PartialEq`, `Debug`, `Default`, `prost::Message` |
| scalar field `uint32 x = 2;` | `pub x: u32` | missing on the wire → `0` |
| `string s` | `pub s: String` | missing → `""` |
| message-typed field `Value v` | `pub v: Option<Value>` | always optional |
| `repeated T xs` | `pub xs: Vec<T>` | |
| `map<K, V> m` | `pub m: HashMap<K, V>` | |
| `enum E` field | `pub e: i32` + getter/setter | see below |
| `oneof o { ... }` | `pub o: Option<m::O>` + `enum O` in module `m` | see below |
| `google.protobuf.Timestamp` | `prost_types::Timestamp` | `From<SystemTime>`, `TryFrom<Timestamp> for SystemTime` |
| nested names `SignalID` | module `signal_id`, type `SignalId` | snake_case modules, CamelCase types |

The **tag numbers** (`= 1`, `= 2`) are what goes on the wire, not the names.
That's why protobuf APIs evolve safely: renaming a field is fine, reusing a
tag number is not.

Every message implements `prost::Message`:

```rust
use prost::Message;
let bytes: Vec<u8> = request.encode_to_vec();
let back = proto::PublishValueRequest::decode(bytes.as_slice())?;
```

You rarely call these yourself — tonic does — but they're great for tests
and for understanding that a message is "just bytes". An all-default message
encodes to **zero bytes**: proto3 doesn't send default values.

> **proto3 and "absent"**: for scalars, proto3 can't distinguish "not sent"
> from "sent as 0". `val.proto:15` asks contributors not to use the
> `optional` keyword (which would restore presence) because of old protoc
> versions. Consequence: in `SubscribeRequest`, `buffer_size = 0` must mean
> "default" — see its comment at `val.proto:249`.

## oneof and enums

### oneof → enum

```proto
message SignalID {
  oneof signal {
    int32 id    = 1;
    string path = 2;
  }
}
```

```rust
pub struct SignalId { pub signal: Option<signal_id::Signal> }
pub mod signal_id {
    pub enum Signal { Id(i32), Path(String) }
}
```

A `oneof` is exactly a Rust enum — at most one variant is set. The outer
`Option` is `None` when the client set nothing. Handlers must deal with
that: `get_signal` (`databroker/src/grpc/kuksa_val_v2/val.rs:1308`) returns
`INVALID_ARGUMENT "No SignalId provided"`.

The same pattern carries the whole provider protocol:
`OpenProviderStreamRequest { oneof action { ... 7 variants ... } }`
(`val.proto:377`) becomes `open_provider_stream_request::Action`, and
`open_provider_stream` (`val.rs:909`) is one big `match req.action`.

### enum → i32

```proto
enum DataType { DATA_TYPE_UNSPECIFIED = 0; DATA_TYPE_STRING = 1; ... }
message Metadata { DataType data_type = 4; }
```

```rust
pub enum DataType { Unspecified = 0, String = 1, ... }   // #[repr(i32)]
pub struct Metadata { pub data_type: i32, ... }          // raw number!

meta.data_type()                       // -> DataType; unknown numbers -> Unspecified
meta.set_data_type(DataType::Float);   // stores `DataType::Float as i32`
DataType::try_from(7)                  // -> Result<DataType, UnknownEnumValue>
DataType::Float.as_str_name()          // -> "DATA_TYPE_FLOAT"
```

Why a raw `i32`? A newer client may send a value this server was compiled
without. A Rust enum can't hold an unknown discriminant, so prost keeps the
number and lets *you* choose: lenient getter (`data_type()`) or strict
`try_from`. Also note the `_UNSPECIFIED = 0` convention: 0 is what you get
when the field is absent, so it should never be a meaningful value.

## Converting at the boundary

The broker core (`broker.rs`, `types.rs`) never mentions protobuf. Each API
version has a `conversions.rs` that translates in both directions:

```rust
// databroker/src/grpc/kuksa_val_v2/conversions.rs:394
impl From<proto::Value> for broker::DataValue { ... }
// :436
impl From<broker::DataValue> for proto::Value { ... }
// :48
impl From<broker::Datapoint> for Option<proto::Datapoint> { ... }
```

Benefits: the core has one data model; v1 and v2 are thin adapters
(`kuksa_val_v1/conversions.rs` does the same for the old API); and handlers
read naturally: `let value: DataValue = request_value.into();`.

### The orphan rule

You may `impl Trait for Type` only if **the trait or the type is local to
your crate** (roughly — the precise rule allows local types as trait type
parameters too). This is why:

* `impl From<proto::Value> for broker::DataValue` lives in the *databroker*
  crate: `DataValue` is local there.
* You can't write `impl From<proto::Value> for databroker::types::DataValue`
  in your own crate — both are foreign (E0117). Exercises use a local
  `DataValue` for that reason.
* `impl From<broker::Datapoint> for Option<proto::Datapoint>` is allowed
  even though `Option` and `proto::Datapoint` are foreign: the trait
  parameter `broker::Datapoint` is local.

The rule guarantees two crates can never provide conflicting impls.

## Errors on the wire: tonic::Status

A unary gRPC call returns `Result<tonic::Response<T>, tonic::Status>`.
`Status` = a **code** (`tonic::Code::NotFound`, `InvalidArgument`,
`PermissionDenied`, `Unauthenticated`, `Internal`, ...) + a message. Clients
switch on the code, so the code is API contract — `val.proto` documents it
per rpc (e.g. `val.proto:28`–`33` for `GetValue`).

The databroker's mapping (`conversions.rs:356`, `val.rs:257`):

| Broker error | gRPC code |
|---|---|
| `NotFound` | `NOT_FOUND` |
| `WrongType`, `OutOfBounds*`, `UnsupportedType` | `INVALID_ARGUMENT` |
| `PermissionDenied` | `PERMISSION_DENIED` |
| `PermissionExpired`, no token | `UNAUTHENTICATED` |

Implementing `From<MyError> for tonic::Status` lets handlers use `?`
directly. For streams (`PublishValuesResponse.status`) errors travel inside
messages as `proto::Error { code: ErrorCode, message }` instead, because one
bad value mustn't kill the whole stream.

## Interceptors, metadata and extensions

```text
client ──HTTP/2 headers + body──▶ ValServer::with_interceptor(broker, authorization)
                                          │
                  Interceptor::call(Request<()>)   ◀─ sees headers ("metadata") only
                     • read "authorization: Bearer <jwt>"
                     • decode → Permissions
                     • request.extensions_mut().insert(permissions)
                                          │ Ok(request)  (or Err(Status) → rejected)
                                          ▼
                  Val::get_value(Request<GetValueRequest>)
                     • request.extensions().get::<Permissions>()
```

* **Metadata** (`request.metadata()`) = gRPC headers, strings from the
  client. **Extensions** (`request.extensions()`) = a type-keyed map of Rust
  values, attached server-side. `get::<Permissions>()` looks up by *type*.
* The interceptor (`databroker/src/grpc/server.rs:51`) is a trait impl on
  the `Authorization` enum: disabled → insert `ALLOW_ALL` (`server.rs:60`);
  enabled → parse the header (`server.rs:64`), decode, `Permissions::try_from(claims)`
  (`server.rs:69`), insert.
* It's wired in `serve_with_incoming_shutdown` (`server.rs:190`) with
  `ValServer::with_interceptor(broker.clone(), authorization.clone())`
  (`server.rs:247`).
* Every handler starts with the same `match request.extensions().get::<Permissions>()`
  (`val.rs:262`) and returns `UNAUTHENTICATED` if missing.

## The Val trait: a service is a trait

```proto
service VAL {
  rpc GetValue(GetValueRequest) returns (GetValueResponse);
  rpc Subscribe(SubscribeRequest) returns (stream SubscribeResponse);
  rpc OpenProviderStream(stream OpenProviderStreamRequest) returns (stream OpenProviderStreamResponse);
  ...
}
```

tonic generates `pub mod val_server { pub trait Val { ... } pub struct ValServer<T> }`:

```rust
#[tonic::async_trait]
pub trait Val: Send + Sync + 'static {
    async fn get_value(&self, request: Request<GetValueRequest>)
        -> Result<Response<GetValueResponse>, Status>;

    type SubscribeStream: Stream<Item = Result<SubscribeResponse, Status>> + Send + 'static;
    async fn subscribe(&self, request: Request<SubscribeRequest>)
        -> Result<Response<Self::SubscribeStream>, Status>;

    type OpenProviderStreamStream: Stream<Item = Result<OpenProviderStreamResponse, Status>> + Send + 'static;
    async fn open_provider_stream(&self, request: Request<Streaming<OpenProviderStreamRequest>>)
        -> Result<Response<Self::OpenProviderStreamStream>, Status>;
    ...
}
```

The four RPC shapes:

| proto | Handler receives | Handler returns | Databroker example |
|---|---|---|---|
| unary | `Request<Req>` | `Response<Resp>` | `get_value` (`val.rs:257`), `publish_value` (`val.rs:822`) |
| server streaming | `Request<Req>` | `Response<Self::XStream>` | `subscribe` (`val.rs:383`) |
| client streaming | `Request<Streaming<Req>>` | `Response<Resp>` | `actuate_stream` |
| bidirectional | `Request<Streaming<Req>>` | `Response<Self::XStream>` | `open_provider_stream` (`val.rs:909`) |

For streaming responses the implementor picks a concrete stream type with an
**associated type**: `type SubscribeStream = Pin<Box<dyn Stream<...> + Send + Sync + 'static>>`
(`val.rs:369`) or `type OpenProviderStreamStream = ReceiverStream<...>`
(`val.rs:880`) — chapter 22 covers why.

The databroker implements `Val` **directly on `DataBroker`**
(`val.rs:251`) — and v1's `Val` too (`kuksa_val_v1/val.rs:42`): same struct,
two traits from two generated modules. `DataBroker` is cheap to clone (it's a
bundle of `Arc`s), which `ValServer::with_interceptor(broker.clone(), ..)` needs.

Because the service is "just a trait", tests call the methods directly — no
sockets, no HTTP: build a `tonic::Request`, insert `Permissions` into its
extensions, `broker.get_value(request).await` (see `test_get_value_id_ok`,
`val.rs:1429`). The test helper `streaming_request` (`val.rs:1407`) even
builds a `tonic::Streaming` from a `Vec` by encoding messages by hand.

## kuksa.val.v1 vs kuksa.val.v2

Both APIs are served on the same port by the same `DataBroker`.

| | v1 (`proto/kuksa/val/v1/val.proto`) | v2 (`proto/kuksa/val/v2/val.proto`) |
|---|---|---|
| rpcs | `Get`, `Set`, `Subscribe`, `StreamedUpdate`, `GetServerInfo` | `GetValue(s)`, `Subscribe(ById)`, `Actuate`, `BatchActuate`, `ActuateStream`, `ListMetadata`, `PublishValue`, `OpenProviderStream`, `GetServerInfo` |
| shape | Generic: `DataEntry` + `View` + field masks (`Field` enum) → one `Get`/`Set` does everything | Specific: one rpc per intent |
| paths | Wildcards in requests (`Matcher`) | Exact paths or numeric ids (`SignalID` oneof) |
| actuators | Client writes `actuator_target`; providers subscribe to it | Broker **forwards** actuation to the provider that claimed it (`ProvideActuationRequest`) — no provider, `UNAVAILABLE` |
| providers | Publish via `Set`/`StreamedUpdate` | One bidirectional `OpenProviderStream` per provider: claim signals, publish, receive actuations, answer `GetProviderValueRequest`, update filters |
| sampling | — | `Filter { min_sample_interval }` on subscriptions, propagated to providers |
| errors | In-band `Error` messages | gRPC status codes (streams: in-band) |

v2 is the one to learn first; v1 is kept for compatibility. An even older
API, `sdv.databroker.v1`, is gone from the code (`Api` in
`databroker/src/grpc/server.rs:46` only has `KuksaValV1`/`KuksaValV2`) even
though the top-level `README.md` still mentions it. Read
`proto/kuksa/val/v2/README.md` and `doc/protocol.md`.

## Mental model

| Concept | Analogy |
|---|---|
| `.proto` file | The contract signed by client and server |
| prost structs | Printed forms matching the contract |
| tag numbers | Box numbers on the form — what's actually transmitted |
| `oneof` | A multiple-choice question: tick at most one |
| protobuf enum as `i32` | A code number that might be newer than your codebook |
| `Val` trait | The job description; `DataBroker` is the employee |
| interceptor | Security desk that checks badges and clips a visitor pass (extension) on |
| `tonic::Status` code | The standardised outcome category on a receipt |

**Guarantees**: generated types always match the `.proto` (they're rebuilt
from it); the compiler forces you to implement every rpc of the service; a
handler can't accidentally return the wrong message type.

## Common misconceptions

| Wrong model | Correct model |
|---|---|
| "Message fields can be `None` or a value" | Only message-typed (and `oneof`) fields are `Option`; scalars default to 0/""/false. |
| "Protobuf enums become Rust enum fields" | They're stored as `i32`; use the generated getter/setter or `try_from`. |
| "Renaming a proto field breaks clients" | Only tag numbers matter on the wire. |
| "I can implement `From` between any two types" | Orphan rule (E0117): the trait or the type must be local. |
| "The handler sees the JWT" | The interceptor consumes it; handlers only see `Permissions` in extensions. |
| "Testing a gRPC service needs a running server" | The service is a trait; call its methods with a hand-built `Request`. |

## Smells and idioms

* **`todo!()` on a reachable path.** `impl From<proto::Value> for broker::DataValue`
  ends with `None => todo!()` (`conversions.rs:431`). `Actuate` converts the
  client's `Value` with `DataValue::from(value)` (`val.rs:677`), so a request
  whose `value` is present but empty (`Value { typed_value: None }`) appears
  able to panic the task handling that connection instead of returning
  `INVALID_ARGUMENT`. `From` can't fail; a conversion that *can* fail should
  be `TryFrom` (or map to `NotAvailable`, as `From<&proto::Datapoint>` does).
  A nice first contribution — write the failing test first.
* **Match on a reference, then clone.** The same impl does
  `match &value.typed_value { Some(String(v)) => DataValue::String(v.to_owned()) ... }`
  although it *owns* `value`. Matching by value moves the data out, no clones
  (`c30_e03` does this).
* **Repeated extension boilerplate.** Every handler repeats the ten-line
  `match request.extensions().get::<Permissions>()` block. A helper
  `fn permissions<T>(req: &Request<T>) -> Result<Permissions, Status>` would
  shrink each to one line with `?`.
* **Huge `match` in a `select!` arm** (`open_provider_stream`, `val.rs:909`–`1075`,
  nesting nine levels deep). Extracting one function per action would make it
  testable piece by piece.
* **Good idiom to copy:** keep protobuf types at the edge; convert with
  `From`/`Into`; the core stays API-agnostic.

## In the databroker

| Where | What to notice |
|---|---|
| `proto/kuksa/val/v2/val.proto:23` | `service VAL` — each rpc documents its error codes |
| `proto/kuksa/val/v2/types.proto:29` | `Value` oneof (note `sint32` for signed ints) |
| `proto/kuksa/val/v2/types.proto:73` | `SignalID` oneof id/path |
| `databroker-proto/build.rs:17` | vendored protoc; descriptor sets at `:33`, `:43` |
| `databroker-proto/src/lib.rs:143` | `include_proto!("kuksa.val.v2")` |
| `databroker-proto/src/lib.rs:49` | hand-written `FromStr for DataType` (v1) next to generated code |
| `databroker/src/grpc/server.rs:51` | the auth interceptor |
| `databroker/src/grpc/server.rs:229` | reflection service from the descriptor sets |
| `databroker/src/grpc/server.rs:247` | `ValServer::with_interceptor` for v1 and v2 |
| `databroker/src/grpc/kuksa_val_v2/val.rs:251` | `impl Val for DataBroker` |
| `databroker/src/grpc/kuksa_val_v2/val.rs:257` | `get_value`: permissions → signal id → broker → `Status` mapping |
| `databroker/src/grpc/kuksa_val_v2/val.rs:822` | `publish_value`: proto → `EntryUpdate` → `update_entries` |
| `databroker/src/grpc/kuksa_val_v2/val.rs:1308` | `get_signal`: the `SignalID` oneof, path length limit (`:57`) |
| `databroker/src/grpc/kuksa_val_v2/conversions.rs:24` | `&proto::Datapoint` → `broker::Datapoint` (timestamps!) |
| `databroker/src/grpc/kuksa_val_v2/conversions.rs:356` | `UpdateError::to_status_with_code` |
| `databroker/src/grpc/kuksa_val_v2/val.rs:1429` | `test_get_value_id_ok` — calling the trait directly |
| `databroker/src/grpc/kuksa_val_v1/val.rs:42` | v1's `Val` on the same `DataBroker` |

## Exercises

| Exercise | Goal |
|---|---|
| `c30_e01_prost_messages` | Build real v2 messages; `Default`; encode/decode round trip |
| `c30_e02_oneof_enums` | `SignalId` oneof; enum fields as `i32`, getters, `try_from`, `as_str_name` |
| `c30_e03_proto_conversions` | `From` both ways, `Option<proto::Value>`, the orphan rule |
| `c30_e04_status_codes` | Domain errors → `tonic::Status` and in-stream `proto::Error` |
| `c30_e05_request_extensions_interceptor` | A tonic `Interceptor`: metadata → extensions |
| `c30_e06_call_val_service_directly` | Call the real `Val::publish_value` / `get_value` on `DataBroker` |

## Further reading

* Protocol Buffers language guide (proto3) — <https://protobuf.dev/programming-guides/proto3/>.
* prost README — "Generated code" section (docs.rs/prost).
* tonic docs and examples — `helloworld`, `routeguide` (streaming), `interceptor`
  (github.com/hyperium/tonic/tree/master/examples).
* gRPC status codes — <https://grpc.io/docs/guides/status-codes/>.
* Rust reference: "Orphan rules" / Rust Book ch. 10.2 ("implement a trait on a type").
* `proto/kuksa/val/v2/README.md`, `doc/protocol.md`, `doc/behavior.md`.

## Check yourself

1. Why is `Datapoint.value` an `Option<Value>` but `SubscribeRequest.buffer_size` a plain `u32`?
2. How does a client choose between addressing a signal by id or by path, and what does the server do if it chooses neither?
3. `meta.data_type` is `i32`. Why not `DataType`? How do you get a `DataType` out?
4. Where do `Permissions` come from inside `get_value`, and who put them there?
5. Why can the databroker write `impl From<broker::Datapoint> for Option<proto::Datapoint>`?
6. What would you need to change to add a new rpc to the v2 API?

<details><summary>Answers</summary>

1. Message-typed fields are always optional in prost; scalars aren't (proto3
   has no presence for them without `optional`), so 0 must mean "default".
2. By setting the `SignalId.signal` oneof to `Id` or `Path`; if unset,
   `get_signal` returns `INVALID_ARGUMENT`.
3. A client might send a number this server doesn't know; a Rust enum can't
   hold it. Use `meta.data_type()` (unknown → `Unspecified`) or
   `DataType::try_from(meta.data_type)`.
4. From `request.extensions()`; the auth interceptor inserted them
   (`server.rs:51`), or the test did by hand.
5. The orphan rule is satisfied because the trait's type parameter
   (`broker::Datapoint`) is local to the databroker crate.
6. Add it to `val.proto` (in the kuksa-proto repo/submodule), rebuild
   (`build.rs` regenerates the trait), then implement the new trait method on
   `DataBroker` — the compiler will refuse to build until you do.

</details>
