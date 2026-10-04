#![no_std]
#![no_main]
// netcheck <check>...: tries network access with the flow grant its launcher got from the policy broker (issue 102)
// and prints one line per check: OK, or why not. Checks: tcp:A.B.C.D:PORT (connect), udp:A.B.C.D:PORT (send a
// datagram), ping:A.B.C.D, dns:NAME, hold:A.B.C.D:PORT:SECONDS (keep a connection and send once a second until it
// fails or the time is up). A console program; without a grant every check reports NO GRANT.
use mind::abi::{BootInfo, SLOT_NETWORK};
use mind::idl::socket;
use mind::ipc::Endpoint;

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
        _ => Err(socket::Error::Invalid),
    })
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("netcheck — tries network access with the grant the policy broker gave it, one line per check.\nUsage: netcheck <check>...   tcp:A.B.C.D:PORT  udp:A.B.C.D:PORT  ping:A.B.C.D  dns:NAME  hold:A.B.C.D:PORT:SECONDS");
    for word in mind::process::args_str().split_whitespace() { report(word, check(word)); }
}
