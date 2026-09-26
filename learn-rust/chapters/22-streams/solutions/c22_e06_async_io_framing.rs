// c22_e06_async_io_framing.rs
//
// CONCEPT: `AsyncRead`/`AsyncWrite` byte streams, and *framing*: turning a
//          stream of bytes into a stream of messages.
// GUIDE:   chapters/22-streams/guide.md  (section "Async I/O: bytes in, frames out")
// DATABROKER: every gRPC message the databroker sends or receives travels
//          over HTTP/2 with exactly this framing: 1 byte "compressed" flag,
//          a 4-byte big-endian length, then the protobuf bytes. tonic/hyper
//          do it for you; here you do it by hand.
// EXPECT:  tests fail: `todo!()` panics.
//
// TCP (and the in-memory `tokio::io::duplex` used in the tests) delivers
// BYTES, not messages: one `write` can arrive as several reads, or several
// writes as one read. A frame header tells the reader where each message ends.
//
//   +------+----------------+----------------------+
//   | flag | length (u32 BE)| payload (length bytes)|
//   +------+----------------+----------------------+
//
// YOUR TASK
//   Implement `write_frame`, `read_frame` and `frame_stream`.
//   Use the extension traits `AsyncReadExt` (`read_u8`, `read_u32`,
//   `read_exact`) and `AsyncWriteExt` (`write_u8`, `write_u32`, `write_all`).
//   Do not edit the tests.

use futures::Stream;
use std::io;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Refuse frames larger than this: a hostile peer must not be able to make
/// us allocate gigabytes by sending a huge length.
const MAX_FRAME: u32 = 1024;

/// Writes one uncompressed frame (flag 0, big-endian length, payload).
async fn write_frame<W: AsyncWrite + Unpin>(writer: &mut W, payload: &[u8]) -> io::Result<()> {
    let len = u32::try_from(payload.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "payload too large"))?;
    writer.write_u8(0).await?;
    writer.write_u32(len).await?;
    writer.write_all(payload).await?;
    // WHY: writers may buffer; flush so the peer actually receives the frame.
    writer.flush().await
}

/// Reads one frame.
/// * `Ok(None)` — clean end of stream *between* frames.
/// * `Err(UnexpectedEof)` — the stream ended in the middle of a frame.
/// * `Err(InvalidData)` — compressed flag set, or length above `MAX_FRAME`.
async fn read_frame<R: AsyncRead + Unpin>(reader: &mut R) -> io::Result<Option<Vec<u8>>> {
    let flag = match reader.read_u8().await {
        Ok(flag) => flag,
        // WHY: EOF before the first header byte is a normal end, not an error.
        Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(err) => return Err(err),
    };
    if flag != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "compressed frames are not supported",
        ));
    }
    let len = reader.read_u32().await?;
    if len > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("frame of {len} bytes exceeds the {MAX_FRAME}-byte limit"),
        ));
    }
    let mut payload = vec![0; len as usize];
    reader.read_exact(&mut payload).await?;
    Ok(Some(payload))
}

/// Turns a byte reader into a `Stream` of frames. The stream ends after a
/// clean end of input, and also right after yielding the first error.
fn frame_stream<R: AsyncRead + Unpin>(reader: R) -> impl Stream<Item = io::Result<Vec<u8>>> {
    // WHY: the state is `Option<R>`: `None` means "finished", so the stream
    // stops after an error instead of trying to read a broken input forever.
    futures::stream::unfold(Some(reader), |state| async move {
        let mut reader = state?;
        match read_frame(&mut reader).await {
            Ok(Some(frame)) => Some((Ok(frame), Some(reader))),
            Ok(None) => None,
            Err(err) => Some((Err(err), None)),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    #[tokio::test]
    async fn frames_round_trip_through_a_tiny_pipe() {
        // An 8-byte pipe forces partial reads and writes (backpressure), so
        // the writer runs in its own task.
        let (mut client, mut server) = tokio::io::duplex(8);
        let writer = tokio::spawn(async move {
            write_frame(&mut client, b"Vehicle.Speed=42").await?;
            write_frame(&mut client, b"").await?;
            write_frame(&mut client, &[7; 300]).await
        });

        assert_eq!(
            read_frame(&mut server).await.expect("frame 1"),
            Some(b"Vehicle.Speed=42".to_vec())
        );
        assert_eq!(
            read_frame(&mut server).await.expect("frame 2"),
            Some(vec![])
        );
        assert_eq!(
            read_frame(&mut server).await.expect("frame 3"),
            Some(vec![7; 300])
        );
        writer.await.expect("no panic").expect("writes succeed");
        // Writer finished and `client` was dropped: clean end of stream.
        assert_eq!(read_frame(&mut server).await.expect("clean eof"), None);
    }

    #[tokio::test]
    async fn wire_format_is_flag_length_payload() {
        let mut bytes: Vec<u8> = Vec::new(); // Vec<u8> implements AsyncWrite
        write_frame(&mut bytes, b"hi").await.expect("write to Vec");
        assert_eq!(bytes, [0, 0, 0, 0, 2, b'h', b'i']);
    }

    #[tokio::test]
    async fn truncated_frame_is_an_error() {
        // Header says 10 bytes, only 3 arrive.
        let input: &[u8] = &[0, 0, 0, 0, 10, 1, 2, 3];
        let mut reader = input;
        let err = read_frame(&mut reader).await.expect_err("truncated");
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[tokio::test]
    async fn oversized_and_compressed_frames_are_rejected() {
        let huge: &[u8] = &[0, 0xFF, 0xFF, 0xFF, 0xFF];
        let mut reader = huge;
        let err = read_frame(&mut reader).await.expect_err("too large");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);

        let compressed: &[u8] = &[1, 0, 0, 0, 1, 9];
        let mut reader = compressed;
        let err = read_frame(&mut reader).await.expect_err("compressed");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[tokio::test]
    async fn stream_yields_frames_then_stops_after_an_error() {
        let mut bytes: Vec<u8> = Vec::new();
        write_frame(&mut bytes, b"a").await.expect("write");
        write_frame(&mut bytes, b"bc").await.expect("write");
        bytes.extend_from_slice(&[0, 0, 0, 0, 5, 1]); // truncated third frame

        let items: Vec<io::Result<Vec<u8>>> = frame_stream(bytes.as_slice()).collect().await;
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].as_ref().expect("frame a"), b"a");
        assert_eq!(items[1].as_ref().expect("frame bc"), b"bc");
        assert!(items[2].is_err());
    }

    #[tokio::test]
    async fn stream_of_clean_input_just_ends() {
        let mut bytes: Vec<u8> = Vec::new();
        write_frame(&mut bytes, b"only").await.expect("write");
        let frames: Vec<Vec<u8>> = frame_stream(bytes.as_slice())
            .map(|item| item.expect("valid frame"))
            .collect()
            .await;
        assert_eq!(frames, vec![b"only".to_vec()]);
    }
}
