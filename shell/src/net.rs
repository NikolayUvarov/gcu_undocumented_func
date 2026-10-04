//! `net`: diagnostics of the network card driver (idl/net.wit) through the client init grants the shell.
use core::fmt::Write;
use mind::abi::SLOT_NET;
use mind::idl::net;
use mind::ipc::Endpoint;

const SOURCE: [u8; 4] = [10, 0, 2, 15]; // the address QEMU user networking hands out
const ARP_TRIES: usize = 100; // 10 ms apart

fn mac(out: &mut impl Write, mac: &[u8]) { for (i, byte) in mac.iter().enumerate() { let _ = write!(out, "{}{:02X}", if i == 0 { "" } else { ":" }, byte); } }

fn ipv4(text: &str) -> Option<[u8; 4]> {
    let mut address = [0u8; 4]; let mut parts = text.split('.');
    for byte in address.iter_mut() { *byte = parts.next()?.parse().ok()?; }
    parts.next().is_none().then_some(address)
}

pub fn command(out: &mut impl Write, args: &[u8]) {
    let endpoint = Endpoint(SLOT_NET);
    let info = match net::info(endpoint) {
        Ok(Ok(info)) => info,
        Ok(Err(_)) | Err(_) => { let _ = writeln!(out, "NET: NO NETWORK CARD"); return; }
    };
    let own = info.mac.to_be_bytes(); let own = [own[2], own[3], own[4], own[5], own[6], own[7]];
    let text = core::str::from_utf8(args).unwrap_or("");
    let mut words = text.split_whitespace();
    match (words.next(), words.next()) {
        (None, _) => {
            let counters = net::counters(endpoint).unwrap_or_default();
            let _ = write!(out, "NET MAC="); mac(out, &own);
            let _ = writeln!(out, " LINK={} MTU={} SENT={} RECEIVED={} DROPPED={} INTERRUPTS={}", if info.link { "UP" } else { "DOWN" }, info.mtu,
                             counters.sent, counters.received, counters.dropped, counters.interrupts);
        }
        (Some(w), Some(target)) if w.eq_ignore_ascii_case("arp") => {
            let Some(target) = ipv4(target) else { let _ = writeln!(out, "NET ARP <A.B.C.D>"); return };
            arp(out, endpoint, own, target);
        }
        _ => { let _ = writeln!(out, "NET [ARP <A.B.C.D>]"); }
    }
}

// Broadcasts "who has `target`" and waits up to a second for the answer.
fn arp(out: &mut impl Write, endpoint: Endpoint, own: [u8; 6], target: [u8; 4]) {
    let mut frame = [0u8; 42];
    frame[0..6].fill(0xFF); frame[6..12].copy_from_slice(&own); frame[12..14].copy_from_slice(&[0x08, 0x06]);
    frame[14..22].copy_from_slice(&[0, 1, 0x08, 0, 6, 4, 0, 1]); // Ethernet, IPv4, request
    frame[22..28].copy_from_slice(&own); frame[28..32].copy_from_slice(&SOURCE); frame[38..42].copy_from_slice(&target);
    if !matches!(net::send(endpoint, &frame), Ok(Ok(()))) { let _ = writeln!(out, "NET: SEND FAILED"); return; }
    let mut buffer = [0u8; 1514];
    for _ in 0..ARP_TRIES {
        match net::receive(endpoint, &mut buffer) {
            Ok(Ok(len)) => {
                let reply = &buffer[..len.min(buffer.len())];
                if reply.len() >= 42 && reply[12..14] == [0x08, 0x06] && reply[20..22] == [0, 2] && reply[28..32] == target {
                    let _ = write!(out, "ARP {}.{}.{}.{} IS AT ", target[0], target[1], target[2], target[3]); mac(out, &reply[22..28]);
                    let _ = writeln!(out);
                    return;
                }
            }
            _ => { mind::time::sleep(10); }
        }
    }
    let _ = writeln!(out, "ARP {}.{}.{}.{}: NO ANSWER", target[0], target[1], target[2], target[3]);
}
