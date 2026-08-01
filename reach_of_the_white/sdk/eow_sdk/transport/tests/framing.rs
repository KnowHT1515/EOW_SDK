use std::io::Cursor;

use eow_sdk_transport::frame::{self, DecodeResult};
use eow_sdk_transport::limits::{MAX_FRAME_BODY, SENTINEL};
use eow_sdk_transport::{Deframer, FrameAbort, FramedIoError, FramedReader, FramedWriter};

#[test]
fn framing_uses_utf8_byte_length_and_ignores_sentinel_inside_body() {
    let body = "?\\eow/?".as_bytes();
    let framed = frame::encode(body).unwrap();
    assert_eq!(&framed[..4], &(body.len() as u32).to_be_bytes());
    assert_eq!(&framed[framed.len() - 5..], &SENTINEL);
    assert_eq!(
        frame::decode(&framed),
        DecodeResult::Frame {
            body,
            consumed: framed.len(),
        }
    );
}

#[test]
fn incremental_deframer_handles_every_split_and_multiple_frames() {
    let first = frame::encode(b"first").unwrap();
    let second = frame::encode("??".as_bytes()).unwrap();
    let mut bytes = first;
    bytes.extend_from_slice(&second);

    for split in 0..=bytes.len() {
        let mut decoder = Deframer::new();
        decoder.push(&bytes[..split]).unwrap();
        let mut bodies = Vec::new();
        while let Some(body) = decoder.next_frame().unwrap() {
            bodies.push(body);
        }
        decoder.push(&bytes[split..]).unwrap();
        while let Some(body) = decoder.next_frame().unwrap() {
            bodies.push(body);
        }
        assert_eq!(bodies, [b"first".to_vec(), "??".as_bytes().to_vec()]);
    }
}

#[test]
fn illegal_lengths_abort_from_prefix_alone() {
    assert_eq!(
        frame::decode(&0_u32.to_be_bytes()),
        DecodeResult::Abort(FrameAbort::ZeroLength)
    );
    assert_eq!(
        frame::decode(&((MAX_FRAME_BODY + 1) as u32).to_be_bytes()),
        DecodeResult::Abort(FrameAbort::Oversize)
    );
}

#[test]
fn bad_sentinel_permanently_poison_deframer() {
    let mut framed = frame::encode(b"body").unwrap();
    *framed.last_mut().unwrap() = b'!';

    let mut decoder = Deframer::new();
    decoder.push(&framed).unwrap();
    assert_eq!(decoder.next_frame(), Err(FrameAbort::BadSentinel));
    assert!(decoder.is_aborted());
    assert_eq!(
        decoder.push(&frame::encode(b"later").unwrap()),
        Err(FrameAbort::BadSentinel)
    );
    assert_eq!(decoder.next_frame(), Err(FrameAbort::BadSentinel));
}

#[test]
fn encode_rejects_empty_body() {
    assert!(matches!(frame::encode(b""), Err(FramedIoError::EmptyBody)));
}

#[test]
fn framed_io_works_with_in_memory_readers_and_writers() {
    let mut writer = FramedWriter::new(Vec::new());
    writer.write_frame(b"one").unwrap();
    writer.write_frame("?".as_bytes()).unwrap();
    let bytes = writer.into_inner();

    let mut reader = FramedReader::with_chunk_size(Cursor::new(bytes), 1);
    assert_eq!(reader.read_frame().unwrap(), Some(b"one".to_vec()));
    assert_eq!(reader.read_frame().unwrap(), Some("?".as_bytes().to_vec()));
    assert_eq!(reader.read_frame().unwrap(), None);
}

#[test]
fn framed_reader_rejects_truncated_eof() {
    let mut framed = frame::encode(b"partial").unwrap();
    framed.pop();
    let mut reader = FramedReader::new(Cursor::new(framed));
    assert!(matches!(
        reader.read_frame(),
        Err(FramedIoError::TruncatedFrame)
    ));
}
