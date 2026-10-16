// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2020 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! Normal H2C headers and bounded ACK tracking; firmware download uses a separate sequence contract.

use crate::{c2h::{Ack, Identity}, fwcmd, io::{RegisterIo, TimedIo}};

pub const HEADER_BYTES: usize = 8;
pub const MAX_BODY: usize = fwcmd::MAX_PAYLOAD - HEADER_BYTES;
pub const PENDING: usize = 8;
pub const MAX_TIMEOUT_US: u32 = 1_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Command { pub category: u8, pub class: u8, pub function: u8 }
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Acks { pub received: bool, pub done: bool }
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Ticket { identity: Identity, generation: u64 }
impl Ticket { pub fn identity(self) -> Identity { self.identity } }
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome { Submitted, Received, Done, Rejected(u8) }
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Failure { Timeout, Clock, Transport(fwcmd::Error), Aborted }
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error { Request, Full, Exhausted, Unknown, Failed(Failure) }
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Progress { Pending { received: bool, done: bool }, Complete(Outcome) }
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Observation { Unrelated, Duplicate, Unexpected, Progress, Complete }

/// Full must mean no descriptor was published; other errors may follow publication and cannot be retried.
pub trait Transport {
    fn now_us(&mut self) -> u64;
    fn submit(&mut self, packet: &[u8], timeout_us: u32) -> Result<(), fwcmd::Error>;
}

#[cfg(test)]
#[path="../../tests/rtw89_h2c_runtime_cases.rs"]
mod runtime_tests;

/// Production channel-12 adapter; the caller retains all DMA memory and independently services RX/C2H.
pub struct CommandQueue<'a, 'dma, B> { pub bus: &'a mut B, pub queue: &'a mut fwcmd::Queue<'dma> }
impl<B: RegisterIo> Transport for CommandQueue<'_, '_, B> {
    fn now_us(&mut self) -> u64 { self.bus.now_us() }
    fn submit(&mut self, packet: &[u8], timeout_us: u32) -> Result<(), fwcmd::Error> {
        let mut timed = TimedIo::new(self.bus, u64::from(timeout_us));
        self.queue.submit(&mut timed, packet, false)?;
        timed.delay_us(0).map_err(fwcmd::Error::Io)
    }
}

#[derive(Clone, Copy)]
struct Pending {
    identity: Identity, generation: u64, deadline: u64, wants: Acks, received: bool,
    done: bool, rejected: Option<u8>, outcome: Option<Outcome>,
}

/// One exclusive fresh firmware session, starting at sequence zero; do not reconstruct against a live session.
/// Startup refuses wire-tag reuse; the owned serial runtime explicitly opts in after hardware quiescence.
pub struct Engine { slots: [Option<Pending>; PENDING], next: u64, last: u64, failed: Option<Failure>, serial: bool }

impl Engine {
    pub const fn new(now_us: u64) -> Self { Self { slots: [None; PENDING], next: 0, last: now_us, failed: None, serial: false } }
    pub fn failed(&self) -> Option<Failure> { self.failed }
    pub fn submitted(&self) -> u64 { self.next }
    pub fn abort(&mut self) { if self.failed.is_none() { self.failed = Some(Failure::Aborted); } }

    /// The actual owner must first validate CH12 consumption/idle; no software reset or RX-empty barrier is implied.
    pub(crate) fn enable_serial(&mut self, now_us: u64) -> Result<(), Error> {
        self.poll(now_us)?;
        if self.slots.iter().any(Option::is_some) { return Err(Error::Full); }
        if self.next == u64::MAX { return Err(Error::Exhausted); }
        self.serial = true; Ok(())
    }

    fn fail<T>(&mut self, cause: Failure) -> Result<T, Error> { self.failed = Some(cause); Err(Error::Failed(cause)) }

    /// Call between bounded RX polls even when no ACK arrives; no method waits for firmware or resends a command.
    pub fn poll(&mut self, now_us: u64) -> Result<(), Error> {
        if let Some(cause) = self.failed { return Err(Error::Failed(cause)); }
        if now_us < self.last { return self.fail(Failure::Clock); }
        self.last = now_us;
        if self.slots.iter().flatten().any(|slot| slot.outcome.is_none() && now_us >= slot.deadline) {
            return self.fail(Failure::Timeout);
        }
        Ok(())
    }

    pub fn submit(&mut self, transport: &mut impl Transport, command: Command, body: &[u8], acks: Acks, timeout_us: u32) -> Result<Ticket, Error> {
        let before = transport.now_us(); self.poll(before)?;
        if command.category > 3 || command.class > 63 || body.len() > MAX_BODY || timeout_us == 0 || timeout_us > MAX_TIMEOUT_US {
            return Err(Error::Request);
        }
        let deadline = before.checked_add(u64::from(timeout_us)).ok_or(Error::Request)?;
        if self.next == u64::MAX || !self.serial && self.next == 256 { return Err(Error::Exhausted); }
        if self.serial && self.slots.iter().any(Option::is_some) { return Err(Error::Full); }
        let index = self.slots.iter().position(Option::is_none).ok_or(Error::Full)?;
        let generation = self.next;
        let identity = Identity { category: command.category, class: command.class, function: command.function, sequence: self.next as u8 };
        let wants = Acks { received: self.serial || acks.received || identity.sequence % 4 == 0, done: acks.done };
        let mut packet = [0u8; fwcmd::MAX_PAYLOAD];
        let w0 = u32::from(identity.category) | u32::from(identity.class) << 2 | u32::from(identity.function) << 8 | u32::from(identity.sequence) << 24;
        let w1 = (body.len() + HEADER_BYTES) as u32 | u32::from(wants.received) << 14 | u32::from(wants.done) << 15;
        packet[..4].copy_from_slice(&w0.to_le_bytes()); packet[4..8].copy_from_slice(&w1.to_le_bytes());
        packet[HEADER_BYTES..HEADER_BYTES + body.len()].copy_from_slice(body);
        let publish_at = transport.now_us(); self.poll(publish_at)?;
        if publish_at >= deadline { return self.fail(Failure::Timeout); }
        let remaining = (deadline - publish_at) as u32;
        let sent = transport.submit(&packet[..HEADER_BYTES + body.len()], remaining);
        let after = transport.now_us();
        if after < publish_at { return self.fail(Failure::Clock); }
        self.last = after;
        if let Err(cause) = sent {
            if cause == fwcmd::Error::Full { self.poll(after)?; return Err(Error::Full); }
            return self.fail(Failure::Transport(cause));
        }
        self.next += 1;
        if after >= deadline { return self.fail(Failure::Timeout); }
        self.slots[index] = Some(Pending { identity, generation, deadline, wants, received: false, done: false, rejected: None,
            outcome: if !wants.received && !wants.done { Some(Outcome::Submitted) } else { None } });
        self.poll(after)?;
        Ok(Ticket { identity, generation })
    }

    /// ACKs are already length-checked by c2h::parse; no RX DMA index or unrelated event completes a command.
    pub fn acknowledge(&mut self, now_us: u64, ack: Ack) -> Result<Observation, Error> {
        self.poll(now_us)?;
        let Some(slot) = self.slots.iter_mut().flatten().find(|slot| ack.matches(slot.identity)) else { return Ok(Observation::Unrelated); };
        if slot.outcome.is_some() { return Ok(Observation::Duplicate); }
        match ack {
            Ack::Received(_) => {
                if !slot.wants.received { return Ok(Observation::Unexpected); }
                if slot.received { return Ok(Observation::Duplicate); }
                slot.received = true;
            }
            Ack::Done { status, .. } => {
                if !slot.wants.done { return Ok(Observation::Unexpected); }
                if slot.done { return Ok(Observation::Duplicate); }
                slot.done = true;
                if status != 0 { slot.rejected = Some(status); if !self.serial { slot.outcome = Some(Outcome::Rejected(status)); } }
            }
        }
        if slot.outcome.is_none() && (!slot.wants.received || slot.received) && (!slot.wants.done || slot.done) {
            slot.outcome = Some(if let Some(status) = slot.rejected { Outcome::Rejected(status) } else if slot.wants.done { Outcome::Done } else { Outcome::Received });
        }
        Ok(if slot.outcome.is_some() { Observation::Complete } else { Observation::Progress })
    }

    pub fn progress(&mut self, now_us: u64, ticket: Ticket) -> Result<Progress, Error> {
        self.poll(now_us)?;
        let slot = self.slots.iter().flatten().find(|slot| slot.identity == ticket.identity && slot.generation == ticket.generation).ok_or(Error::Unknown)?;
        Ok(match slot.outcome { Some(value) => Progress::Complete(value), None => Progress::Pending { received: slot.received, done: slot.done } })
    }

    /// Release only a terminal result slot; the hardware DMA retention and firmware ownership remain unchanged.
    pub fn take(&mut self, now_us: u64, ticket: Ticket) -> Result<Option<Outcome>, Error> {
        self.poll(now_us)?;
        let slot = self.slots.iter_mut().find(|slot| slot.as_ref().is_some_and(|value| value.identity == ticket.identity && value.generation == ticket.generation)).ok_or(Error::Unknown)?;
        let outcome = slot.unwrap().outcome;
        if outcome.is_some() { *slot = None; }
        Ok(outcome)
    }
}
