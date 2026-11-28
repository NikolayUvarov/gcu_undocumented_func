// Host cases of display's stage 16 against display.wit 1.3 (213-DRV-0092, 213-KRN-0272): screen.rs's offer and
// screen_gone go through libmind's generated client, a model of the IPC, the generated `decode` the compositor runs, its
// answer by the rules of its second_screen, the generated reply, and back to step 4's names for the answers.
#![allow(dead_code)]
#[path = "../common/abi.rs"] mod abi;
#[path = "../libmind/src/arch/mod.rs"] mod arch;
#[path = "../libmind/src/sys.rs"] mod sys;
#[path = "../libmind/src/display.rs"] mod marks;
#[path = "../display/src/timing.rs"] mod timing;
#[path = "../display/src/edid.rs"] mod edid;
#[path = "../display/src/dcn.rs"] mod dcn;
#[path = "../display/src/batches.rs"] mod batches;
#[path = "../display/src/atom.rs"] mod atom;
#[path = "../display/src/light.rs"] mod light;
#[path = "../display/src/picture.rs"] mod picture;
#[path = "../display/src/screen.rs"] mod screen;

mod idl {
    #[path = "../../libmind/src/idl/codec.rs"] pub mod codec;
    #[path = "../../libmind/src/idl/wire.rs"] pub mod wire;
    #[path = "../../libmind/src/idl/display.rs"] pub mod display;
}

mod dev {
    /// A slot's kind and length, as SYSCALL_CAP_INFO gives them: display's, or the compositor's receive slot.
    pub fn cap_info(slot: usize) -> (usize, usize, usize) { crate::ipc::slot(slot).map_or((crate::abi::CAP_KIND_NONE, 0, 0), |(k, l)| (k, 0, l)) }
}

mod mem {
    /// Client buffers: no buffer call is made here.
    pub struct Pages(Vec<u8>);
    impl Pages {
        pub fn new(bytes: usize) -> Option<Self> { Some(Self(vec![0; bytes])) }
        pub fn len(&self) -> usize { self.0.len() }
        pub fn as_slice(&self) -> &[u8] { &self.0 }
        pub fn as_mut_slice(&mut self) -> &mut [u8] { &mut self.0 }
        pub fn share(&self) -> crate::sys::Result<usize> { Err(crate::sys::Error::Rights) }
    }
    pub struct Mapping(Vec<u8>);
    impl Mapping {
        pub fn new(_slot: usize) -> crate::sys::Result<Self> { Err(crate::sys::Error::Rights) }
        pub fn len(&self) -> usize { self.0.len() }
        pub fn as_slice(&self) -> &[u8] { &self.0 }
        pub fn as_mut_slice(&mut self) -> &mut [u8] { &mut self.0 }
    }
}

mod ipc {
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct Message { pub data: [usize; 2], pub cap: Option<usize>, pub moved: bool }
    impl Message {
        pub fn new(a: usize, b: usize) -> Self { Self { data: [a, b], cap: None, moved: false } }
        pub fn with_cap(mut self, cap: usize, _rights: u8) -> Self { self.cap = Some(cap); self }
        pub fn with_cap_moved(mut self, cap: usize, _rights: u8) -> Self { self.cap = Some(cap); self.moved = true; self }
    }
    #[derive(Clone, Copy, Debug)]
    pub struct Received { pub data: [usize; 2], pub sender: u64, pub badge: u16, pub cap_received: bool, pub is_call: bool, pub irq: Option<usize> }
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Endpoint(pub usize);
    /// The compositor's receive slot, and the two tasks' PIDs.
    pub const RECEIVED: usize = 9;
    pub const DISPLAY: u64 = 38;
    pub const COMPOSITOR: u64 = 21;
    thread_local! {
        pub static SLOTS: RefCell<HashMap<usize, (usize, usize)>> = RefCell::new(HashMap::new());
        pub static SENT: RefCell<Vec<(usize, Message)>> = RefCell::new(Vec::new());
        pub static DROPPED: RefCell<Vec<usize>> = RefCell::new(Vec::new());
        pub static REPLY: Cell<Option<Message>> = Cell::new(None);
        /// The badge init gave display's client of the compositor; the call failing in the kernel instead.
        pub static BADGE: Cell<u16> = Cell::new(crate::marks::BADGE_SCREEN);
        /// The badge init gave display's second client, in slot 8 (213-KRN-0285).
        pub static THIRD_BADGE: Cell<u16> = Cell::new(crate::marks::BADGE_THIRD_SCREEN);
        pub static TRANSPORT: Cell<Option<crate::sys::Error>> = Cell::new(None);
    }
    pub fn slot(slot: usize) -> Option<(usize, usize)> { SLOTS.with(|s| s.borrow().get(&slot).copied()) }
    impl Endpoint {
        /// The kernel's part: the lent capability copied into the compositor's receive slot, the request delivered, the
        /// reply handed back.
        pub fn call(&self, message: &Message, _receive: usize) -> crate::sys::Result<Received> {
            SENT.with(|s| s.borrow_mut().push((self.0, *message)));
            if let Some(e) = TRANSPORT.with(|t| t.get()) { return Err(e); }
            if let Some(c) = message.cap { if let Some(k) = slot(c) { SLOTS.with(|s| s.borrow_mut().insert(RECEIVED, k)); } }
            let badge = if self.0 == crate::marks::SLOT_THIRD_SCREEN { THIRD_BADGE.with(|b| b.get()) } else { BADGE.with(|b| b.get()) };
            let request = Received { data: message.data, sender: DISPLAY, badge, cap_received: message.cap.is_some(), is_call: true, irq: None };
            crate::compositor(&request);
            let reply = REPLY.with(|r| r.take()).ok_or(crate::sys::Error::Peer)?;
            Ok(Received { data: reply.data, sender: COMPOSITOR, badge: 0, cap_received: false, is_call: false, irq: None })
        }
        pub fn call_timeout(&self, message: &Message, receive: usize, _ms: u32) -> crate::sys::Result<Received> { self.call(message, receive) }
    }
    pub fn reply(message: &Message) -> crate::sys::Result<()> { REPLY.with(|r| r.set(Some(*message))); Ok(()) }
    pub fn save_reply() -> crate::sys::Result<usize> { Ok(30) }
    pub fn reply_saved(_slot: usize, message: &Message) -> crate::sys::Result<()> { reply(message) }
    pub fn drop_cap(slot: usize) -> crate::sys::Result<()> { DROPPED.with(|d| d.borrow_mut().push(slot)); SLOTS.with(|s| s.borrow_mut().remove(&slot)); Ok(()) }
    pub fn revoke(_slot: usize) -> crate::sys::Result<usize> { Ok(1) }
}

use abi::{CAP_KIND_ENDPOINT, CAP_KIND_MEMORY, CAP_KIND_MMIO, SLOT_DISPLAY, SLOT_MEM};
use idl::display::{Error, PixelFormat, Request};
use light::{ScreenRefused, Screens};
use std::cell::{Cell, RefCell};

thread_local! {
    static DECODED: RefCell<Vec<Request>> = RefCell::new(Vec::new());
    static REJECTED: Cell<usize> = Cell::new(0);
    /// An answer the compositor gives instead of its own: a case of the interface, or raw reply words.
    static FORCED: Cell<Option<Error>> = Cell::new(None);
    static RAW: Cell<Option<[usize; 2]>> = Cell::new(None);
    /// The driver screen each call's badge named.
    static SCREENS: RefCell<Vec<Option<usize>>> = RefCell::new(Vec::new());
}

/// The compositor as compositor/src/main.rs serves display.wit 1.3 (213-KRN-0272) and 1.5 (213-KRN-0285): `decode`, then
/// its driver screens' rules: a client whose badge names no screen (`screen_of`) is denied; an offer's shape must fit
/// the lent range (`bad-screen`). Each call is kept with the screen its badge names.
fn compositor(request: &ipc::Received) {
    use idl::display::{decode, reply_offer_screen, reply_screen_gone};
    let (decoded, call) = match decode(request, ipc::RECEIVED) {
        Ok(x) => x,
        Err(reason) => { REJECTED.with(|r| r.set(r.get() + 1)); let _ = idl::wire::reject(reason); return; }
    };
    DECODED.with(|d| d.borrow_mut().push(decoded));
    if let Some(words) = RAW.with(|r| r.get()) { let _ = idl::wire::finish(call, words); return; }
    let screen = marks::screen_of(request.badge).is_some();
    SCREENS.with(|s| s.borrow_mut().push(marks::screen_of(request.badge)));
    let forced = FORCED.with(|f| f.get());
    match decoded {
        Request::OfferScreen { width, height, pitch, pixels, .. } => {
            let (w, h, p, lent) = (width as usize, height as usize, pitch as usize, dev::cap_info(pixels).2);
            let answer = if !screen { Err(Error::Denied) } else if let Some(e) = forced { Err(e) }
                else if w == 0 || h == 0 || p % 4 != 0 || p < w * 4 || lent < p * h { Err(Error::BadScreen) } else { Ok(()) };
            let _ = ipc::drop_cap(pixels);
            let _ = reply_offer_screen(call, answer);
        }
        Request::ScreenGone => { let _ = reply_screen_gone(call, if !screen { Err(Error::Denied) } else { Ok(()) }); }
        _ => unreachable!("display sends only offer_screen and screen_gone"),
    }
}

/// display's slots as init grants them (213-KRN-0271, 0272): its client of the compositor, and slot 5's 20 MiB of BAR 0.
fn granted() {
    ipc::SLOTS.with(|s| { let mut s = s.borrow_mut(); s.clear(); s.insert(SLOT_DISPLAY, (CAP_KIND_ENDPOINT, 0)); s.insert(SLOT_MEM, (CAP_KIND_MMIO, 0x0240_0000 - 0x0100_0000)); });
    ipc::SENT.with(|s| s.borrow_mut().clear());
    ipc::DROPPED.with(|d| d.borrow_mut().clear());
    ipc::BADGE.with(|b| b.set(marks::BADGE_SCREEN));
    ipc::THIRD_BADGE.with(|b| b.set(marks::BADGE_THIRD_SCREEN));
    SCREENS.with(|s| s.borrow_mut().clear());
    ipc::TRANSPORT.with(|t| t.set(None));
    DECODED.with(|d| d.borrow_mut().clear());
    REJECTED.with(|r| r.set(0));
    FORCED.with(|f| f.set(None));
    RAW.with(|r| r.set(None));
}
fn sent() -> Vec<(usize, ipc::Message)> { ipc::SENT.with(|s| s.borrow().clone()) }
fn decoded() -> Vec<Request> { DECODED.with(|d| d.borrow().clone()) }
/// What stage 16 offers (light.rs's Step::Offer): the picture's shape, a row 4 bytes a pixel apart.
fn offer() -> Result<(), ScreenRefused> {
    screen::Compositor { pixels: SLOT_MEM }.offer(picture::WIDTH as u16, picture::HEIGHT as u16, (picture::PITCH * 4) as u32)
}

#[test]
fn stage_16s_offer_reaches_the_compositor_as_display_wit_1_3_decodes_it_and_is_taken() {
    granted();
    assert_eq!(offer(), Ok(()));
    // One call to SLOT_DISPLAY, slot 5 lent (copied, not moved: display keeps its range); method 5 of major 1.
    let s = sent();
    assert_eq!(s.len(), 1);
    assert_eq!((s[0].0, s[0].1.cap, s[0].1.moved), (SLOT_DISPLAY, Some(SLOT_MEM), false));
    assert_eq!(s[0].1.data[0] & 0xFFFF, 5 | 1 << 8);
    // The compositor's decode: 1920x1080, pitch 7680 bytes, bgr, the pixels its receive slot, an MMIO capability.
    assert_eq!(decoded(), [Request::OfferScreen { width: 1920, height: 1080, pitch: 7680, format: PixelFormat::Bgr, pixels: ipc::RECEIVED }]);
    assert_eq!(REJECTED.with(|r| r.get()), 0);
    // The shape fits slot 5 with room to spare: 8,294,400 of 20,971,520 bytes.
    assert!(7680 * 1080 <= 0x0140_0000);
    // The compositor let its copy of the capability go; display's own stays.
    assert!(ipc::DROPPED.with(|d| d.borrow().contains(&ipc::RECEIVED)));
    assert_eq!(ipc::slot(SLOT_MEM), Some((CAP_KIND_MMIO, 0x0140_0000)));
    // screen_gone: method 6, no capability, taken.
    assert_eq!(screen::Compositor { pixels: SLOT_MEM }.gone(), Ok(()));
    let s = sent();
    assert_eq!((s[1].0, s[1].1.cap, s[1].1.data[0] & 0xFFFF), (SLOT_DISPLAY, None, 6 | 1 << 8));
    assert_eq!(decoded()[1], Request::ScreenGone);
}

#[test]
fn the_compositors_refusals_come_back_as_step_4_names_them() {
    // Not badged BADGE_SCREEN: denied, both calls.
    granted();
    ipc::BADGE.with(|b| b.set(0));
    assert_eq!(offer(), Err(ScreenRefused::Denied));
    assert_eq!(screen::Compositor { pixels: SLOT_MEM }.gone(), Err(ScreenRefused::Denied));
    // A range that does not hold the shape: bad-screen.
    granted();
    ipc::SLOTS.with(|s| s.borrow_mut().insert(SLOT_MEM, (CAP_KIND_MMIO, 7680 * 1080 - 4)));
    assert_eq!(offer(), Err(ScreenRefused::BadScreen));
    granted();
    assert_eq!(screen::Compositor { pixels: SLOT_MEM }.offer(1920, 1080, 1920), Err(ScreenRefused::BadScreen), "a pitch in pixels, not bytes");
    // The compositor's other answers.
    for (e, want) in [(Error::NoMemory, ScreenRefused::NoMemory), (Error::NoScreen, ScreenRefused::NoScreen)] {
        granted();
        FORCED.with(|f| f.set(Some(e)));
        assert_eq!(offer(), Err(want));
    }
}

#[test]
fn a_call_that_fails_or_an_answer_outside_the_interface_is_a_failed_call_and_no_client_makes_no_call() {
    // The kernel refuses the call (the compositor gone): Call.
    granted();
    ipc::TRANSPORT.with(|t| t.set(Some(sys::Error::Peer)));
    assert_eq!(offer(), Err(ScreenRefused::Call));
    // An error case the interface does not define (7), or a status other than ok: Call.
    granted();
    RAW.with(|r| r.set(Some([idl::wire::STATUS_ERROR, 7])));
    assert_eq!(offer(), Err(ScreenRefused::Call));
    granted();
    RAW.with(|r| r.set(Some([idl::wire::STATUS_OK | 1 << 16, 0])));
    assert_eq!(offer(), Err(ScreenRefused::Call), "unused reply bits set");
    // Slot 5 as memory instead of MMIO: the compositor's schema check refuses it before any answer.
    granted();
    ipc::SLOTS.with(|s| s.borrow_mut().insert(SLOT_MEM, (CAP_KIND_MEMORY, 0x0140_0000)));
    assert_eq!(offer(), Err(ScreenRefused::Call));
    assert_eq!((REJECTED.with(|r| r.get()), decoded().len()), (1, 0));
    // No client of the compositor granted (an init without 213-KRN-0272): Unavailable, and nothing is sent.
    granted();
    ipc::SLOTS.with(|s| s.borrow_mut().remove(&SLOT_DISPLAY));
    assert_eq!(offer(), Err(ScreenRefused::Unavailable));
    assert_eq!(screen::Compositor { pixels: SLOT_MEM }.gone(), Err(ScreenRefused::Unavailable));
    assert!(sent().is_empty());
}

#[test]
fn the_third_screen_goes_through_slot_8_with_slot_7_lent_and_the_second_stays_on_slot_15() {
    use marks::{SLOT_THIRD_PIXELS, SLOT_THIRD_SCREEN};
    // init's grants with 213-KRN-0285: slot 7's 20 MiB of BAR 0 and display's second client of the compositor in slot 8.
    granted();
    ipc::SLOTS.with(|s| { let mut s = s.borrow_mut(); s.insert(SLOT_THIRD_SCREEN, (CAP_KIND_ENDPOINT, 0)); s.insert(SLOT_THIRD_PIXELS, (CAP_KIND_MMIO, 0x0140_0000)); });
    let third = || screen::Client { pixels: SLOT_THIRD_PIXELS, client: SLOT_THIRD_SCREEN };
    assert_eq!(third().offer(1366, 768, 5632), Ok(()));
    assert_eq!(offer(), Ok(()));
    assert_eq!(third().gone(), Ok(()));
    // Each through its own client, its own pixels lent; the compositor's rule names the third screen, then the second.
    let s = sent();
    assert_eq!(s.iter().map(|(at, m)| (*at, m.cap, m.data[0] & 0xFFFF)).collect::<Vec<_>>(),
               [(SLOT_THIRD_SCREEN, Some(SLOT_THIRD_PIXELS), 5 | 1 << 8), (SLOT_DISPLAY, Some(SLOT_MEM), 5 | 1 << 8), (SLOT_THIRD_SCREEN, None, 6 | 1 << 8)]);
    assert_eq!(SCREENS.with(|s| s.borrow().clone()), [Some(1), Some(0), Some(1)]);
    assert_eq!(decoded()[0], Request::OfferScreen { width: 1366, height: 768, pitch: 5632, format: PixelFormat::Bgr, pixels: ipc::RECEIVED });
    // Slot 8 not granted (an init without 0285): said as the third's client, nothing sent.
    granted();
    assert_eq!(third().offer(1366, 768, 5632), Err(ScreenRefused::ThirdUnavailable));
    assert_eq!(third().gone(), Err(ScreenRefused::ThirdUnavailable));
    assert!(sent().is_empty());
    assert_eq!(ScreenRefused::ThirdUnavailable.to_string(), "NO SECOND CLIENT OF IT (SLOT 8) WAS GRANTED");
    // A badge naming no screen on slot 8: denied.
    granted();
    ipc::SLOTS.with(|s| { let mut s = s.borrow_mut(); s.insert(SLOT_THIRD_SCREEN, (CAP_KIND_ENDPOINT, 0)); s.insert(SLOT_THIRD_PIXELS, (CAP_KIND_MMIO, 0x0140_0000)); });
    ipc::THIRD_BADGE.with(|b| b.set(0));
    assert_eq!(third().offer(1366, 768, 5632), Err(ScreenRefused::Denied));
}
