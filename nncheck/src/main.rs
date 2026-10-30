#![no_std]
#![no_main]
// nncheck [hold SECONDS]: the test program of the model service nnd (262-APP-0112, docs/nn/runtime.md section 8). It
// checks the client the shell lends for REQUEST_NN: that there is one, its badge (one of this start's, never 0) and
// rights (no right to badge), nnd's status through it, and that a retirement from it is refused (`owner`: only the
// shell's badge 0 retires). `hold` then keeps the client that many seconds, for the shell's `nn status` to show.
// One line each, `NNCHECK <what> <result>`; exit 0 when every check holds.
use mind::abi::{BootInfo, CAP_BADGE, CAP_KIND_ENDPOINT, SLOT_NN};
use mind::idl::{nn, wire};
use mind::ipc::Endpoint;

mind::request!(REQUEST_CONSOLE | REQUEST_NN);

const NND: Endpoint = Endpoint(SLOT_NN);

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("nncheck — checks the model service's client the shell lent this start: its badge and rights, nnd's status, a retirement refused.\nUsage: nncheck [hold SECONDS]");
    let (kind, badge, rights) = mind::dev::cap_info(SLOT_NN);
    if kind != CAP_KIND_ENDPOINT { mind::println!("NNCHECK CLIENT NONE"); mind::process::exit_with(1); }
    let mut ok = badge != 0 && rights & CAP_BADGE as usize == 0;
    mind::println!("NNCHECK CLIENT BADGE={} RIGHTS={} {}", badge, rights, if ok { "OK" } else { "WRONG" });
    match wire::with_timeout(2000, || nn::status(NND)) {
        Ok(s) => mind::println!("NNCHECK STATUS QUOTA={} USED={} WORKERS={}/{} MODELS={} SESSIONS={} BADGES={}", s.quota, s.used, s.workers, s.workers_max, s.models, s.sessions, s.badges),
        Err(error) => { ok = false; mind::println!("NNCHECK STATUS {:?}", error); }
    }
    for (what, answer) in [("RETIRE", wire::with_timeout(2000, || nn::retire(NND, badge as u16))), ("RETIRE-ALL", wire::with_timeout(2000, || nn::retire_all(NND)))] {
        let refused = answer == Ok(Err(nn::Error::Owner));
        ok &= refused;
        mind::println!("NNCHECK {} {:?} {}", what, answer, if refused { "REFUSED" } else { "WRONG" });
    }
    let mut words = mind::process::args_str().split_whitespace();
    if words.next() == Some("hold") {
        let seconds: u64 = words.next().and_then(|w| w.parse().ok()).unwrap_or(60);
        mind::println!("NNCHECK HOLD {} S", seconds);
        let _ = mind::time::sleep(seconds as usize * 1000);
    }
    mind::println!("NNCHECK {}", if ok { "DONE" } else { "FAILED" });
    mind::process::exit_with(if ok { 0 } else { 1 });
}
