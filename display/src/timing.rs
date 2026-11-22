//! A display mode's timing and its arithmetic (213-DRV-0086): totals, refresh rate, the TMDS clock it needs, and
//! the values DCN 2.1's timing generator (OTG) takes for it. No hardware access; `tests/display_host.rs` checks it.
//!
//! The OTG counts each line from the start of its sync pulse: sync from 0 to the sync width, blank end after the back
//! porch, blank start after the active pixels, total minus one at the end of the front porch. That is the convention
//! Linux v7.0 programs (`optc1_program_timing`, `drivers/gpu/drm/amd/display/dc/optc/dcn10/dcn10_optc.c`) and that
//! Display Core's dump showed for both of the Ryzen's monitors (hwdocs/cezanne-display.md).

/// The largest value of the OTG's 15-bit counters (`OTG_H_TOTAL`, `OTG_V_TOTAL` and the blank and sync fields).
pub const OTG_COUNTER_MAX: u32 = 0x7FFF;
/// The TMDS character rate above which HDMI 2.0 needs scrambling: first light stays at or below it.
pub const TMDS_NO_SCRAMBLING_KHZ: u32 = 340_000;
/// A single TMDS link's limit for DVI signalling.
pub const DVI_SINGLE_LINK_KHZ: u32 = 165_000;

/// One mode: the pixel clock and each part of a line and of a frame, as an EDID detailed timing gives them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timing {
    pub pixel_khz: u32,
    pub h_active: u16,
    pub h_front: u16,
    pub h_sync: u16,
    pub h_back: u16,
    pub v_active: u16,
    pub v_front: u16,
    pub v_sync: u16,
    pub v_back: u16,
    pub h_sync_positive: bool,
    pub v_sync_positive: bool,
    pub interlaced: bool,
}

/// Why a timing cannot be set as it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimingError {
    /// A zero clock, active area or sync width.
    Empty,
    /// Interlaced modes are not driven.
    Interlaced,
    /// A total beyond the OTG's 15-bit counters.
    TooLarge,
    /// No front porch: the OTG needs at least one pixel and one line of it.
    NoFrontPorch,
}

/// The CTA-861 1920x1080 at 60 Hz mode (VIC 16): 148.5 MHz, both syncs positive. First light's mode where the
/// monitor offers it, before its own preferred one.
pub const CEA_1080P60: Timing = Timing {
    pixel_khz: 148_500, h_active: 1920, h_front: 88, h_sync: 44, h_back: 148,
    v_active: 1080, v_front: 4, v_sync: 5, v_back: 36, h_sync_positive: true, v_sync_positive: true, interlaced: false,
};

/// The OTG's register values for one timing, each a field of its register, already minus one where the register
/// counts from zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Otg {
    /// `OTG_H_TOTAL`: pixels a line, minus one.
    pub h_total: u32,
    /// `OTG_H_BLANK_START_END`: where blank begins (after the active pixels) and ends (after the back porch).
    pub h_blank_start: u32,
    pub h_blank_end: u32,
    /// `OTG_H_SYNC_A`: the sync pulse's end; it starts at 0.
    pub h_sync_end: u32,
    /// `OTG_V_TOTAL` (and `_MIN`, `_MAX`): lines a frame, minus one.
    pub v_total: u32,
    pub v_blank_start: u32,
    pub v_blank_end: u32,
    pub v_sync_end: u32,
    /// `OTG_H_SYNC_A_CNTL.OTG_H_SYNC_A_POL` and `OTG_V_SYNC_A_CNTL.OTG_V_SYNC_A_POL`: 1 for a negative pulse.
    pub h_sync_negative: bool,
    pub v_sync_negative: bool,
    /// `VTG0_CONTROL.VTG0_VCOUNT_INIT`: the line the count starts from, the start of vertical blank.
    pub vcount_init: u32,
}

impl Timing {
    pub fn h_blank(&self) -> u32 { self.h_front as u32 + self.h_sync as u32 + self.h_back as u32 }
    pub fn v_blank(&self) -> u32 { self.v_front as u32 + self.v_sync as u32 + self.v_back as u32 }
    pub fn h_total(&self) -> u32 { self.h_active as u32 + self.h_blank() }
    pub fn v_total(&self) -> u32 { self.v_active as u32 + self.v_blank() }

    /// The refresh rate in thousandths of a hertz, rounded to the nearest.
    pub fn refresh_millihertz(&self) -> u32 {
        let pixels = self.h_total() as u64 * self.v_total() as u64;
        if pixels == 0 { return 0; }
        ((self.pixel_khz as u64 * 1_000_000 + pixels / 2) / pixels) as u32
    }

    /// The TMDS character rate for `bpc` bits a colour: the pixel clock for 8 bits, 5/4 of it for 10, 3/2 for 12 and
    /// twice it for 16 (HDMI's deep colour ratios).
    pub fn tmds_khz(&self, bpc: u8) -> Option<u32> {
        let (num, den) = match bpc { 8 => (1, 1), 10 => (5, 4), 12 => (3, 2), 16 => (2, 1), _ => return None };
        Some((self.pixel_khz as u64 * num / den) as u32)
    }

    /// Whether the OTG can run this timing as it is.
    pub fn check(&self) -> Result<(), TimingError> {
        if self.pixel_khz == 0 || self.h_active == 0 || self.v_active == 0 || self.h_sync == 0 || self.v_sync == 0 {
            return Err(TimingError::Empty);
        }
        if self.interlaced { return Err(TimingError::Interlaced); }
        if self.h_front == 0 || self.v_front == 0 { return Err(TimingError::NoFrontPorch); }
        if self.h_total() - 1 > OTG_COUNTER_MAX || self.v_total() - 1 > OTG_COUNTER_MAX { return Err(TimingError::TooLarge); }
        Ok(())
    }

    /// The OTG's values for this timing.
    pub fn otg(&self) -> Result<Otg, TimingError> {
        self.check()?;
        let (h_total, v_total) = (self.h_total(), self.v_total());
        let h_blank_start = h_total - self.h_front as u32;
        let v_blank_start = v_total - self.v_front as u32;
        Ok(Otg {
            h_total: h_total - 1,
            h_blank_start,
            h_blank_end: h_blank_start - self.h_active as u32,
            h_sync_end: self.h_sync as u32,
            v_total: v_total - 1,
            v_blank_start,
            v_blank_end: v_blank_start - self.v_active as u32,
            v_sync_end: self.v_sync as u32,
            h_sync_negative: !self.h_sync_positive,
            v_sync_negative: !self.v_sync_positive,
            vcount_init: v_blank_start,
        })
    }
}

impl Otg {
    /// The timing these register values describe, for a pixel clock known from elsewhere: the inverse of
    /// `Timing::otg`, for logging what the firmware left. None for values no timing gives.
    pub fn timing(&self, pixel_khz: u32) -> Option<Timing> {
        let (h_total, v_total) = (self.h_total + 1, self.v_total + 1);
        if self.h_sync_end == 0 || self.h_blank_end <= self.h_sync_end || self.h_blank_start <= self.h_blank_end || self.h_blank_start >= h_total {
            return None;
        }
        if self.v_sync_end == 0 || self.v_blank_end <= self.v_sync_end || self.v_blank_start <= self.v_blank_end || self.v_blank_start >= v_total {
            return None;
        }
        let narrow = |v: u32| u16::try_from(v).ok();
        Some(Timing {
            pixel_khz,
            h_active: narrow(self.h_blank_start - self.h_blank_end)?,
            h_front: narrow(h_total - self.h_blank_start)?,
            h_sync: narrow(self.h_sync_end)?,
            h_back: narrow(self.h_blank_end - self.h_sync_end)?,
            v_active: narrow(self.v_blank_start - self.v_blank_end)?,
            v_front: narrow(v_total - self.v_blank_start)?,
            v_sync: narrow(self.v_sync_end)?,
            v_back: narrow(self.v_blank_end - self.v_sync_end)?,
            h_sync_positive: !self.h_sync_negative,
            v_sync_positive: !self.v_sync_negative,
            interlaced: false,
        })
    }
}

/// A CTA-861 video format by its VIC, for the few a first light may choose (CTA-861-G, table 1 and its timings): the
/// 60 Hz and 50 Hz progressive ones a monitor of this machine lists. None for any other.
pub fn cta(vic: u8) -> Option<Timing> {
    let t = |pixel_khz, ha, hf, hs, hb, va, vf, vs, vb, positive| Timing {
        pixel_khz, h_active: ha, h_front: hf, h_sync: hs, h_back: hb, v_active: va, v_front: vf, v_sync: vs, v_back: vb,
        h_sync_positive: positive, v_sync_positive: positive, interlaced: false,
    };
    Some(match vic {
        1 => t(25_175, 640, 16, 96, 48, 480, 10, 2, 33, false),
        4 => t(74_250, 1280, 110, 40, 220, 720, 5, 5, 20, true),
        16 => t(148_500, 1920, 88, 44, 148, 1080, 4, 5, 36, true),
        19 => t(74_250, 1280, 440, 40, 220, 720, 5, 5, 20, true),
        31 => t(148_500, 1920, 528, 44, 148, 1080, 4, 5, 36, true),
        _ => return None,
    })
}

/// The CTA VIC whose timing `t` is, of those `cta` knows; None for any other timing.
pub fn vic_of(t: &Timing) -> Option<u8> { [1u8, 4, 16, 19, 31].into_iter().find(|&v| cta(v).as_ref() == Some(t)) }

/// First light's mode for a sink that lists `vics`: CTA 1080p60 (VIC 16) where it is listed, else 720p60 (VIC 4), else
/// 640x480 (VIC 1, which every CTA sink must take); all within one DVI link.
pub fn first_light_mode(vics: &[u8]) -> Timing {
    [16, 4].into_iter().find(|v| vics.contains(v)).and_then(cta).unwrap_or(cta(1).unwrap_or(CEA_1080P60))
}
