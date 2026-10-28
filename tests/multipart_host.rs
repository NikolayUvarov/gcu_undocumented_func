//! Host tests of `multipart/form-data` bodies (libmind/src/multipart.rs, 000-NET-0047): the body as RFC 7578 writes
//! it, its length known first, a file streamed in uneven pieces, the boundary refused in a field and found in a file
//! across reads, names that cannot be written, and a file that gives too few bytes.
#[path = "../libmind/src/http.rs"]
#[allow(dead_code)]
mod http;
#[path = "../libmind/src/multipart.rs"]
mod multipart;

use http::{Error, Source};
use multipart::{Multipart, Part, BOUNDARY_LEN};

const RANDOM: [u8; 16] = [0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0xFE, 0xDC, 0xBA, 0x98, 0x76, 0x54, 0x32, 0x10];
const BOUNDARY: &str = "MIND-0123456789abcdeffedcba9876543210";

/// A file given in pieces of `step` bytes.
struct Pieces { data: Vec<u8>, at: usize, step: usize }
impl Source for Pieces {
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Error> {
        let n = buffer.len().min(self.step).min(self.data.len() - self.at);
        buffer[..n].copy_from_slice(&self.data[self.at..self.at + n]);
        self.at += n;
        Ok(n)
    }
}

fn all(body: &mut Multipart, read: usize) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    let mut buffer = vec![0u8; read];
    loop {
        let n = body.read(&mut buffer)?;
        if n == 0 { return Ok(out); }
        out.extend_from_slice(&buffer[..n]);
    }
}

#[test]
fn a_body_as_rfc_7578_writes_it() {
    for read in [1, 3, 7, 64, 4096] {
        let mut file = Pieces { data: b"%PDF-1.7 a document".to_vec(), at: 0, step: 5 };
        let mut parts = [
            Part::Field { name: "chat_id", value: b"12345" },
            Part::File { name: "document", filename: "report.pdf", media: "application/pdf", length: 19, source: &mut file },
            Part::Field { name: "caption", value: "Отчёт".as_bytes() },
        ];
        let mut body = Multipart::new(&mut parts, RANDOM).unwrap();
        assert_eq!(body.boundary(), BOUNDARY);
        assert_eq!(BOUNDARY.len(), BOUNDARY_LEN);
        let length = body.length();
        let bytes = all(&mut body, read).unwrap();
        let expected = format!("--{b}\r\nContent-Disposition: form-data; name=\"chat_id\"\r\n\r\n12345\r\n\
                                --{b}\r\nContent-Disposition: form-data; name=\"document\"; filename=\"report.pdf\"\r\nContent-Type: application/pdf\r\n\r\n%PDF-1.7 a document\r\n\
                                --{b}\r\nContent-Disposition: form-data; name=\"caption\"\r\n\r\nОтчёт\r\n--{b}--\r\n", b = BOUNDARY);
        assert_eq!(String::from_utf8(bytes.clone()).unwrap(), expected, "read {}", read);
        assert_eq!(length, bytes.len() as u64);
    }
}

#[test]
fn the_boundary_must_not_occur_in_a_part() {
    // In a field: refused before anything is sent.
    let value = format!("x--{}y", BOUNDARY);
    assert!(matches!(Multipart::new(&mut [Part::Field { name: "a", value: value.as_bytes() }], RANDOM), Err(Error::Body)));
    // In a file, split across reads and after a partial match ("---" before it): found as it streams.
    for step in [1, 2, 9, 40, 1000] {
        let data = format!("{}---{}--{}tail", "z".repeat(50), &BOUNDARY[..10], BOUNDARY).into_bytes();
        let mut file = Pieces { data: data.clone(), at: 0, step };
        let length = data.len() as u64;
        let mut parts = [Part::File { name: "f", filename: "f.bin", media: "application/octet-stream", length, source: &mut file }];
        let mut body = Multipart::new(&mut parts, RANDOM).unwrap();
        assert_eq!(all(&mut body, 4096), Err(Error::Body), "step {}", step);
    }
    // A near miss is sent as it is.
    let data = format!("--{}", &BOUNDARY[..BOUNDARY_LEN - 1]).into_bytes();
    let mut file = Pieces { data: data.clone(), at: 0, step: 3 };
    let mut parts = [Part::File { name: "f", filename: "f.bin", media: "application/octet-stream", length: data.len() as u64, source: &mut file }];
    assert!(all(&mut Multipart::new(&mut parts, RANDOM).unwrap(), 10).is_ok());
}

#[test]
fn what_cannot_be_written() {
    let mut empty = Pieces { data: Vec::new(), at: 0, step: 1 };
    for (name, filename, media) in [("a\"b", "f", "a/b"), ("a", "f\r\n", "a/b"), ("", "f", "a/b"), ("a", "f", "nomedia"), ("a", "f\\", "a/b"), ("a", "f", "a/b c")] {
        let mut parts = [Part::File { name, filename, media, length: 0, source: &mut empty }];
        assert!(matches!(Multipart::new(&mut parts, RANDOM), Err(Error::Body)), "{:?}", (name, filename, media));
    }
}

#[test]
fn a_file_that_gives_too_few_bytes() {
    let mut file = Pieces { data: b"short".to_vec(), at: 0, step: 2 };
    let mut parts = [Part::File { name: "f", filename: "f.txt", media: "text/plain", length: 10, source: &mut file }];
    assert_eq!(all(&mut Multipart::new(&mut parts, RANDOM).unwrap(), 4096), Err(Error::Body));
}
