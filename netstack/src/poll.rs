// A finite ingress burst leaves time for egress, other cards and client requests (550-NET-0031).
use smoltcp::iface::{Interface, PollIngressSingleResult, SocketSet};
use smoltcp::phy::Device;
use smoltcp::time::Instant;

pub const BURST: usize = 64;

pub fn interface<D: Device>(iface: &mut Interface, device: &mut D, sockets: &mut SocketSet<'_>, now: Instant, available: impl Fn() -> bool) {
    iface.poll_maintenance(now);
    for _ in 0..BURST {
        if !available() || matches!(iface.poll_ingress_single(now, device, sockets), PollIngressSingleResult::None) { break; }
    }
    // One egress pass is bounded by the socket set; devices refuse tokens after their deadline.
    if available() { let _ = iface.poll_egress(now, device, sockets); }
}
