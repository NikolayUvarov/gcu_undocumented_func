#![no_std]
#![no_main]
//! Test-only console program (QEMU `shell` suite, 000-APP-0126): asks the shell for one secret line through the copy of
//! its client badged BADGE_SHELL_SECRET in SLOT_SHELL, and prints what came back as its length and SHA-256, never the
//! bytes; with `other`, asks the machine's name through that copy, which the shell refuses; with `later`, asks 3 s
//! after its start. Never packaged into the normal OS; the program the line is for is NET's `get -u`.
use core::fmt::Write;
use mind::abi::*;
use mind::idl::shell;
use mind::ipc::Endpoint;

mind::request!(REQUEST_CONSOLE | REQUEST_SECRET_LINE);

fn said(error: shell::Error) -> &'static str {
    match error { shell::Error::Refused => "REFUSED", shell::Error::Declined => "DECLINED", shell::Error::Busy => "BUSY", shell::Error::Unavailable => "UNAVAILABLE", _ => "ANOTHER ERROR" }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let client = Endpoint(SLOT_SHELL);
    if mind::dev::cap_info(SLOT_SHELL).0 != CAP_KIND_ENDPOINT { mind::println!("SECRET STUB: NO CLIENT OF THE SHELL"); mind::process::exit_with(3) }
    if mind::process::args_str().trim() == "other" {
        let answer = match shell::hostname(client) { Ok(Ok(_)) => "ANSWERED", Ok(Err(error)) => said(error), Err(_) => "FAILED" };
        mind::println!("SECRET STUB: HOSTNAME THROUGH THE SECRET CLIENT {}", answer);
        mind::process::exit_with(0)
    }
    // `later`: the call 3 s after the start, for a line another program is typing meanwhile.
    if mind::process::args_str().trim() == "later" { let _ = mind::time::sleep(3000); }
    let mut line = [0u8; 256];
    // The user types it: no time limit of the stub's own.
    match shell::secret_line(client, "PASSWORD FOR THE STUB", &mut line) {
        Ok(Ok(len)) => {
            let digest = mind::sha256::digest(&line[..len]);
            line.fill(0);
            let mut hex = mind::util::FixedBuf::<64>::new();
            for byte in digest { let _ = write!(hex, "{:02x}", byte); }
            mind::println!("SECRET STUB: GIVEN {} BYTES, SHA-256 {}", len, hex.as_str());
        }
        Ok(Err(error)) => mind::println!("SECRET STUB: {}", said(error)),
        Err(error) => mind::println!("SECRET STUB: THE CALL FAILED ({:?})", error),
    }
    mind::process::exit_with(0)
}
