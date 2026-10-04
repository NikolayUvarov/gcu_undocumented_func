// Per-client request rate (MC-10.2: observation has quotas): a token bucket for each client PID. A client may make
// BURST requests at once and PER_SECOND on average; the least recently refilled entry is reused for a new client.
pub const CLIENTS: usize = 16;
pub const BURST: u32 = 20;
pub const PER_SECOND: u32 = 40;

pub struct Limiter { clients: [(u64, u32, u64); CLIENTS] } // (pid, tokens, time of the last refill in ms)

impl Limiter {
    pub const fn new() -> Self { Self { clients: [(0, 0, 0); CLIENTS] } }

    /// Whether the request of `pid` at `now` (ms) is within its rate; takes a token if it is.
    pub fn admit(&mut self, pid: u64, now: u64) -> bool {
        let index = match self.clients.iter().position(|c| c.0 == pid) {
            Some(index) => index,
            None => {
                let index = self.clients.iter().enumerate().min_by_key(|(_, c)| c.2).map_or(0, |(i, _)| i);
                self.clients[index] = (pid, BURST, now);
                index
            }
        };
        let client = &mut self.clients[index];
        let refill = (now.saturating_sub(client.2) * PER_SECOND as u64 / 1000) as u32;
        if refill > 0 { client.1 = (client.1 + refill).min(BURST); client.2 = now; }
        if client.1 == 0 { return false; }
        client.1 -= 1;
        true
    }
}
