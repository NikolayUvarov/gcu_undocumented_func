//! Flow grants (issue 102): what a client with a grant's badge may reach, for how long and how much. Registered only
//! by the policy broker; the stack checks every request with a grant's badge against them.
use alloc::vec::Vec;
use mind::idl::socket::{self, Error, Protocol, Rule};
use mind::network::{BADGE_GRANT_LAST, BADGE_OPERATOR, BADGE_POLICY};

pub const GRANTS: usize = 64;

pub struct Grant { pub badge: u16, rules: Vec<Rule>, expires: u64, budget: u64, pub sent: u64, pub received: u64, pub ended: bool }

pub enum Access { Operator, Policy, Grant(u16), Nothing }

pub fn access(badge: u16) -> Access {
    match badge { BADGE_OPERATOR => Access::Operator, BADGE_POLICY => Access::Policy, 1..=BADGE_GRANT_LAST => Access::Grant(badge), _ => Access::Nothing }
}

#[derive(Default)]
pub struct Policy { grants: Vec<Grant> }

impl Policy {
    pub fn set(&mut self, badge: u16, rules: &[Rule], seconds: u32, bytes: u64, now: u64) -> Result<(), Error> {
        if !(1..=BADGE_GRANT_LAST).contains(&badge) || rules.iter().any(|r| r.address == 0) { return Err(Error::Invalid); }
        self.grants.retain(|g| g.badge != badge);
        if self.grants.len() >= GRANTS { return Err(Error::Limit); }
        self.grants.push(Grant { badge, rules: rules.to_vec(), expires: now + seconds as u64 * 1000, budget: bytes, sent: 0, received: 0, ended: false });
        Ok(())
    }
    pub fn drop(&mut self, badge: u16) -> bool { let before = self.grants.len(); self.grants.retain(|g| g.badge != badge); before != self.grants.len() }
    pub fn usage(&self, badge: u16, now: u64, sockets: u32) -> Option<socket::Usage> {
        self.grants.iter().find(|g| g.badge == badge).map(|g| socket::Usage { sent: g.sent, received: g.received, sockets, left_ms: g.expires.saturating_sub(now).min(u32::MAX as u64) as u32 })
    }
    fn live(&self, badge: u16, now: u64) -> Result<&Grant, Error> {
        let grant = self.grants.iter().find(|g| g.badge == badge).ok_or(Error::Denied)?;
        if grant.ended || now >= grant.expires { return Err(Error::Denied); }
        if grant.sent + grant.received >= grant.budget { return Err(Error::Limit); }
        Ok(grant)
    }
    /// Whether grant `badge` may reach `address`:`port` over `protocol` (port 0 asks for any rule of the protocol).
    pub fn allows(&self, badge: u16, protocol: Protocol, address: u32, port: u16, now: u64) -> Result<(), Error> {
        let grant = self.live(badge, now)?;
        let matches = |r: &Rule| r.protocol == protocol && (address == 0 || r.address == address) && (r.port == 0 || port == 0 || r.port == port);
        if grant.rules.iter().any(matches) { Ok(()) } else { Err(Error::Denied) }
    }
    /// Any request of a live grant (sockets it already holds).
    pub fn alive(&self, badge: u16, now: u64) -> Result<(), Error> { self.live(badge, now).map(drop) }
    pub fn charge(&mut self, badge: u16, sent: usize, received: usize) {
        if let Some(g) = self.grants.iter_mut().find(|g| g.badge == badge) { g.sent += sent as u64; g.received += received as u64; }
    }
    /// Grants that just ran out of time or volume: their sockets are closed once (the record stays, answering denied).
    pub fn ended(&mut self, now: u64) -> Vec<u16> {
        let mut ended = Vec::new();
        for g in self.grants.iter_mut().filter(|g| !g.ended && (now >= g.expires || g.sent + g.received >= g.budget)) { g.ended = true; ended.push(g.badge); }
        ended
    }
}
