// Stage 16's client of the compositor (213-DRV-0092, 213-KRN-0272): slot 5's surface offered as the second screen through
// display.wit 1.3, and let go; slot 7's as the third through display's second client in slot 8 (display.wit 1.5,
// 213-KRN-0285, 213-DRV-0113). Written against the crate root's `abi`, `dev`, `idl`, `ipc` and `sys`: libmind's in
// display, a model of them in the host cases.
use crate::abi::{CAP_KIND_ENDPOINT, SLOT_DISPLAY};
use crate::idl::display::{self as wit, Error, PixelFormat};
use crate::ipc::Endpoint;
use crate::light::{ScreenRefused, Screens};

/// The compositor through display's client of it in SLOT_DISPLAY; the pixels are the MMIO range in slot `pixels`, lent.
pub struct Compositor { pub pixels: usize }
/// The compositor through the client in slot `client`, which init badged for one screen; the pixels slot `pixels`'s.
pub struct Client { pub pixels: usize, pub client: usize }

/// The compositor's answer as step 4 names it; a call that failed, or an answer the interface does not define, is `Call`.
pub fn answer(a: crate::sys::Result<Result<(), Error>>) -> Result<(), ScreenRefused> {
    match a {
        Ok(Ok(())) => Ok(()),
        Ok(Err(Error::NoScreen)) => Err(ScreenRefused::NoScreen),
        Ok(Err(Error::NoMemory)) => Err(ScreenRefused::NoMemory),
        Ok(Err(Error::Denied)) => Err(ScreenRefused::Denied),
        Ok(Err(Error::BadScreen)) => Err(ScreenRefused::BadScreen),
        Err(_) => Err(ScreenRefused::Call),
    }
}

/// Whether init granted a client of the compositor in `slot`.
pub fn granted(slot: usize) -> bool { crate::dev::cap_info(slot).0 == CAP_KIND_ENDPOINT }

impl Screens for Compositor {
    fn offer(&mut self, width: u16, height: u16, pitch: u32) -> Result<(), ScreenRefused> { Client { pixels: self.pixels, client: SLOT_DISPLAY }.offer(width, height, pitch) }
    fn gone(&mut self) -> Result<(), ScreenRefused> { Client { pixels: self.pixels, client: SLOT_DISPLAY }.gone() }
}

impl Client {
    // Not granted: said as the second's client or the third's.
    fn missing(&self) -> ScreenRefused { if self.client == SLOT_DISPLAY { ScreenRefused::Unavailable } else { ScreenRefused::ThirdUnavailable } }
}

impl Screens for Client {
    // Blue first in memory: picture.rs's pixels are 0x00RRGGBB words, display.wit's `bgr`.
    fn offer(&mut self, width: u16, height: u16, pitch: u32) -> Result<(), ScreenRefused> {
        if !granted(self.client) { return Err(self.missing()); }
        answer(wit::offer_screen(Endpoint(self.client), width, height, pitch, PixelFormat::Bgr, self.pixels))
    }
    fn gone(&mut self) -> Result<(), ScreenRefused> {
        if !granted(self.client) { return Err(self.missing()); }
        answer(wit::screen_gone(Endpoint(self.client)))
    }
}
