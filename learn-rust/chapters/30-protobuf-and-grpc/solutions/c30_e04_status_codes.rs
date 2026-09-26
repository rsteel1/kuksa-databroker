// c30_e04_status_codes.rs
//
// CONCEPT: mapping domain errors to `tonic::Status` (gRPC status codes).
// GUIDE:   chapters/30-protobuf-and-grpc/guide.md  (section "Errors on the wire: tonic::Status")
// DATABROKER: databroker/src/grpc/kuksa_val_v2/conversions.rs — `UpdateError::to_status_with_code`;
//             val.rs `get_value` (ReadError -> Status)
// EXPECT:  tests fail: `todo!()` panics
//
// A gRPC call either returns a response or a `tonic::Status` = a standard
// `Code` + a human message. Clients branch on the CODE, so choosing it is
// part of the API contract (val.proto documents which codes each rpc
// returns). The databroker's convention:
//
//   NotFound                                -> Code::NotFound
//   WrongType / OutOfBounds* / Unsupported  -> Code::InvalidArgument
//   PermissionDenied                        -> Code::PermissionDenied
//   PermissionExpired (token expired)       -> Code::Unauthenticated   (!)
//
// Messages include the signal id: "Signal not found (id: 12)".
//
// For PublishValues the errors travel *inside* the response as
// `proto::Error { code: ErrorCode, message }` instead, using the smaller
// protobuf `ErrorCode` enum.
//
// YOUR TASK
//   Implement `UpdateError::to_status`, `impl From<ReadError> for tonic::Status`,
//   and `to_proto_error`. Do not edit the tests.

use databroker_proto::kuksa::val::v2 as proto;
use tonic::{Code, Status};

#[derive(Debug, Clone, Copy, PartialEq)]
enum UpdateError {
    NotFound,
    WrongType,
    OutOfBoundsMinMax,
    PermissionDenied,
    PermissionExpired,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ReadError {
    NotFound,
    PermissionDenied,
    PermissionExpired,
}

impl UpdateError {
    fn to_status(self, id: i32) -> Status {
        match self {
            UpdateError::NotFound => {
                Status::new(Code::NotFound, format!("Signal not found (id: {id})"))
            }
            UpdateError::WrongType => Status::new(
                Code::InvalidArgument,
                format!("Wrong type provided (id: {id})"),
            ),
            UpdateError::OutOfBoundsMinMax => Status::new(
                Code::InvalidArgument,
                format!("Value out of min/max bounds (id: {id})"),
            ),
            UpdateError::PermissionDenied => Status::new(
                Code::PermissionDenied,
                format!("Permission denied (id: {id})"),
            ),
            UpdateError::PermissionExpired => Status::new(
                Code::Unauthenticated,
                format!("Permission expired (id: {id})"),
            ),
        }
    }
}

impl From<ReadError> for Status {
    fn from(err: ReadError) -> Self {
        match err {
            ReadError::NotFound => Status::not_found("Path not found"),
            ReadError::PermissionDenied => Status::permission_denied("Permission denied"),
            ReadError::PermissionExpired => Status::unauthenticated("Permission expired"),
        }
    }
}

/// For in-stream errors (PublishValuesResponse.status). Expired permissions
/// map to PermissionDenied here: ErrorCode has no "unauthenticated".
fn to_proto_error(err: UpdateError) -> proto::Error {
    let (code, message) = match err {
        UpdateError::NotFound => (proto::ErrorCode::NotFound, "Signal not found"),
        UpdateError::WrongType => (proto::ErrorCode::InvalidArgument, "Wrong type"),
        UpdateError::OutOfBoundsMinMax => (proto::ErrorCode::InvalidArgument, "Out of bounds"),
        UpdateError::PermissionDenied => (proto::ErrorCode::PermissionDenied, "Permission denied"),
        UpdateError::PermissionExpired => {
            (proto::ErrorCode::PermissionDenied, "Permission expired")
        }
    };
    proto::Error {
        code: code as i32,
        message: message.to_string(),
    }
}

/// A handler-shaped function: `?` converts ReadError into Status via From.
fn read_speed(result: Result<f32, ReadError>) -> Result<f32, Status> {
    let speed = result?;
    Ok(speed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_errors_to_status() {
        let s = UpdateError::NotFound.to_status(12);
        assert_eq!(s.code(), Code::NotFound);
        assert_eq!(s.message(), "Signal not found (id: 12)");

        assert_eq!(
            UpdateError::WrongType.to_status(1).code(),
            Code::InvalidArgument
        );
        assert_eq!(
            UpdateError::OutOfBoundsMinMax.to_status(1).code(),
            Code::InvalidArgument
        );
        assert_eq!(
            UpdateError::PermissionDenied.to_status(1).code(),
            Code::PermissionDenied
        );
        let expired = UpdateError::PermissionExpired.to_status(3);
        assert_eq!(expired.code(), Code::Unauthenticated);
        assert!(expired.message().contains("(id: 3)"));
    }

    #[test]
    fn read_errors_convert_with_question_mark() {
        assert_eq!(read_speed(Ok(5.0)).unwrap(), 5.0);
        assert_eq!(
            read_speed(Err(ReadError::NotFound)).unwrap_err().code(),
            Code::NotFound
        );
        assert_eq!(
            read_speed(Err(ReadError::PermissionDenied))
                .unwrap_err()
                .code(),
            Code::PermissionDenied
        );
        assert_eq!(
            read_speed(Err(ReadError::PermissionExpired))
                .unwrap_err()
                .code(),
            Code::Unauthenticated
        );
    }

    #[test]
    fn in_stream_errors() {
        let e = to_proto_error(UpdateError::WrongType);
        assert_eq!(e.code(), proto::ErrorCode::InvalidArgument);
        assert!(!e.message.is_empty());
        assert_eq!(
            to_proto_error(UpdateError::PermissionExpired).code(),
            proto::ErrorCode::PermissionDenied
        );
        assert_eq!(
            to_proto_error(UpdateError::NotFound).code,
            proto::ErrorCode::NotFound as i32
        );
    }
}
