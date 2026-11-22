//! How a select changes an interface's isochronous streams (000-DRV-0081): which goes first, the request or the
//! endpoints' command, and what a TD that ends while its stream closes counts as. No system calls: tests/usb_close_host.rs.

// xHCI 6.4.5's completion codes, as xhci.rs has them.
const SUCCESS: u32 = 1; const SHORT_PACKET: u32 = 13; const RING_UNDERRUN: u32 = 14; const RING_OVERRUN: u32 = 15; const MISSED_SERVICE: u32 = 23;

/// What a select does, from whether the interface streams now and whether the setting asked for has a stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    /// No stream before or after: SET_INTERFACE alone.
    Request,
    /// The setting streams: its endpoints configured first (the old ones dropped in the same command), then SET_INTERFACE.
    EndpointsFirst,
    /// A close: SET_INTERFACE first, while the TDs on the rings still go out, then the endpoints dropped and their rings
    /// freed. Dropped first, the device's stream stayed open with nothing coming: the speakerphone counted frames lacking.
    RequestFirst,
}

pub fn order(streaming: bool, to_stream: bool) -> Order {
    if to_stream { Order::EndpointsFirst } else if streaming { Order::RequestFirst } else { Order::Request }
}

/// Whether a select reads the configuration descriptor for the setting's endpoints. Setting 0's are the interface's
/// own, kept from its set-up, unless one of them is isochronous: those are kept without a packet's extra transactions.
pub fn reads_descriptor(alternate: u8, known_isochronous: bool) -> bool { alternate != 0 || known_isochronous }

/// What a TD's end counts as on its stream's last line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Counted { Good, Missed, Dry, Error, InClose }

/// A TD that ended with completion `code`, its stream `closing` or not. A dry ring and a missed service count as they
/// always do; an error after the device was told to close is the device no longer answering, not a fault.
pub fn counted(code: u32, closing: bool) -> Counted {
    match code {
        SUCCESS | SHORT_PACKET => Counted::Good,
        RING_UNDERRUN | RING_OVERRUN => Counted::Dry,
        MISSED_SERVICE => Counted::Missed,
        _ if closing => Counted::InClose,
        _ => Counted::Error,
    }
}
