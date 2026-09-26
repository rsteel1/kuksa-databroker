# Chapter 30 hints

## c30_e01_prost_messages
Build structs literally, wrapping message fields in `Some(...)`:
`proto::Datapoint { timestamp: Some(at.into()), value: Some(proto::Value { typed_value: Some(proto::value::TypedValue::Float(value)) }) }`.
The path goes in `proto::SignalId { signal: Some(proto::signal_id::Signal::Path(path.to_string())) }`.
For SubscribeRequest set `signal_paths` and `buffer_size`, then `..Default::default()`.
Round trip: `let bytes = request.encode_to_vec();` then
`proto::PublishValueRequest::decode(bytes.as_slice())` — `decode` needs the
`prost::Message` trait in scope (it is imported).

## c30_e02_oneof_enums
`describe`: `match &signal_id.signal { Some(proto::signal_id::Signal::Id(id)) => ..., Some(...Path(path)) => ..., None => ... }`.
Metadata: start from `proto::Metadata { path: ..., id, ..Default::default() }`,
make it `mut`, then `meta.set_data_type(proto::DataType::Float)` and
`meta.set_entry_type(proto::EntryType::Sensor)`.
Name: `meta.data_type()` (the generated getter, lenient) then `.as_str_name()`.
Strict parse: `proto::DataType::try_from(raw).ok()`.

## c30_e03_proto_conversions
Match on `value.typed_value` by value (you own `value`), e.g.
`Some(TypedValue::StringArray(array)) => DataValue::StringArray(array.values)` —
moving `array.values` out avoids a clone. Add a `Some(_)` arm for the
variants this local enum doesn't model.
The other direction builds `proto::Value { typed_value }` where
`typed_value` is `None` for `NotAvailable` and e.g.
`Some(TypedValue::Int32Array(proto::Int32Array { values }))` for arrays.
`From<DataValue> for Option<proto::Value>`: `NotAvailable => None, other => Some(other.into())`.

## c30_e04_status_codes
`Status::new(Code::NotFound, format!("Signal not found (id: {id})"))`, and so
on — read the table in the header carefully (expired → `Unauthenticated`).
`From<ReadError> for Status` can use the shortcuts `Status::not_found(msg)`,
`Status::permission_denied(msg)`, `Status::unauthenticated(msg)`.
`proto::Error` stores the code as `i32`: `code: proto::ErrorCode::NotFound as i32`
(or build it and call `set_code`).

## c30_e05_request_extensions_interceptor
Decide the `Permissions` first with `match self { Auth::Disabled => ..., Auth::Enabled { tokens } => ... }`.
In the enabled branch chain `?`:
`request.metadata().get("authorization").ok_or_else(|| Status::unauthenticated("No auth token provided"))?`,
then `.to_str().ok().and_then(|h| h.strip_prefix("Bearer "))`, then
`tokens.get(token).cloned()`. Finally
`request.extensions_mut().insert(permissions); Ok(request)`.
Handler: `request.extensions().get::<Permissions>().cloned().ok_or_else(...)`.

## c30_e06_call_val_service_directly
`request_with`: `let mut r = Request::new(message); r.extensions_mut().insert(permissions.clone()); r`.
With `use ...::val_server::Val;` in scope you can call
`broker.publish_value(request).await` and `broker.get_value(request).await`
— both return `Result<tonic::Response<_>, Status>`; `?` propagates the
`Status`, `.into_inner()` unwraps the response message.
Extract the float with `response.data_point.and_then(|dp| dp.value).and_then(|v| match v.typed_value { Some(TypedValue::Float(f)) => Some(f), _ => None })`.
