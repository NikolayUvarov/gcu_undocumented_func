//! Host tests of Server-Sent Events (libmind/src/sse.rs, 000-NET-0047): the standard's examples, every line ending, a
//! stream split at every byte boundary, comments, ids and retry, the limits, and a caller that stops.
extern crate alloc;
#[path = "../libmind/src/sse.rs"]
mod sse;

use sse::{Error, Event, Stream};

fn event(kind: &str, data: &str, id: &str) -> Event { Event { kind: kind.into(), data: data.into(), id: id.into() } }

fn read(stream: &mut Stream, pieces: &[&[u8]]) -> Result<Vec<Event>, Error> {
    let mut events = Vec::new();
    for piece in pieces { stream.feed(piece, &mut |e| { events.push(e); true })?; }
    Ok(events)
}

#[test]
fn the_standards_examples() {
    // Two events: the first's data on two lines, the second after a comment.
    let text = b"data: YHOO\ndata: +2\ndata: 10\n\n: a comment\ndata: second\n\n";
    assert_eq!(read(&mut Stream::new(1024), &[text]).unwrap(), [event("message", "YHOO\n+2\n10", ""), event("message", "second", "")]);
    // "data:test" and "data: test" are the same; a lone "data" adds an empty line; the last event has no blank line yet.
    let text = b"data\n\ndata\ndata\n\ndata:\n\ndata:test\n\ndata: test\n\ndata: unfinished";
    assert_eq!(read(&mut Stream::new(1024), &[text]).unwrap(), [event("message", "", ""), event("message", "\n", ""), event("message", "", ""), event("message", "test", ""), event("message", "test", "")]);
    // An event with a type and an id; the id stays for the next.
    let text = b"event: add\nid: 7\ndata: 73857293\n\ndata: next\n\n";
    assert_eq!(read(&mut Stream::new(1024), &[text]).unwrap(), [event("add", "73857293", "7"), event("message", "next", "7")]);
}

#[test]
fn every_line_ending_and_every_split() {
    let lf = b"\xEF\xBB\xBFevent: delta\ndata: {\"text\": \"\xD0\x9F\xD1\x80\xD0\xB8\"}\nid: a1\nretry: 2500\n\n: keep-alive\n\nevent: done\ndata: [DONE]\n\n".to_vec();
    let expected = [event("delta", "{\"text\": \"При\"}", "a1"), event("done", "[DONE]", "a1")];
    for ending in [&b"\n"[..], b"\r\n", b"\r"] {
        let text: Vec<u8> = lf.split(|&b| b == b'\n').collect::<Vec<_>>().join(ending);
        // Whole, and split at every byte boundary into two pieces.
        for cut in 0..=text.len() {
            let mut stream = Stream::new(256);
            assert_eq!(read(&mut stream, &[&text[..cut], &text[cut..]]).unwrap(), expected, "cut at {} with {:?}", cut, ending);
            assert_eq!((stream.retry(), stream.last_id()), (Some(2500), "a1"));
        }
        // One byte at a time.
        let mut stream = Stream::new(256);
        let pieces: Vec<&[u8]> = text.chunks(1).collect();
        assert_eq!(read(&mut stream, &pieces).unwrap(), expected);
    }
}

#[test]
fn fields_that_are_ignored() {
    // An unknown field, an id with a NUL, a retry that is not a number, a field without a colon.
    let text = b"foo: bar\nid: x\0y\nretry: 10s\ndata\n\n";
    let mut stream = Stream::new(64);
    assert_eq!(read(&mut stream, &[text]).unwrap(), [event("message", "", "")]);
    assert_eq!((stream.retry(), stream.last_id()), (None, ""));
    // An event without data is not dispatched, and its type does not carry over.
    assert_eq!(read(&mut Stream::new(64), &[b"event: lost\n\ndata: x\n\n"]).unwrap(), [event("message", "x", "")]);
}

#[test]
fn the_limits() {
    // A line longer than the limit, and an event's data over it across several lines.
    assert_eq!(read(&mut Stream::new(16), &[b"data: 0123456789abcdef\n\n"]), Err(Error::TooLarge));
    assert_eq!(read(&mut Stream::new(16), &[b"data: 01234567\ndata: 01234567\n\n"]), Err(Error::TooLarge));
    assert_eq!(read(&mut Stream::new(16), &[b"data: 0123456\ndata: 0123456\n\n"]).unwrap(), [event("message", "0123456\n0123456", "")]);
}

#[test]
fn a_caller_that_stops() {
    let mut stream = Stream::new(64);
    let mut seen = 0;
    assert_eq!(stream.feed(b"data: 1\n\ndata: 2\n\ndata: 3\n\n", &mut |_| { seen += 1; seen < 2 }), Ok(false));
    assert_eq!(seen, 2);
}

#[test]
fn a_thousand_events() {
    let mut text = Vec::new();
    for i in 0..1000 { text.extend_from_slice(format!("id: {}\ndata: event {}\n\n", i, i).as_bytes()); }
    let mut stream = Stream::new(64);
    let events = read(&mut stream, &text.chunks(97).collect::<Vec<_>>()).unwrap();
    assert_eq!(events.len(), 1000);
    assert!(events.iter().enumerate().all(|(i, e)| e.data == format!("event {}", i) && e.id == i.to_string()));
}
