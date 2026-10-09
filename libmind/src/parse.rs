//! The parser service's client side (109-NET-0008, idl/parse.wit): typed HTTP heads to and from `mind::http::Head`, and
//! a `mind::http::Parser` that asks the service, so a program that holds the network and its files parses no response
//! head itself (MC-11.11). `mind::http::get` checks whatever comes back against what it asked for (MC-11.5).
use crate::http::{self, Head, Range};
use crate::idl::parse::{self, HttpHead};
use crate::ipc::Endpoint;

pub fn to_record(head: &Head) -> HttpHead {
    let (has_range, range_unsatisfied, range_start, range_end, range_total) = match head.range {
        None => (false, false, 0, 0, 0),
        Some(Range::Bytes { start, end, total }) => (true, false, start, end, total),
        Some(Range::Unsatisfied { total }) => (true, true, 0, 0, total),
    };
    HttpHead { status: head.status, has_length: head.length.is_some(), length: head.length.unwrap_or(0), has_range, range_unsatisfied, range_start, range_end, range_total, chunked: head.chunked }
}

pub fn from_record(record: &HttpHead) -> Head {
    let range = match (record.has_range, record.range_unsatisfied) {
        (false, _) => None,
        (true, true) => Some(Range::Unsatisfied { total: record.range_total }),
        (true, false) => Some(Range::Bytes { start: record.range_start, end: record.range_end, total: record.range_total }),
    };
    Head { status: record.status, length: record.has_length.then_some(record.length), range, chunked: record.chunked }
}

/// A `mind::http::Parser` that asks the parser service at the endpoint (`SLOT_PARSE` in a program that asked for it).
pub struct Service(pub Endpoint);

impl http::Parser for Service {
    fn head(&mut self, head: &[u8]) -> Result<Head, http::Error> {
        match parse::http_head(self.0, head) {
            Ok(Ok(record)) => Ok(from_record(&record)),
            Ok(Err(parse::Error::Malformed)) => Err(http::Error::Head),
            Err(_) => Err(http::Error::Parser),
        }
    }
}
