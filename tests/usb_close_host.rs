//! usb_host's close of an interface's isochronous streams on the host (000-DRV-0081): the order of the request and the
//! endpoints' command, when a select reads the descriptor, and what a TD that ends in a close counts as.
#[path = "../usb_host/src/close.rs"]
mod close;
use close::{counted, order, reads_descriptor, Counted, Order};

#[test]
fn a_close_tells_the_device_before_its_endpoints_are_dropped() {
    // Streaming, to a setting without a stream: SET_INTERFACE first.
    assert_eq!(order(true, false), Order::RequestFirst);
    // To a setting with a stream, from none or from another: its endpoints first, so they are there when it sends.
    assert_eq!(order(false, true), Order::EndpointsFirst);
    assert_eq!(order(true, true), Order::EndpointsFirst);
    // Neither: the request alone, no command.
    assert_eq!(order(false, false), Order::Request);
}

#[test]
fn setting_0_needs_no_descriptor_unless_it_has_an_isochronous_endpoint() {
    assert!(!reads_descriptor(0, false));
    // Setting 0 with an isochronous endpoint is read again: its packet's extra transactions were not kept.
    assert!(reads_descriptor(0, true));
    assert!(reads_descriptor(1, false));
    assert!(reads_descriptor(2, true));
}

#[test]
fn an_error_in_a_close_is_not_the_streams_but_a_dry_ring_or_a_miss_still_is() {
    // Success and a short packet are good either way.
    for closing in [false, true] {
        assert_eq!(counted(1, closing), Counted::Good);
        assert_eq!(counted(13, closing), Counted::Good);
        // A ring that ran dry (underrun, overrun) and a missed service count as always: the device saw them.
        assert_eq!(counted(14, closing), Counted::Dry);
        assert_eq!(counted(15, closing), Counted::Dry);
        assert_eq!(counted(23, closing), Counted::Missed);
    }
    // A transaction error, a stall (QEMU's usb-audio after SET_INTERFACE 0), a babble: errors while it runs, not in its close.
    for code in [4, 6, 3, 36] {
        assert_eq!(counted(code, false), Counted::Error);
        assert_eq!(counted(code, true), Counted::InClose);
    }
}
