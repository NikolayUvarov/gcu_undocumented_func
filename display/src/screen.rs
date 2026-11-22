// Stage 16's client of the compositor (213-DRV-0092, 213-KRN-0272): slot 5's surface offered as the second screen through
// display.wit 1.3, and let go. Written against the crate root's `abi`, `dev`, `idl`, `ipc` and `sys`: libmind's in
// display, a model of them in the host cases.
use crate::abi::{CAP_KIND_ENDPOINT, SLOT_DISPLAY};
use crate::idl::display::{self as wit, Error, PixelFormat};
use crate::ipc::Endpoint;
use crate::light::{ScreenRefused, Screens};

/// The compositor through display's client of it in SLOT_DISPLAY; the pixels are the MMIO range in slot `pixels`, lent.
pub struct Compositor { pub pixels: usize }

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

fn granted() -> bool { crate::dev::cap_info(SLOT_DISPLAY).0 == CAP_KIND_ENDPOINT }

impl Screens for Compositor {
    // Blue first in memory: picture.rs's pixels are 0x00RRGGBB words, display.wit's `bgr`.
    fn offer(&mut self, width: u16, height: u16, pitch: u32) -> Result<(), ScreenRefused> {
        if !granted() { return Err(ScreenRefused::Unavailable); }
        answer(wit::offer_screen(Endpoint(SLOT_DISPLAY), width, height, pitch, PixelFormat::Bgr, self.pixels))
    }
    fn gone(&mut self) -> Result<(), ScreenRefused> {
        if !granted() { return Err(ScreenRefused::Unavailable); }
        answer(wit::screen_gone(Endpoint(SLOT_DISPLAY)))
    }
}
