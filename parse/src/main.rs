#![no_std]
#![no_main]
// Parser service (109-NET-0008, MC-11.11, Appendix B.6's session parser): bounded bytes from outside in, typed messages
// out (idl/parse.wit), so the program that holds the network and the files does not parse them itself. Holds its own
// endpoint and the system log, nothing else: no files, no network, no spawn, no devices. Each request is parsed on its
// own and nothing of it is kept; a refusal is logged with the client's PID.
use mind::abi::BootInfo;
use mind::http;
use mind::idl::parse::{self, Error};
use mind::idl::wire;
use mind::ipc::Endpoint;

const RECEIVED: usize = 9;

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::println!("[PARSE] READY: HTTP RESPONSE HEADS");
    let mut scratch = [0u8; parse::REQUEST_MAX];
    loop {
        let Ok(request) = Endpoint::SERVICE.recv(RECEIVED) else { continue };
        if !request.is_call { continue; }
        let _ = match parse::decode(&request, RECEIVED, &mut scratch) {
            Err(reason) => wire::reject(reason),
            Ok((parse::Request::HttpHead { head }, call)) => {
                let typed = http::parse_head(head).map(|head| mind::parse::to_record(&head)).map_err(|_| {
                    mind::println!("[PARSE] REFUSED AN HTTP HEAD FOR PID {}: MALFORMED ({} BYTES)", request.sender, head.len());
                    Error::Malformed
                });
                parse::reply_http_head(call, typed.as_ref().map_err(|e| *e))
            }
        };
    }
}
