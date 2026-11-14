// What a claim after a lost transfer found, and whether that is the device the driver had or one that came back
// (000-DRV-0058). usb_host's handle names one enumeration of a device (its generation, device and interface): the
// same handle from the same usb_host instance is the device that never left, which only missed a transfer
// (211-DRV-0053). A device enumerated again, another one, or any after usb_host started again (its numbering starts
// over) may hold another medium, as may one whose medium the device says changed: the client must check it first.
// One of another capacity is not used at all, as before, while vfs_server checks a medium only after it saw the drive
// not answer (000-KRN-0079), not after a return (000-KRN-0085's client side).

/// What usb_host answered one claim: an interface (its handle, and its bulk pipes when it is SCSI over Bulk-Only), none
/// now, or a new instance without the client's buffer (the next claim attaches it).
pub enum Claim<T> { Interface(u32, Option<T>), None, NewHost }

/// The driver's side of usb_host while it looks for the device again.
pub trait Port<T> {
    fn claim(&mut self) -> Claim<T>;
    fn release(&mut self, handle: u32);
    /// The sector count when the interface answers with a medium (TEST UNIT READY, READ CAPACITY).
    fn ready(&mut self, interface: &mut T) -> Option<u64>;
    /// Between rounds: usb_host may still be setting a device up (211-DRV-0021).
    fn pause(&mut self);
}

/// What the claims gave: the same interface of the same enumeration, another one that answers with a medium of so many
/// sectors, or none.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Found { Same, Other(u64), Nothing }

/// Interfaces held at once while the others are looked through: one given back would be the next claim's again.
pub const HELD: usize = 8;

/// Claims every interface usb_host has free, in `tries` rounds `pause` apart, after the old claim was given back. `old`:
/// the handle that claim had, None when usb_host may have started again since. Returns what was found and whether a
/// new usb_host instance answered. Every interface claimed here is given back, except the same one, which stays the
/// driver's; another one is given back too, for the fresh driver instance to claim.
pub fn reclaim<T>(old: Option<u32>, tries: usize, port: &mut impl Port<T>) -> (Found, bool) {
    // `checked`: asked for a medium already, or not one this driver can drive.
    let mut held: [Option<(u32, Option<T>, bool)>; HELD] = [const { None }; HELD];
    let (mut found, mut new_host) = (Found::Nothing, false);
    'rounds: for round in 0..tries {
        // Every free interface first: the same one may come after another device's.
        loop {
            let Some(free) = held.iter().position(Option::is_none) else { break };
            match port.claim() {
                // A new instance numbers its devices from the start: a handle of the old one means nothing there.
                Claim::Interface(handle, _) if !new_host && old == Some(handle) => { found = Found::Same; break 'rounds; }
                Claim::Interface(handle, bot) => { let checked = bot.is_none(); held[free] = Some((handle, bot, checked)); }
                Claim::NewHost => { new_host = true; break; }
                Claim::None => break,
            }
        }
        for (_, bot, checked) in held.iter_mut().flatten().filter(|(_, _, checked)| !*checked) {
            *checked = true;
            if let Some(sectors) = bot.as_mut().and_then(|bot| port.ready(bot)) { found = Found::Other(sectors); break 'rounds; }
        }
        // Every place taken by interfaces that are not it: no free one can be claimed, so no later round finds more.
        if held.iter().all(Option::is_some) { break; }
        if round + 1 < tries { port.pause(); }
    }
    for (handle, _, _) in held.into_iter().flatten() { port.release(handle); }
    (found, new_host)
}

/// Why the device the driver finds is not taken as the one it had.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Why { Enumerated, NewHost, MediumChanged }

impl Why {
    pub fn text(self) -> &'static str {
        match self {
            Why::Enumerated => "A DEVICE CAME BACK, ENUMERATED AGAIN",
            Why::NewHost => "A DEVICE CAME BACK THROUGH USB_HOST STARTED AGAIN",
            Why::MediumChanged => "THE DEVICE CAME BACK SAYING ITS MEDIUM MAY HAVE CHANGED",
        }
    }
}

/// What a loss was: the device back (it only missed a transfer), one that came back (the command is not run on it),
/// a medium of another capacity (not used), or gone.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Verdict { Back, Returned(Why), Unused(u64), Gone }

/// The same interface's answer after the claim: its sector count and whether it said its medium may have changed.
#[derive(Clone, Copy)]
pub struct Answer { pub sectors: u64, pub changed: bool }

/// The verdict on what `reclaim` found, for a driver whose medium had `sectors` sectors; `same`: the same interface's
/// answer, None when it gave none (it stays claimed and is asked again at the next command).
pub fn verdict(found: Found, new_host: bool, sectors: u64, same: Option<Answer>) -> Verdict {
    match (found, same) {
        (Found::Nothing, _) => Verdict::Gone,
        (Found::Other(other), _) if other != sectors => Verdict::Unused(other),
        (Found::Other(_), _) => Verdict::Returned(if new_host { Why::NewHost } else { Why::Enumerated }),
        (Found::Same, None) => Verdict::Gone,
        (Found::Same, Some(answer)) if answer.sectors != sectors => Verdict::Unused(answer.sectors),
        (Found::Same, Some(answer)) if answer.changed => Verdict::Returned(Why::MediumChanged),
        (Found::Same, Some(_)) => Verdict::Back,
    }
}
