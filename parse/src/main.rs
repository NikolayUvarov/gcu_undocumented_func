#![no_std]
#![no_main]
// Parser service (109-NET-0008, MC-11.11, Appendix B.6's session parser): bounded bytes from outside in, typed messages
// out (idl/parse.wit), so the program that holds the network and the files does not parse them itself: HTTP response
// heads, release channels and boot manifests for the updater (351-NET-0011), and a model disk's manifest for the importer
// into the block store (251-STO-0014), and any JSON text as its values (750-NET-0021). Holds its own endpoint and the system log, nothing else: no files, no network, no
// spawn, no devices. Each request is parsed on its own and nothing of it is kept; a refusal is logged with the client's
// PID. Started by the loader as a helper with `--session` (109-NET-0010), it serves one session's endpoint and holds
// nothing else, not even the log, and writes nothing: it ends on a message that is not a call, or after IDLE_MS without
// a request. A refusal reaches its client as the answer.
use mind::abi::BootInfo;
use mind::http;
use mind::idl::codec::Text;
use mind::idl::parse::{self, Error};
use mind::json;
use mind::idl::wire;
use mind::ipc::Endpoint;
use mind::release;

const RECEIVED: usize = 9;
const IDLE_MS: u32 = 30_000; // a session's parser left alone this long ends
// A request of up to 60 000 bytes (a model manifest): kept out of the stack.
static mut SCRATCH: [u8; parse::REQUEST_MAX] = [0; parse::REQUEST_MAX];

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let session = mind::process::args_str().split_whitespace().any(|a| a == "--session");
    if !session { mind::println!("[PARSE] READY: HTTP RESPONSE HEADS, RELEASE CHANNELS, BOOT MANIFESTS, MODEL MANIFESTS, JSON"); }
    let scratch = unsafe { &mut *core::ptr::addr_of_mut!(SCRATCH) };
    loop {
        // SLOT_INIT is SLOT_SERVICE: a session's endpoint is served the same way.
        let request = if session { Endpoint::SERVICE.recv_timeout(RECEIVED, IDLE_MS) } else { Endpoint::SERVICE.recv(RECEIVED) };
        let Ok(request) = request else { if session { return } else { continue } };
        if !request.is_call { if session { return } else { continue } }
        let _ = match parse::decode(&request, RECEIVED, scratch) {
            Err(reason) => wire::reject(reason),
            Ok((parse::Request::HttpHead { head }, call)) => {
                let typed = http::parse_head(head).map(|head| mind::parse::to_record(&head)).map_err(|_| {
                    if !session { mind::println!("[PARSE] REFUSED AN HTTP HEAD FOR PID {}: MALFORMED ({} BYTES)", request.sender, head.len()); }
                    Error::Malformed
                });
                parse::reply_http_head(call, typed.as_ref().map_err(|e| *e))
            }
            Ok((parse::Request::HttpResponse { head }, call)) => {
                let typed = http::parse_head(head).map(|head| mind::parse::to_response(&head)).map_err(|_| {
                    mind::println!("[PARSE] REFUSED AN HTTP HEAD FOR PID {}: MALFORMED ({} BYTES)", request.sender, head.len());
                    Error::Malformed
                });
                parse::reply_http_response(call, typed.as_ref().map_err(|e| *e))
            }
            Ok((parse::Request::Json { text, start }, call)) => {
                // The whole text is read on every page; the values from `start` are kept (750-NET-0021).
                let mut page = parse::JsonPage::default();
                let walked = json::walk(text, &mut |index, value| {
                    if (index as u32) < start || page.page.as_slice().len() == 16 { return Ok(()); }
                    let kind = match value.kind {
                        json::Kind::Object => parse::JsonKind::Object, json::Kind::Array => parse::JsonKind::Array, json::Kind::String => parse::JsonKind::String,
                        json::Kind::Number => parse::JsonKind::Number, json::Kind::True => parse::JsonKind::True, json::Kind::False => parse::JsonKind::False,
                        json::Kind::Null => parse::JsonKind::Null,
                    };
                    let key = core::str::from_utf8(value.key).ok().and_then(Text::new).ok_or(json::Malformed)?;
                    let text = core::str::from_utf8(value.text).ok().and_then(Text::new).ok_or(json::Malformed)?;
                    page.page.push(parse::JsonValue { kind, parent: value.parent.unwrap_or(u16::MAX), key, text });
                    Ok(())
                });
                let typed = match walked {
                    Ok(values) => { page.values = values as u32; Ok(&page) }
                    Err(_) => {
                        mind::println!("[PARSE] REFUSED JSON FOR PID {}: MALFORMED OR PAST ITS BOUNDS ({} BYTES)", request.sender, text.len());
                        Err(Error::Malformed)
                    }
                };
                parse::reply_json(call, typed)
            }
            Ok((parse::Request::Channel { file }, call)) => {
                let typed = release::parse_channel(file).map(|(channel, signed)| mind::parse::channel_record(&channel, &signed)).map_err(|_| {
                    if !session { mind::println!("[PARSE] REFUSED A CHANNEL FOR PID {}: MALFORMED ({} BYTES)", request.sender, file.len()); }
                    Error::Malformed
                });
                parse::reply_channel(call, typed.as_ref().map_err(|e| *e))
            }
            Ok((parse::Request::Manifest { text, start }, call)) => {
                let typed = release::Manifest::parse(text).map(|manifest| mind::parse::manifest_record(&manifest, start)).map_err(|_| {
                    if !session { mind::println!("[PARSE] REFUSED A MANIFEST FOR PID {}: MALFORMED ({} BYTES)", request.sender, text.len()); }
                    Error::Malformed
                });
                parse::reply_manifest(call, typed.as_ref().map_err(|e| *e))
            }
            Ok((parse::Request::Model { text, index, start }, call)) => {
                let typed = mind::parse::model_record(text, index, start).map_err(|_| {
                    if !session { mind::println!("[PARSE] REFUSED A MODEL MANIFEST FOR PID {}: MALFORMED OR NO MODEL {} ({} BYTES)", request.sender, index, text.len()); }
                    Error::Malformed
                });
                parse::reply_model(call, typed.as_ref().map_err(|e| *e))
            }
        };
    }
}
