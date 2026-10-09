#![no_std]
#![no_main]
// Parser service (109-NET-0008, MC-11.11, Appendix B.6's session parser): bounded bytes from outside in, typed messages
// out (idl/parse.wit), so the program that holds the network and the files does not parse them itself: HTTP response
// heads, and release channels and boot manifests for the updater (351-NET-0011). Holds its own endpoint and the system
// log, nothing else: no files, no network, no spawn, no devices. Each request is parsed on its own and nothing of it is
// kept; a refusal is logged with the client's PID.
use mind::abi::BootInfo;
use mind::http;
use mind::idl::parse::{self, Error};
use mind::idl::wire;
use mind::ipc::Endpoint;
use mind::release;

const RECEIVED: usize = 9;
// A request of up to 32 KiB (a manifest): kept out of the stack.
static mut SCRATCH: [u8; parse::REQUEST_MAX] = [0; parse::REQUEST_MAX];

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::println!("[PARSE] READY: HTTP RESPONSE HEADS, RELEASE CHANNELS, BOOT MANIFESTS");
    let scratch = unsafe { &mut *core::ptr::addr_of_mut!(SCRATCH) };
    loop {
        let Ok(request) = Endpoint::SERVICE.recv(RECEIVED) else { continue };
        if !request.is_call { continue; }
        let _ = match parse::decode(&request, RECEIVED, scratch) {
            Err(reason) => wire::reject(reason),
            Ok((parse::Request::HttpHead { head }, call)) => {
                let typed = http::parse_head(head).map(|head| mind::parse::to_record(&head)).map_err(|_| {
                    mind::println!("[PARSE] REFUSED AN HTTP HEAD FOR PID {}: MALFORMED ({} BYTES)", request.sender, head.len());
                    Error::Malformed
                });
                parse::reply_http_head(call, typed.as_ref().map_err(|e| *e))
            }
            Ok((parse::Request::Channel { file }, call)) => {
                let typed = release::parse_channel(file).map(|(channel, signed)| mind::parse::channel_record(&channel, &signed)).map_err(|_| {
                    mind::println!("[PARSE] REFUSED A CHANNEL FOR PID {}: MALFORMED ({} BYTES)", request.sender, file.len());
                    Error::Malformed
                });
                parse::reply_channel(call, typed.as_ref().map_err(|e| *e))
            }
            Ok((parse::Request::Manifest { text, start }, call)) => {
                let typed = release::Manifest::parse(text).map(|manifest| mind::parse::manifest_record(&manifest, start)).map_err(|_| {
                    mind::println!("[PARSE] REFUSED A MANIFEST FOR PID {}: MALFORMED ({} BYTES)", request.sender, text.len());
                    Error::Malformed
                });
                parse::reply_manifest(call, typed.as_ref().map_err(|e| *e))
            }
        };
    }
}
