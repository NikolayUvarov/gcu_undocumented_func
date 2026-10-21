#![no_std]
#![no_main]
// netcheck <check>...: tries network access with the flow grant its launcher got from the policy broker (issue 102)
// and prints one line per check: OK, or why not. Checks: tcp:A.B.C.D:PORT (connect), udp:A.B.C.D:PORT (send a
// datagram), ping:A.B.C.D, dns:NAME, hold:A.B.C.D:PORT:SECONDS (keep a connection and send once a second until it
// fails or the time is up), open:A.B.C.D:PORT (connect and keep the connection until the program ends), sockets (the
// stack's list of this grant's open sockets, socket.wit 2.4), cards (each card's kind and state, which one is preferred
// and which is the default, socket.wit 2.5), card (the card client in SLOT_NET, which a launcher lends read-only for
// REQUEST_NET_COUNTERS: its info and counters read, a frame sent or received and a ring lent each refused, 000-NET-0015),
// sessions (ten parser sessions in a row, each with a helper of its own, with one endpoint left in the quota, 109-KRN-0133).
// A console program; without a grant every network check reports NO GRANT.
use mind::abi::{BootInfo, CAP_KIND_ENDPOINT, SLOT_NET, SLOT_NETWORK};
use mind::idl::{net, socket};
use mind::ipc::Endpoint;
use mind::http::Parser as _;
use mind::mem::Pages;
use mind::parse::Session;

mind::request!(REQUEST_CONSOLE | REQUEST_NETWORK);

const STACK: Endpoint = Endpoint(SLOT_NETWORK);

fn ipv4(text: &str) -> Option<u32> {
    let mut address = [0u8; 4]; let mut parts = text.split('.');
    for byte in address.iter_mut() { *byte = parts.next()?.parse().ok()?; }
    parts.next().is_none().then_some(u32::from_be_bytes(address))
}

fn report(check: &str, result: mind::sys::Result<Result<(), socket::Error>>) {
    match result {
        Ok(Ok(())) => mind::println!("NETCHECK {} OK", check),
        Ok(Err(error)) => mind::println!("NETCHECK {} {:?}", check, error),
        Err(mind::sys::Error::Rights) | Err(mind::sys::Error::Invalid) => mind::println!("NETCHECK {} NO GRANT", check),
        Err(error) => mind::println!("NETCHECK {} {:?}", check, error),
    }
}

fn hold(address: u32, port: u16, seconds: u32) -> mind::sys::Result<Result<(), socket::Error>> {
    let handle = match socket::tcp_connect(STACK, address, port, 3000)? { Ok(handle) => handle, Err(error) => return Ok(Err(error)) };
    mind::println!("NETCHECK HOLDING {}", handle);
    for _ in 0..seconds {
        if let Err(error) = socket::tcp_send(STACK, handle, b"x")? { return Ok(Err(error)); }
        mind::time::sleep(1000);
    }
    let _ = socket::close(STACK, handle);
    Ok(Ok(()))
}

// One line per socket the stack lists for this grant (socket.wit 2.4).
fn sockets() -> mind::sys::Result<Result<(), socket::Error>> {
    let list = match socket::sockets(STACK, 0)? { Ok(list) => list, Err(error) => return Ok(Err(error)) };
    for s in list.as_slice() {
        let [a, b, c, d] = s.remote_address.to_be_bytes();
        mind::println!("NETCHECK SOCKET {} {:?} LOCAL {} REMOTE {}.{}.{}.{}:{} {:?} BADGE {} SENT {} RECEIVED {} CARD {}",
            s.id, s.protocol, s.local_port, a, b, c, d, s.remote_port, s.state, s.badge, s.sent, s.received, s.card);
    }
    mind::println!("NETCHECK {} SOCKETS", list.as_slice().len());
    Ok(Ok(()))
}

fn cards() -> mind::sys::Result<Result<(), socket::Error>> {
    let list = match socket::cards(STACK)? { Ok(list) => list, Err(error) => return Ok(Err(error)) };
    for c in list.as_slice() {
        let flag = |on: bool, word: &'static str| if on { word } else { "" };
        mind::println!("NETCHECK CARD {} {:?}{}{}{}{}{}", c.card, c.kind, flag(c.present, " PRESENT"), flag(c.up, " UP"), flag(c.configured, " CONFIGURED"),
                       flag(c.preferred, " PREFERRED"), flag(c.default, " DEFAULT"));
    }
    Ok(Ok(()))
}

// Ten parser sessions in a row with one endpoint free, as the updater has: each has a helper of its own and parses a
// head, and its end frees the endpoint for the next (109-KRN-0133).
fn sessions() -> mind::sys::Result<Result<(), socket::Error>> {
    let mut held = [0usize; 16];
    let mut count = 0;
    while count < held.len() {
        match Endpoint::create() { Ok(endpoint) => { held[count] = endpoint.0; count += 1; } Err(_) => break }
    }
    if count == 0 { mind::println!("NETCHECK sessions: NO ENDPOINT IN THE QUOTA"); return Ok(Err(socket::Error::Invalid)); }
    count -= 1;
    let _ = mind::ipc::drop_cap(held[count]);
    let mut own = 0;
    for round in 1..=10 {
        let mut session = match Session::own(Endpoint::LOADER) {
            Ok(session) => session,
            Err(why) => { mind::println!("NETCHECK sessions: SESSION {}: NO PARSER OF ITS OWN ({:?})", round, why); break }
        };
        match session.head(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\n") {
            Ok(head) if head.status == 200 && head.length == Some(5) => own += 1,
            other => { mind::println!("NETCHECK sessions: SESSION {}: {:?}", round, other.map(|h| h.status)); break }
        }
    }
    for endpoint in &held[..count] { let _ = mind::ipc::drop_cap(*endpoint); }
    mind::println!("NETCHECK sessions: {} OF 10 WITH A PARSER OF THEIR OWN, {} ENDPOINTS HELD", own, count);
    Ok(if own == 10 { Ok(()) } else { Err(socket::Error::Invalid) })
}

// The card client in SLOT_NET: what it may read, and what a read-only one is refused (000-NET-0015).
fn card() -> mind::sys::Result<Result<(), socket::Error>> {
    let nic = Endpoint(SLOT_NET);
    if mind::dev::cap_info(SLOT_NET).0 != CAP_KIND_ENDPOINT { mind::println!("NETCHECK card NO CARD CLIENT"); return Ok(Err(socket::Error::Invalid)); }
    match net::info(nic)? {
        Ok(info) => mind::println!("NETCHECK card INFO MAC {:012X} MTU {} LINK {}", info.mac, info.mtu, if info.link { "UP" } else { "DOWN" }),
        Err(error) => mind::println!("NETCHECK card INFO {:?}", error),
    }
    let counted = net::counters(nic)?;
    mind::println!("NETCHECK card COUNTERS SENT {} RECEIVED {} DROPPED {}", counted.sent, counted.received, counted.dropped);
    let shown = |what: &str, result: Result<(), net::Error>| match result { Ok(()) => mind::println!("NETCHECK card {} OK", what), Err(error) => mind::println!("NETCHECK card {} {:?}", what, error) };
    shown("SEND", net::send(nic, &[0xFF; 60])?);
    let mut frame = [0u8; 1514];
    shown("RECEIVE", net::receive(nic, &mut frame)?.map(drop));
    let pages = Pages::new(4096).ok_or(mind::sys::Error::NoMemory)?;
    let lent = pages.share()?;
    shown("ATTACH", net::attach(nic, lent)?.map(drop));
    let _ = mind::ipc::revoke(lent);
    let _ = mind::ipc::drop_cap(lent);
    Ok(Ok(()))
}

fn check(text: &str) -> mind::sys::Result<Result<(), socket::Error>> {
    let mut parts = text.split(':');
    let (kind, a, b, c) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    let address = ipv4(a).unwrap_or(0);
    let port = b.parse::<u16>().unwrap_or(0);
    Ok(match kind {
        "tcp" => socket::tcp_connect(STACK, address, port, 3000)?.map(|handle| { let _ = socket::close(STACK, handle); }),
        "udp" => match socket::udp_open(STACK, 0)? {
            Ok(handle) => { let sent = socket::udp_send(STACK, handle, address, port, b"mind")?; let _ = socket::close(STACK, handle); sent }
            Err(error) => Err(error),
        },
        "ping" => socket::ping(STACK, address, 1000)?.map(drop),
        "dns" => socket::resolve(STACK, a, 0, 0, 3000)?.map(drop),
        "hold" => return hold(address, port, c.parse().unwrap_or(10)),
        // A request's first line only, so the connection has moved bytes and a web server waits for the rest; left
        // open for a later `sockets`.
        "open" => socket::tcp_connect(STACK, address, port, 3000)?.map(|handle| { let _ = socket::tcp_send(STACK, handle, b"GET / HTTP/1.0\r\n"); }),
        "sockets" => return sockets(),
        "cards" => return cards(),
        "card" => return card(),
        "sessions" => return sessions(),
        _ => Err(socket::Error::Invalid),
    })
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("netcheck — tries network access with the grant the policy broker gave it, one line per check.\nUsage: netcheck <check>...   tcp:A.B.C.D:PORT  udp:A.B.C.D:PORT  ping:A.B.C.D  dns:NAME  hold:A.B.C.D:PORT:SECONDS  open:A.B.C.D:PORT  sockets  cards  card  sessions");
    for word in mind::process::args_str().split_whitespace() { report(word, check(word)); }
}
