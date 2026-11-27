//! Step 1 of 213-DRV-0086: the firmware's display state read in ten batches, the registers Display Core itself reads
//! (hwdocs/cezanne-display.md, "a read proposed as safe"); in batch 8 those Linux's dmub code reads to tell whether the
//! display microcontroller (DMCUB) runs; in batches 9 and 10 the firmware's pipe as step 4's reference (213-DRV-0092):
//! its HUBP's request, DLG and TTU registers, its DPP, the MPC tree, DCHUBBUB's watermarks and DCCG's DPP clocks; in batch
//! 11 HDMI-A-1's DDC line (213-DRV-0095). In the survey's order each batch is named in the log before its first read and
//! paused after, each value logged as it is read, so that a hang names its register; in the production order
//! (213-DRV-0098) only the registers steps 2 to 4 take are read (`USED`), in the same order, with no pause and one line.
//! Batch 4 reads only the HUBPs batch 3 shows powered. Batch 12 (213-DRV-0112, `read_second`) reads HDMI-A-2's blocks
//! after the eleven, whole in either order, its values summed up in lines both orders log. `Bus` has no write: nothing
//! here can change the device. `tests/display_batches_host.rs` checks it.

use crate::dcn::{field::*, Field, Reg, R};
use crate::timing::Otg;
use core::fmt;

pub const BATCHES: u8 = 11;
/// The batches steps 3 and 4 need read: batch 11 (the DDC line, 213-DRV-0095) is for the EDID's read, not for them.
pub const STEP3_BATCHES: u8 = 10;
/// Batch 12 (213-DRV-0112): HDMI-A-2's blocks, read after the eleven by `read_second`.
pub const SECOND: u8 = 12;
/// Batches 13 and 14 (213-DRV-0113): the pipe a second run would take, and its plane, read by `read_pipe`.
pub const PIPE: u8 = 13;
pub const PLANE: u8 = 14;
/// The most registers one batch reads (batch 2: 19 for each of the four timing generators).
pub const MAX_REGISTERS: usize = 80;
/// After a batch's name is logged and before its first read: long enough for the line to be kept.
pub const PAUSE_BEFORE_READ_MS: u32 = 1000;
/// After a batch's values, before the next batch's name.
pub const PAUSE_BETWEEN_BATCHES_MS: u32 = 3000;

/// The device's registers, read only.
pub trait Bus {
    /// The register's value; None when the grant does not map it.
    fn read(&mut self, reg: Reg) -> Option<u32>;
}

/// Where the lines go, and the pauses. The survey's order logs every line and makes every pause; the production order
/// (213-DRV-0098, `Held`) makes no pause and holds a part's detail, logged only if the part fails. Neither touches the
/// device: the reads and writes are the same in both orders.
pub trait Out {
    /// A line of detail: a read, a write, a plan, a value.
    fn line(&mut self, args: fmt::Arguments);
    fn pause(&mut self, ms: u32);
    /// A line both orders log: a part's result and its time, a refusal, a failure.
    fn note(&mut self, args: fmt::Arguments) { self.line(args) }
    /// A part begins (a batch, step 2, a stage): the detail held of the one before is let go.
    fn part(&mut self) {}
    /// The part under way failed: the detail held of it is logged, and every line after it until the next part.
    fn failed(&mut self) {}
}

/// Steps 1 to 4's order (213-DRV-0098).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    /// For bring-up: every batch named and paused, every value, write and plan logged, a pause around every stage.
    Survey,
    /// For a machine in use: step 1 reads only what steps 2 to 4 take (`USED`), no pause, a line a part, the detail
    /// only on a failure. The same writes and checks in the same order as the survey's.
    Production,
}

/// The production order's held detail of one part, on the heap: the longest stage's lines and more.
pub const HELD_BYTES: usize = 64 * 1024;

/// The production order's log: no pause; a part's detail lines held in `buf` (the latest kept, the oldest let go when
/// it is full) and logged only if the part fails; notes logged at once through `inner`.
pub struct Held<'a, O: Out> { pub inner: O, buf: &'a mut [u8], len: usize, dropped: u32, failing: bool }

impl<'a, O: Out> Held<'a, O> {
    pub fn new(inner: O, buf: &'a mut [u8]) -> Held<'a, O> { Held { inner, buf, len: 0, dropped: 0, failing: false } }
    // One line as a length byte and its bytes (cut at 255).
    fn hold(&mut self, args: fmt::Arguments) {
        let mut line = Cut { bytes: [0; 255], len: 0 };
        let _ = fmt::write(&mut line, args);
        let n = line.len;
        if n + 1 > self.buf.len() { self.dropped += 1; return; }
        while self.len + n + 1 > self.buf.len() {
            let first = 1 + self.buf[0] as usize;
            self.buf.copy_within(first..self.len, 0);
            self.len -= first;
            self.dropped += 1;
        }
        self.buf[self.len] = n as u8;
        self.buf[self.len + 1..self.len + 1 + n].copy_from_slice(&line.bytes[..n]);
        self.len += n + 1;
    }
}

impl<O: Out> Out for Held<'_, O> {
    fn line(&mut self, args: fmt::Arguments) { if self.failing { self.inner.line(args) } else { self.hold(args) } }
    fn pause(&mut self, _: u32) {}
    fn note(&mut self, args: fmt::Arguments) { self.inner.note(args) }
    fn part(&mut self) { (self.len, self.dropped, self.failing) = (0, 0, false); }
    fn failed(&mut self) {
        if self.failing { return; }
        self.failing = true;
        if self.dropped > 0 { self.inner.line(format_args!("{} EARLIER LINES OF THIS PART'S DETAIL WERE NOT KEPT", self.dropped)); }
        let mut at = 0;
        while at < self.len {
            let n = self.buf[at] as usize;
            self.inner.line(format_args!("{}", core::str::from_utf8(&self.buf[at + 1..at + 1 + n]).unwrap_or("?")));
            at += 1 + n;
        }
        (self.len, self.dropped) = (0, 0);
    }
}

// A line formatted into a fixed buffer, cut at its end.
struct Cut { bytes: [u8; 255], len: usize }
impl fmt::Write for Cut {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let n = s.len().min(self.bytes.len() - self.len);
        self.bytes[self.len..self.len + n].copy_from_slice(&s.as_bytes()[..n]);
        self.len += n;
        Ok(())
    }
}

/// One register of a batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Named { pub r: R, pub inst: u8, pub reg: Reg }

impl fmt::Display for Named {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.r.block() {
            "" if self.r.instances() > 1 => write!(f, "{}{} AT {:05X}", self.r.name(), self.inst, self.reg.offset()),
            "" => write!(f, "{} AT {:05X}", self.r.name(), self.reg.offset()),
            block => write!(f, "{}{}_{} AT {:05X}", block, self.inst, self.r.name(), self.reg.offset()),
        }
    }
}

/// A batch's registers, in reading order.
#[derive(Clone, Copy, Debug)]
pub struct List { items: [Option<Named>; MAX_REGISTERS], len: usize }

impl List {
    const fn new() -> List { List { items: [None; MAX_REGISTERS], len: 0 } }
    fn push(&mut self, r: R, inst: u8) {
        if let (Some(reg), true) = (r.at(inst), self.len < MAX_REGISTERS) {
            self.items[self.len] = Some(Named { r, inst, reg });
            self.len += 1;
        }
    }
    pub fn iter(&self) -> impl Iterator<Item = Named> + '_ { self.items[..self.len].iter().flatten().copied() }
    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    /// Only the registers steps 2 to 4 take (`USED`), in the same order.
    pub fn used(&self) -> List {
        let mut list = List::new();
        for n in self.iter().filter(|n| USED.contains(&n.r)) { list.push(n.r, n.inst); }
        list
    }
}

/// What earlier batches showed that later ones depend on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Known {
    /// Batch 3: whether HUBP n's power domain (2n) is on; None until batch 3 has read all four.
    pub hubp_powered: Option<[bool; 4]>,
    /// Batch 2: the one timing generator that runs.
    pub running_otg: Option<u8>,
    /// Batch 4: the one powered HUBP that follows it, the firmware's pipe (batches 9 and 10 read its blocks).
    pub firmware_pipe: Option<u8>,
}

/// What `optc1_read_otg_state` reads, and the clock and source of each timing generator.
const OTG_STATE: [R; 19] = {
    use R::*;
    [OtgControl, OtgHTotal, OtgHBlankStartEnd, OtgHSyncA, OtgHSyncACntl, OtgVTotal, OtgVTotalMin, OtgVTotalMax,
     OtgVTotalControl, OtgVBlankStartEnd, OtgVSyncA, OtgVSyncACntl, OtgVerticalInterrupt1Control,
     OtgVerticalInterrupt1Position, OtgVerticalInterrupt2Control, OtgVerticalInterrupt2Position, OptcInputGlobalControl,
     OtgClockControl, OptcDataSourceSelect]
};
const HUBP_STATE: [R; 5] = {
    use R::*;
    [HubpreqPrimarySurfaceAddress, HubpreqPrimarySurfaceAddressHigh, HubpSurfaceConfig, HubpreqSurfacePitch, HubpCntl]
};
const APERTURE: [R; 6] = {
    use R::*;
    [DcnVmFbLocationBase, DcnVmFbLocationTop, DcnVmFbOffset, DcnVmAgpBot, DcnVmAgpTop, DcnVmAgpBase]
};
/// Batch 8 around the scratch registers: whether the board has a DMCUB and its control; its mailboxes.
const DMCUB_HEAD: [R; 3] = [R::CcDcPipeDis, R::DmcubCntl, R::DmcubSecCntl];
/// Batch 11 (213-DRV-0095): HDMI-A-1's DDC line, DC_GPIO_DDC1 (the VBIOS's I2C line 0), and the I2C engine, as
/// dce_i2c_hw and hw_ddc read them before a transfer: who holds the engine, its last transfer, the line's speed and
/// set-up, the four transactions, the microsecond time base its speed is set from, and the pads' mode and levels.
/// DC_I2C_DATA is not read: a read of it moves its index.
const DDC1: [(R, u8); 15] = {
    use R::*;
    [(DcI2cArbitration, 0), (DcI2cControl, 0), (DcI2cSwStatus, 0), (DcI2cDdc1HwStatus, 0), (DcI2cDdc1Speed, 0), (DcI2cDdc1Setup, 0),
     (DcI2cTransaction, 0), (DcI2cTransaction, 1), (DcI2cTransaction, 2), (DcI2cTransaction, 3), (MicrosecondTimeBaseDiv, 0),
     (DcGpioDdc1Mask, 0), (DcGpioDdc1A, 0), (DcGpioDdc1En, 0), (DcGpioDdc1Y, 0)]
};
/// Batch 12 (213-DRV-0112): what steps 2 to 4 would take of HDMI-A-2 (VBIOS path 1: PHY B, DIG back end B, DDC line 2,
/// HPD 2), the registers batches 1, 5, 7 and 11 read for HDMI-A-1 at its instances, and the DDC pads' I2C mode
/// (DC_GPIO_AUX_CTRL_5, PHY_AUX_CNTL), which Display Core's hw_ddc sets before a hardware transfer on DCN 2.x and step
/// 2 does not. Never DC_I2C_DATA, nor a PHY's DPCS or RDPCS (the survey's rule).
pub const HDMI_A_2: [(R, u8); 15] = {
    use R::*;
    [(HpdIntStatus, 1), (HpdControl, 1), (DigFeCntl, 1), (DigBeCntl, 1), (DigBeEnCntl, 1), (PhypllPixclkResyncCntl, 1),
     (DcI2cDdc2HwStatus, 0), (DcI2cDdc2Speed, 0), (DcI2cDdc2Setup, 0), (DcGpioDdc2Mask, 0), (DcGpioDdc2A, 0), (DcGpioDdc2En, 0),
     (DcGpioDdc2Y, 0), (DcGpioAuxCtrl5, 0), (PhyAuxCntl, 0)]
};
const DMCUB_TAIL: [R; 10] = {
    use R::*;
    [DmcubInbox0Wptr, DmcubInbox0Rptr, DmcubInbox1BaseAddress, DmcubInbox1Size, DmcubInbox1Wptr, DmcubInbox1Rptr,
     DmcubOutbox1Wptr, DmcubOutbox1Rptr, DmcubGpintDatain1, DmcubGpintDataout]
};
/// Batch 9: the firmware's HUBP as step 4's reference (`hubp2_setup`'s request, DLG and TTU registers, and its viewport).
const HUBP_REFERENCE: [R; 40] = {
    use R::*;
    [HubpAddrConfig, HubpTilingConfig, HubpPriViewportStart, HubpPriViewportDimension, HubpReqSizeConfig, HubpReqSizeConfigC,
     HubpClkCntl, HubpreqSurfaceControl, HubpreqExpansionMode, HubpreqTtuQosWm, HubpreqGlobalTtuCntl, HubpreqSurf0TtuCntl0,
     HubpreqSurf0TtuCntl1, HubpreqCur0TtuCntl0, HubpreqCur0TtuCntl1, HubpreqVmMxL1TlbCntl, HubpreqBlankOffset0,
     HubpreqBlankOffset1, HubpreqDstDimensions, HubpreqDstAfterScaler, HubpreqPrefetchSettings, HubpreqVblankParameters0,
     HubpreqVblankParameters1, HubpreqVblankParameters2, HubpreqVblankParameters3, HubpreqVblankParameters4,
     HubpreqNomParameters0, HubpreqNomParameters1, HubpreqNomParameters4, HubpreqNomParameters5, HubpreqPerLineDeliveryPre,
     HubpreqPerLineDelivery, HubpreqCursorSettings, HubpreqRefFreqToPixFreq, HubpreqDstYDeltaDrqLimit, HubpretControl,
     HubpreqVmidSettings0, HubpreqFlipControl, HubpreqVmSystemApertureLow, HubpreqVmSystemApertureHigh]
};
/// Batch 10: the firmware's DPP (its clock, format, scaler and colour stages).
const DPP_REFERENCE: [R; 17] = {
    use R::*;
    [DppControl, CnvcSurfacePixelFormat, CnvcFormatControl, DsclSclMode, DsclControl, DsclRecoutStart, DsclRecoutSize,
     DsclMpcSize, DsclLbDataFormat, CmControl, CmIcscControl, CmGamutRemapControl, CmDgamControl, CmBlndgamControl,
     DsclTapControl, DsclHorzScaleRatio, DsclVertScaleRatio]
};
/// Batch 10: the MPC tree, DCHUBBUB's arbitration and DCCG's DPP clocks, shared by every pipe.
const MPCC_STATE: [R; 4] = [R::MpccTopSel, R::MpccBotSel, R::MpccOppId, R::MpccControl];
const SHARED: [R; 6] = {
    use R::*;
    [DchubbubArbDfReqOutstand, DchubbubArbSatLevel, DchubbubArbDramStateCntl, DchubbubArbWatermarkChangeCntl,
     DchubbubGlobalTimerCntl, DppclkDtoCtrl]
};
/// The DIG back ends of the board's outputs: PHYs A, B, D and E (the VBIOS's paths; C has no connector).
pub const BOARD_PHYS: [u8; 4] = [0, 1, 3, 4];

/// What steps 2 to 4 take from step 1's values (`light::assess`, `light::reference`, `ddc::read`), each at every
/// instance the survey reads it: the production order reads these alone (213-DRV-0098). `tests/display_batches_host.rs`
/// and `tests/display_ddc_host.rs` show that each is taken, and that no other register the survey reads changes what
/// steps 2 to 4 do.
pub const USED: [R; 77] = {
    use R::*;
    [// Batch 1: HPD 1's sense. Batch 2: which timing generator runs, from which OPP; the firmware's timing for stage 9.
     HpdIntStatus, OtgControl, OptcDataSourceSelect, OtgHTotal, OtgVTotal, OtgHBlankStartEnd, OtgVBlankStartEnd,
     // Batches 3 and 4: the powered HUBPs, what they follow, the firmware's surface's format and pitch.
     DomainPgStatus, HubpCntl, HubpSurfaceConfig, HubpreqSurfacePitch,
     // Batch 5: the DIG front and back ends. Batch 6: the frame buffer's base. Batch 8: the DMCUB's control.
     DigFeCntl, DigBeCntl, DigBeEnCntl, DcnVmFbLocationBase, DmcubCntl,
     // Batch 9: the firmware's HUBP and OTG, stage 9's reference and what pipe 1 copies.
     HubpTilingConfig, HubpPriViewportDimension, HubpReqSizeConfig, HubpReqSizeConfigC, HubpreqSurfaceControl, HubpreqExpansionMode,
     HubpreqTtuQosWm, HubpreqGlobalTtuCntl, HubpreqSurf0TtuCntl0, HubpreqSurf0TtuCntl1, HubpreqCur0TtuCntl0, HubpreqCur0TtuCntl1,
     HubpreqVmMxL1TlbCntl, HubpreqBlankOffset0, HubpreqDstDimensions, HubpreqDstAfterScaler, HubpreqPrefetchSettings,
     HubpreqVblankParameters0, HubpreqVblankParameters1, HubpreqVblankParameters2, HubpreqVblankParameters3,
     HubpreqVblankParameters4, HubpreqPerLineDeliveryPre, HubpreqPerLineDelivery, HubpreqRefFreqToPixFreq,
     HubpreqDstYDeltaDrqLimit, HubpretControl, HubpreqVmidSettings0, HubpreqFlipControl, HubpreqVmSystemApertureLow,
     HubpreqVmSystemApertureHigh, OtgVstartupParam, OtgVupdateParam, OtgVreadyParam, HubpreqDebugDb,
     // Batch 10: the firmware's DPP as pipe 1 copies it, the MPC tree, the reference clock's route.
     CnvcSurfacePixelFormat, CnvcFormatControl, DsclSclMode, DsclControl, DsclLbDataFormat, CmControl, CmIcscControl,
     CmGamutRemapControl, CmDgamControl, CmBlndgamControl, DsclTapControl, DsclHorzScaleRatio, DsclVertScaleRatio, MpccTopSel,
     MpccBotSel, MpccOppId, MpccControl, MpcOutMux, DchubbubGlobalTimerCntl, RefclkCntl,
     // Batch 11: the I2C engine and DDC1's pads as step 2 finds them and puts them back.
     DcI2cArbitration, DcI2cControl, DcI2cDdc1Speed, DcI2cDdc1Setup, DcI2cTransaction, DcGpioDdc1Mask]
};

pub fn title(batch: u8) -> &'static str {
    match batch {
        1 => "HPD", 2 => "OTG STATE", 3 => "POWER", 4 => "HUBP", 5 => "DIG", 6 => "APERTURE", 7 => "CLOCKS", 8 => "DMCUB",
        9 => "PIPE REFERENCE", 10 => "PATH AND SHARED", 11 => "DDC1", SECOND => "HDMI-A-2", PIPE => "PIPE", PLANE => "PIPE PLANE", _ => "?",
    }
}

/// The registers batch `batch` reads, given what the earlier ones showed.
pub fn plan(batch: u8, known: &Known) -> List {
    let mut list = List::new();
    match batch {
        1 => for n in 0..5 { list.push(R::HpdIntStatus, n); list.push(R::HpdControl, n); },
        2 => for n in 0..4 { for r in OTG_STATE { list.push(r, n); } },
        3 => for n in 0..8 { list.push(R::DomainPgStatus, n); },
        4 => if let Some(powered) = known.hubp_powered {
            for n in 0..4 { if powered[n as usize] { for r in HUBP_STATE { list.push(r, n); } } }
        },
        5 => {
            for n in 0..5 { list.push(R::DigFeCntl, n); }
            for n in BOARD_PHYS { list.push(R::DigBeCntl, n); list.push(R::DigBeEnCntl, n); }
        }
        6 => for r in APERTURE { list.push(r, 0); },
        7 => {
            for n in 0..4 { list.push(R::OtgPixelRateCntl, n); list.push(R::OtgPhypllPixelRateCntl, n); }
            for n in 0..5 { list.push(R::PhypllPixclkResyncCntl, n); }
            list.push(R::DentistDispclkCntl, 0);
        }
        // Its timer first and last, so that two reads show whether it counts.
        8 => {
            list.push(R::DmcubTimerCurrent, 0);
            for r in DMCUB_HEAD { list.push(r, 0); }
            for n in 0..16 { list.push(R::DmcubScratch, n); }
            for r in DMCUB_TAIL { list.push(r, 0); }
            list.push(R::DmcubTimerCurrent, 0);
        }
        9 => if let (Some(p), Some(t)) = (known.firmware_pipe, known.running_otg) { reference_hubp(&mut list, p, t) },
        10 => if let Some(p) = known.firmware_pipe { path_and_shared(&mut list, p) },
        11 => for (r, n) in DDC1 { list.push(r, n); },
        SECOND => for (r, n) in HDMI_A_2 { list.push(r, n); },
        _ => {}
    }
    list
}

fn reference_hubp(list: &mut List, p: u8, t: u8) {
    for r in HUBP_REFERENCE { list.push(r, p); }
    for r in [R::OtgVstartupParam, R::OtgVupdateParam, R::OtgVreadyParam] { list.push(r, t); }
    list.push(R::HubpreqDebugDb, p);
}

fn path_and_shared(list: &mut List, p: u8) {
    for r in DPP_REFERENCE { list.push(r, p); }
    for n in 0..4 { for r in MPCC_STATE { list.push(r, n); } }
    for n in 0..4 { list.push(R::MpcOutMux, n); }
    for n in 0..20 { list.push(R::DchubbubArbWatermark, n); }
    for r in SHARED { list.push(r, 0); }
    for n in 0..4 { list.push(R::DppclkDtoParam, n); }
    list.push(R::RefclkCntl, 0);
}

/// Batch 4's registers for every HUBP, for naming before batch 3 has been read.
fn every_hubp() -> List {
    let mut list = List::new();
    for n in 0..4 { for r in HUBP_STATE { list.push(r, n); } }
    list
}

/// A batch's values as read.
struct Values { items: [Option<(Named, Option<u32>)>; MAX_REGISTERS], len: usize }

impl Values {
    fn new() -> Values { Values { items: [None; MAX_REGISTERS], len: 0 } }
    fn push(&mut self, n: Named, v: Option<u32>) { if self.len < MAX_REGISTERS { self.items[self.len] = Some((n, v)); self.len += 1; } }
    fn get(&self, r: R, inst: u8) -> Option<u32> {
        self.items[..self.len].iter().flatten().find(|(n, _)| n.r == r && n.inst == inst).and_then(|(_, v)| *v)
    }
    fn field(&self, r: R, inst: u8, f: Field) -> Option<u32> { self.get(r, inst).map(|v| f.get(v)) }
    /// The last value read of a register read more than once.
    fn last(&self, r: R, inst: u8) -> Option<u32> {
        self.items[..self.len].iter().flatten().filter(|(n, _)| n.r == r && n.inst == inst).last().and_then(|(_, v)| *v)
    }
}

/// A register's value in hex, or a dash.
struct Hex32(Option<u32>);
impl fmt::Display for Hex32 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { match self.0 { Some(v) => write!(f, "{:08X}", v), None => f.write_str("-") } }
}

/// A value or a dash for one that was not read.
struct Opt(Option<u32>);
impl fmt::Display for Opt {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { match self.0 { Some(v) => write!(f, "{}", v), None => f.write_str("-") } }
}

/// What a run did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Report { pub batches_read: u8, pub registers_read: usize }

/// Every value a run read, for step 3 to choose its pipe from: all eleven batches (305 registers on the Ryzen's state in
/// the survey's order) and batch 12's own (213-DRV-0112).
pub const STORE: usize = 336;
#[derive(Clone, Copy, Debug)]
pub struct Store { items: [Option<(R, u8, u32)>; STORE], len: usize, batches: u8 }

impl Default for Store { fn default() -> Store { Store { items: [None; STORE], len: 0, batches: 0 } } }

impl Store {
    fn push(&mut self, r: R, inst: u8, v: u32) { if self.len < STORE { self.items[self.len] = Some((r, inst, v)); self.len += 1; } }
    pub fn get(&self, r: R, inst: u8) -> Option<u32> {
        self.items[..self.len].iter().flatten().find(|(x, i, _)| *x == r && *i == inst).map(|(_, _, v)| *v)
    }
    pub fn field(&self, r: R, inst: u8, f: Field) -> Option<u32> { self.get(r, inst).map(|v| f.get(v)) }
    /// How many batches were read in full.
    pub fn batches(&self) -> u8 { self.batches }
    /// For host cases: a value as if step 1 had read it.
    pub fn set(&mut self, r: R, inst: u8, v: u32) {
        match self.items[..self.len].iter_mut().flatten().find(|(x, i, _)| *x == r && *i == inst) { Some(e) => e.2 = v, None => self.push(r, inst, v) }
    }
    pub fn set_batches(&mut self, n: u8) { self.batches = n; }
}

/// Step 1: batches 1 to `allowed` read, the others only named with the registers they would read.
pub fn run(bus: &mut impl Bus, allowed: u8, out: &mut dyn Out) -> Report { run_into(bus, allowed, out, &mut Store::default()) }

/// Step 1 in the survey's order, every value read kept in `store`.
pub fn run_into(bus: &mut impl Bus, allowed: u8, out: &mut dyn Out, store: &mut Store) -> Report { read_into(bus, allowed, Order::Survey, out, store) }

/// Step 1 in `order`, every value read kept in `store`. The production order reads of each batch only the registers
/// steps 2 to 4 take (`List::used`), in the survey's order, and counts the batch read when none of them is in it.
pub fn read_into(bus: &mut impl Bus, allowed: u8, order: Order, out: &mut dyn Out, store: &mut Store) -> Report {
    read_into_beside(bus, allowed, order, out, store, [false; 4])
}

/// Step 1 as `read_into`, with `ours` the timing generators display runs itself (213-DRV-0113): batch 2 counts them out of
/// the running, so that batches 9 and 10 still name the firmware's pipe beside display's.
pub fn read_into_beside(bus: &mut impl Bus, allowed: u8, order: Order, out: &mut dyn Out, store: &mut Store, ours: [bool; 4]) -> Report {
    let allowed = allowed.min(BATCHES);
    out.line(format_args!("STEP 1: THE ENGINE'S STATE IN {} BATCHES, READ ONLY; THIS BUILD MAY READ {}", BATCHES, allowed));
    let mut known = Known::default();
    let mut report = Report::default();
    if ours.iter().any(|&o| o) { out.line(format_args!("STEP 1 BESIDE DISPLAY'S OWN OTG{}{}{}{}: COUNTED OUT OF THE RUNNING", ours[0] as u8, ours[1] as u8, ours[2] as u8, ours[3] as u8)); }
    for batch in 1..=BATCHES {
        out.part();
        let list = plan(batch, &known);
        if batch > allowed {
            name_only(batch, &list, &known, allowed, out);
            continue;
        }
        if list.is_empty() {
            let why = if batch >= 9 { "NOT ONE RUNNING OTG WITH ONE POWERED HUBP ON IT (BATCHES 2 AND 4)" } else { "BATCH 3 SHOWED NO HUBP POWERED, OR WAS NOT READ IN FULL" };
            out.note(format_args!("B{} {}: NOTHING TO READ: {}", batch, title(batch), why));
            continue;
        }
        let list = if order == Order::Production { list.used() } else { list };
        out.line(format_args!("B{} {}: READING {} REGISTERS NEXT", batch, title(batch), list.len()));
        out.pause(PAUSE_BEFORE_READ_MS);
        let mut values = Values::new();
        for n in list.iter() {
            let v = bus.read(n.reg);
            match v {
                Some(v) => { out.line(format_args!("B{} {} = {:08X}", batch, n, v)); report.registers_read += 1; store.push(n.r, n.inst, v); }
                None => out.note(format_args!("B{} {}: NOT MAPPED, NOT READ", batch, n)),
            }
            values.push(n, v);
        }
        summarize(batch, &values, &mut known, ours, out);
        report.batches_read += 1;
        // The store counts the batches read one after another from the first: steps 3 and 4 need 1 to 10 all read,
        // and batch 11 after an empty 9 or 10 does not stand for them.
        if store.batches + 1 == batch { store.batches = batch; }
        out.pause(PAUSE_BETWEEN_BATCHES_MS);
    }
    out.part();
    out.note(format_args!("STEP 1 DONE: {} OF {} BATCHES READ, {} REGISTERS, NOTHING WRITTEN{}", report.batches_read, BATCHES, report.registers_read,
        if order == Order::Production { "; ONLY WHAT STEPS 2 TO 4 TAKE (THE PRODUCTION ORDER)" } else { "" }));
    report
}

/// Batch 12 (213-DRV-0112): HDMI-A-2's blocks read after the eleven when `allowed`, else only named. It is read whole in
/// either order and its values summed up in notes, which both orders log: the second socket's bring-up starts from them.
/// A register an earlier batch kept is not kept twice.
pub fn read_second(bus: &mut impl Bus, allowed: bool, out: &mut dyn Out, store: &mut Store) -> Report {
    out.part();
    let list = plan(SECOND, &Known::default());
    let mut report = Report::default();
    if !allowed {
        out.line(format_args!("B12 {}: NOT READ (STEP 1 READS NOT ALL ELEVEN BATCHES); WOULD READ {} REGISTERS:", title(SECOND), list.len()));
        for n in list.iter() { out.line(format_args!("B12 WOULD READ {}", n)); }
        return report;
    }
    out.line(format_args!("B12 {}: READING {} REGISTERS NEXT, NEVER WRITTEN (213-DRV-0112)", title(SECOND), list.len()));
    out.pause(PAUSE_BEFORE_READ_MS);
    let mut values = Values::new();
    for n in list.iter() {
        let v = bus.read(n.reg);
        match v {
            Some(v) => {
                out.line(format_args!("B12 {} = {:08X}", n, v));
                report.registers_read += 1;
                if store.get(n.r, n.inst).is_none() { store.push(n.r, n.inst, v); }
            }
            None => out.note(format_args!("B12 {}: NOT MAPPED, NOT READ", n)),
        }
        values.push(n, v);
    }
    summarize_second(&values, out);
    report.batches_read = 1;
    if store.batches == BATCHES { store.batches = SECOND; }
    out.pause(PAUSE_BETWEEN_BATCHES_MS);
    out.note(format_args!("B12 {} DONE: {} OF {} REGISTERS READ, NOTHING WRITTEN", title(SECOND), report.registers_read, list.len()));
    report
}

/// Batch 13's registers for pipe `n` (213-DRV-0113): the other timing generators' marks, its timing generator as
/// `optc1_read_otg_state` reads it, its pixel rate, its OPP and pattern generator, its HUBP's and DPP's power domains,
/// DCCG's DPP clock for it, its MPCC and its OPP's MPC_OUT mux.
fn pipe_list(n: u8) -> List {
    let mut list = List::new();
    for t in (0..4).filter(|&t| t != n) { list.push(R::OtgVerticalInterrupt2Position, t); }
    for r in OTG_STATE { list.push(r, n); }
    for r in [R::OtgPixelRateCntl, R::OtgPhypllPixelRateCntl, R::OppPipeControl, R::DpgControl, R::DpgDimensions] { list.push(r, n); }
    list.push(R::DomainPgStatus, 2 * n);
    list.push(R::DomainPgStatus, 2 * n + 1);
    list.push(R::DppclkDtoCtrl, 0);
    list.push(R::DppclkDtoParam, n);
    for r in MPCC_STATE { list.push(r, n); }
    list.push(R::MpcOutMux, n);
    list
}
/// Batch 14's for pipe `n`: its HUBP's state and what stages 11 to 15 write there, and its DPP's, each only where batch
/// 13 found its domain on.
fn plane_list(n: u8, hubp_on: bool, dpp_on: bool) -> List {
    let mut list = List::new();
    if hubp_on { for r in HUBP_STATE.iter().chain(HUBP_REFERENCE.iter()) { list.push(*r, n); } }
    if dpp_on { for r in DPP_REFERENCE { list.push(r, n); } }
    list
}

// A batch's list read whole, each value logged as a line and kept; its values for the summary.
fn read_list(bus: &mut impl Bus, batch: u8, title: fmt::Arguments, list: &List, out: &mut dyn Out, store: &mut Store, report: &mut Report) -> Values {
    out.line(format_args!("B{} {}: READING {} REGISTERS NEXT, NEVER WRITTEN (213-DRV-0113)", batch, title, list.len()));
    out.pause(PAUSE_BEFORE_READ_MS);
    let mut values = Values::new();
    for n in list.iter() {
        let v = bus.read(n.reg);
        match v {
            Some(v) => { out.line(format_args!("B{} {} = {:08X}", batch, n, v)); report.registers_read += 1; store.set(n.r, n.inst, v); }
            None => out.note(format_args!("B{} {}: NOT MAPPED, NOT READ", batch, n)),
        }
        values.push(n, v);
    }
    values
}

/// Batches 13 and 14 (213-DRV-0113): pipe `n` as the firmware left it, read when a second run is about to take it, whole in
/// either order and never written, its values summed up in notes both orders log. The HUBP's and DPP's registers are read
/// only where their domain is on, as batch 4 reads a HUBP.
pub fn read_pipe(bus: &mut impl Bus, n: u8, out: &mut dyn Out, store: &mut Store) -> Report {
    use R::*;
    let mut report = Report::default();
    out.part();
    let list = pipe_list(n);
    let v = read_list(bus, PIPE, format_args!("PIPE {}", n), &list, out, store, &mut report);
    let g = |r: R, i: u8, f: Field| Opt(v.field(r, i, f));
    let x = |r: R, i: u8| Hex32(v.get(r, i));
    let mark = |t: u8| Mark(v.get(OtgVerticalInterrupt2Position, t));
    out.note(format_args!("B13 PIPE {}: OTG{} RUNNING {}, CLOCK_ON {}, FROM OPP {}; MARKS OTG0 {} OTG1 {} OTG2 {} OTG3 {} ({:04X}: DISPLAY'S)", n, n,
        g(OtgControl, n, OTG_CURRENT_MASTER_EN_STATE), g(OtgClockControl, n, OTG_CLOCK_ON), g(OptcDataSourceSelect, n, OPTC_SEG0_SRC_SEL),
        mark(0), mark(1), mark(2), mark(3), crate::light::RESTART_MARK));
    out.note(format_args!("B13 PIPE {}: OTG{} PIXEL RATE: SOURCE {}, DP_DTO {}, PHYPLL SOURCE {}, PLL SOURCE {} ({}, {})", n, n,
        g(OtgPixelRateCntl, n, OTG_PIXEL_RATE_SOURCE), g(OtgPixelRateCntl, n, DP_DTO_ENABLE), g(OtgPhypllPixelRateCntl, n, OTG_PHYPLL_PIXEL_RATE_SOURCE),
        g(OtgPhypllPixelRateCntl, n, OTG_PIXEL_RATE_PLL_SOURCE), x(OtgPixelRateCntl, n), x(OtgPhypllPixelRateCntl, n)));
    out.note(format_args!("B13 PIPE {}: OPP{} PIPE_CONTROL {} (CLOCK_EN {}); DPG CONTROL {} (EN {}), DIMENSIONS {}", n, n,
        x(OppPipeControl, n), g(OppPipeControl, n, OPP_PIPE_CLOCK_EN), x(DpgControl, n), g(DpgControl, n, DPG_EN), x(DpgDimensions, n)));
    let on = |d: u8| v.field(DomainPgStatus, d, DOMAIN_PGFSM_PWR_STATUS).map(|s| s == PGFSM_POWER_ON);
    let word = |d: u8| match on(d) { Some(true) => "ON", Some(false) => "OFF", None => "-" };
    out.note(format_args!("B13 PIPE {}: HUBP{} DOMAIN {}, DPP{} DOMAIN {}; DPPCLK DTO CTRL {}, DTO{} {}/{} ({})", n, n, word(2 * n), n, word(2 * n + 1),
        x(DppclkDtoCtrl, 0), n, g(DppclkDtoParam, n, DTO_PHASE), g(DppclkDtoParam, n, DTO_MODULO), x(DppclkDtoParam, n)));
    out.note(format_args!("B13 PIPE {}: MPCC{} TOP {} BOT {} OPP {} MODE {} ALPHA {} (CONTROL {}); MPC_OUT MUX {}: {} (15: NONE)", n, n,
        g(MpccTopSel, n, MPCC_SEL), g(MpccBotSel, n, MPCC_SEL), g(MpccOppId, n, MPCC_SEL), g(MpccControl, n, MPCC_MODE), g(MpccControl, n, MPCC_GLOBAL_ALPHA),
        x(MpccControl, n), n, g(MpcOutMux, n, MPCC_SEL)));
    out.note(format_args!("B13 PIPE {} DONE: {} OF {} REGISTERS READ, NOTHING WRITTEN", n, report.registers_read, list.len()));
    out.pause(PAUSE_BETWEEN_BATCHES_MS);
    out.part();
    let (hubp_on, dpp_on) = (on(2 * n) == Some(true), on(2 * n + 1) == Some(true));
    let plane = plane_list(n, hubp_on, dpp_on);
    let before = report.registers_read;
    let p = read_list(bus, PLANE, format_args!("PIPE {} PLANE", n), &plane, out, store, &mut report);
    let g = |r: R, f: Field| Opt(p.field(r, n, f));
    if hubp_on {
        out.note(format_args!("B14 PIPE {} PLANE: HUBP{} DCHUBP_CNTL {} (BLANK {}, VTG_SEL {}, TTU_DISABLE {}), SURFACE {:04X}_{}, FORMAT {}, PITCH FIELD {}, CLK_CNTL {}",
            n, n, Hex32(p.get(HubpCntl, n)), g(HubpCntl, HUBP_BLANK_EN), g(HubpCntl, HUBP_VTG_SEL), g(HubpCntl, HUBP_TTU_DISABLE),
            p.field(HubpreqPrimarySurfaceAddressHigh, n, PRIMARY_SURFACE_ADDRESS_HIGH).unwrap_or(0), Hex32(p.get(HubpreqPrimarySurfaceAddress, n)),
            g(HubpSurfaceConfig, SURFACE_PIXEL_FORMAT), g(HubpreqSurfacePitch, PITCH), Hex32(p.get(HubpClkCntl, n))));
    } else { out.note(format_args!("B14 PIPE {} PLANE: HUBP{}'S DOMAIN NOT ON: ITS REGISTERS NOT READ", n, n)); }
    if dpp_on {
        out.note(format_args!("B14 PIPE {} PLANE: DPP{} CONTROL {} (CLOCK {}), FORMAT {}, DSCL_MODE {}, RECOUT {}x{} AT {},{}, MPC {}x{}, CM BYPASS {}", n, n,
            Hex32(p.get(DppControl, n)), g(DppControl, DPP_CLOCK_ENABLE), g(CnvcSurfacePixelFormat, CNVC_SURFACE_PIXEL_FORMAT), g(DsclSclMode, DSCL_MODE),
            g(DsclRecoutSize, LOW14), g(DsclRecoutSize, HIGH14), g(DsclRecoutStart, RECOUT_START_X), g(DsclRecoutStart, RECOUT_START_Y),
            g(DsclMpcSize, LOW14), g(DsclMpcSize, HIGH14), g(CmControl, CM_BYPASS)));
    } else { out.note(format_args!("B14 PIPE {} PLANE: DPP{}'S DOMAIN NOT ON: ITS REGISTERS NOT READ", n, n)); }
    out.note(format_args!("B14 PIPE {} PLANE DONE: {} OF {} REGISTERS READ, NOTHING WRITTEN", n, report.registers_read - before, plane.len()));
    out.pause(PAUSE_BETWEEN_BATCHES_MS);
    report.batches_read = 2;
    report
}

// A timing generator's mark register in four hex digits, or a dash.
struct Mark(Option<u32>);
impl fmt::Display for Mark {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { match self.0 { Some(v) => write!(f, "{:04X}", v & 0xFFFF), None => f.write_str("-") } }
}

/// One batch read on its own in `order`, its values kept in `store` and the batches' count left as it is (213-DRV-0112:
/// HPD 1 and the I2C engine as they are now, before an EDID read of a socket not taken). Read only.
pub fn read_one(bus: &mut impl Bus, batch: u8, order: Order, out: &mut dyn Out, store: &mut Store) -> Report {
    out.part();
    let mut known = Known::default();
    let list = plan(batch, &known);
    let list = if order == Order::Production { list.used() } else { list };
    let mut report = Report::default();
    out.line(format_args!("B{} {}: READING {} REGISTERS NEXT", batch, title(batch), list.len()));
    out.pause(PAUSE_BEFORE_READ_MS);
    let mut values = Values::new();
    for n in list.iter() {
        let v = bus.read(n.reg);
        match v {
            Some(v) => { out.line(format_args!("B{} {} = {:08X}", batch, n, v)); report.registers_read += 1; store.set(n.r, n.inst, v); }
            None => out.note(format_args!("B{} {}: NOT MAPPED, NOT READ", batch, n)),
        }
        values.push(n, v);
    }
    summarize(batch, &values, &mut known, [false; 4], out);
    report.batches_read = 1;
    report
}

fn name_only(batch: u8, list: &List, known: &Known, allowed: u8, out: &mut dyn Out) {
    if batch == 4 && known.hubp_powered.is_none() {
        let all = every_hubp();
        out.line(format_args!("B4 HUBP: NOT READ (THIS BUILD MAY READ {}); WOULD READ, ONLY FOR EACH HUBP n THAT B3 SHOWS POWERED (DOMAIN 2n):", allowed));
        for n in all.iter() { out.line(format_args!("B4 WOULD READ {}", n)); }
        return;
    }
    if (batch == 9 || batch == 10) && known.firmware_pipe.is_none() {
        let mut example = List::new();
        if batch == 9 { reference_hubp(&mut example, 0, 0) } else { path_and_shared(&mut example, 0) }
        out.line(format_args!("B{} {}: NOT READ (THIS BUILD MAY READ {}); WOULD READ, FOR THE PIPE B4 SHOWS ON THE RUNNING OTG (HERE PIPE 0):", batch, title(batch), allowed));
        for n in example.iter() { out.line(format_args!("B{} WOULD READ {}", batch, n)); }
        return;
    }
    out.line(format_args!("B{} {}: NOT READ (THIS BUILD MAY READ {}); WOULD READ {} REGISTERS:", batch, title(batch), allowed, list.len()));
    for n in list.iter() { out.line(format_args!("B{} WOULD READ {}", batch, n)); }
}

const LETTERS: [char; 5] = ['A', 'B', 'C', 'D', 'E'];

fn summarize(batch: u8, v: &Values, known: &mut Known, ours: [bool; 4], out: &mut dyn Out) {
    use R::*;
    match batch {
        1 => {
            let s = |n| Opt(v.field(HpdIntStatus, n, DC_HPD_SENSE));
            let e = |n| Opt(v.field(HpdControl, n, DC_HPD_EN));
            out.line(format_args!("B1 HPD SENSE 1:{} 2:{} 3:{} 4:{} 5:{}; ENABLED 1:{} 2:{} 3:{} 4:{} 5:{}",
                s(0), s(1), s(2), s(3), s(4), e(0), e(1), e(2), e(3), e(4)));
        }
        2 => {
            for n in 0..4 { summarize_otg(v, n, out); }
            let running: [bool; 4] = [0u8, 1, 2, 3].map(|n| !ours[n as usize] && v.field(OtgControl, n, OTG_CURRENT_MASTER_EN_STATE) == Some(1));
            if running.iter().filter(|&&r| r).count() == 1 { known.running_otg = running.iter().position(|&r| r).map(|t| t as u8); }
        }
        3 => {
            let on = |d: u8| v.field(DomainPgStatus, d, DOMAIN_PGFSM_PWR_STATUS).map(|s| s == PGFSM_POWER_ON);
            let word = |d: u8| match on(d) { Some(true) => "ON", Some(false) => "OFF", None => "-" };
            for n in 0..4u8 {
                out.line(format_args!("B3 PIPE {}: HUBP{} {}, DPP{} {}", n, n, word(2 * n), n, word(2 * n + 1)));
            }
            if let (Some(a), Some(b), Some(c), Some(d)) = (on(0), on(2), on(4), on(6)) { known.hubp_powered = Some([a, b, c, d]); }
        }
        4 => {
            for n in 0..4 {
                let Some(low) = v.get(HubpreqPrimarySurfaceAddress, n) else { continue };
                out.line(format_args!("B4 HUBP{}: SURFACE {:04X}_{:08X}, FORMAT {}, PITCH FIELD {}, ROTATION {}, BLANK {}, TTU_DISABLE {}",
                    n, v.field(HubpreqPrimarySurfaceAddressHigh, n, PRIMARY_SURFACE_ADDRESS_HIGH).unwrap_or(0), low,
                    Opt(v.field(HubpSurfaceConfig, n, SURFACE_PIXEL_FORMAT)), Opt(v.field(HubpreqSurfacePitch, n, PITCH)),
                    Opt(v.field(HubpSurfaceConfig, n, ROTATION_ANGLE)), Opt(v.field(HubpCntl, n, HUBP_BLANK_EN)),
                    Opt(v.field(HubpCntl, n, HUBP_TTU_DISABLE))));
            }
            // The firmware's pipe: the one powered HUBP that follows the one running timing generator.
            if let (Some(t), Some(powered)) = (known.running_otg, known.hubp_powered) {
                let on_it: [bool; 4] = [0u8, 1, 2, 3].map(|n| powered[n as usize] && v.field(HubpCntl, n, HUBP_VTG_SEL) == Some(t as u32));
                if on_it.iter().filter(|&&x| x).count() == 1 { known.firmware_pipe = on_it.iter().position(|&x| x).map(|n| n as u8); }
            }
        }
        9 => if let (Some(p), Some(t)) = (known.firmware_pipe, known.running_otg) { summarize_reference(v, p, t, out) },
        10 => if let Some(p) = known.firmware_pipe { summarize_path(v, p, out) },
        11 => summarize_ddc1(v, out),
        5 => {
            for n in 0..5u8 {
                out.line(format_args!("B5 DIG{} FE: FROM OTG {}, SYMCLK_FE_ON {}", LETTERS[n as usize],
                    Opt(v.field(DigFeCntl, n, DIG_SOURCE_SELECT)), Opt(v.field(DigFeCntl, n, DIG_SYMCLK_FE_ON))));
            }
            for n in BOARD_PHYS {
                out.line(format_args!("B5 DIG{} BE: ENABLED {}, MODE {}, FROM FE MASK {}, SYMCLK_BE_ON {}", LETTERS[n as usize],
                    Opt(v.field(DigBeEnCntl, n, DIG_ENABLE)), Opt(v.field(DigBeCntl, n, DIG_MODE)),
                    Opt(v.field(DigBeCntl, n, DIG_FE_SOURCE_SELECT)), Opt(v.field(DigBeEnCntl, n, DIG_SYMCLK_BE_ON))));
            }
        }
        6 => {
            let at = |r: R| v.field(r, 0, VM_16MIB).map(|x| (x as u64) << 24);
            let hex = |a: Option<u64>| Hex(a);
            out.line(format_args!("B6 APERTURE: FB {}..{} (TOP'S LAST 16 MIB), FB OFFSET {}, AGP {}..{} BASE {}",
                hex(at(DcnVmFbLocationBase)), hex(at(DcnVmFbLocationTop)), hex(at(DcnVmFbOffset)),
                hex(at(DcnVmAgpBot)), hex(at(DcnVmAgpTop)), hex(at(DcnVmAgpBase))));
        }
        7 => {
            for n in 0..4u8 {
                out.line(format_args!("B7 OTG{} PIXEL RATE: SOURCE {}, DP_DTO {}, PHYPLL SOURCE {}, PLL SOURCE {}", n,
                    Opt(v.field(OtgPixelRateCntl, n, OTG_PIXEL_RATE_SOURCE)), Opt(v.field(OtgPixelRateCntl, n, DP_DTO_ENABLE)),
                    Opt(v.field(OtgPhypllPixelRateCntl, n, OTG_PHYPLL_PIXEL_RATE_SOURCE)),
                    Opt(v.field(OtgPhypllPixelRateCntl, n, OTG_PIXEL_RATE_PLL_SOURCE))));
            }
            for n in 0..5u8 {
                out.line(format_args!("B7 PHYPLL{}: PIXCLK_ENABLE {}, DEEP COLOR {}", LETTERS[n as usize],
                    Opt(v.field(PhypllPixclkResyncCntl, n, PHYPLL_PIXCLK_ENABLE)),
                    Opt(v.field(PhypllPixclkResyncCntl, n, PHYPLL_DEEP_COLOR_CNTL))));
            }
            out.line(format_args!("B7 DENTIST: DISPCLK WDIVIDER {}, RDIVIDER {}",
                Opt(v.field(DentistDispclkCntl, 0, DENTIST_DISPCLK_WDIVIDER)), Opt(v.field(DentistDispclkCntl, 0, DENTIST_DISPCLK_RDIVIDER))));
        }
        8 => {
            let (t0, t1) = (v.get(DmcubTimerCurrent, 0), v.last(DmcubTimerCurrent, 0));
            let timer = match (t0, t1) { (Some(a), Some(b)) if a != b => "COUNTS", (Some(_), Some(_)) => "STILL", _ => "-" };
            out.line(format_args!("B8 DMCUB: ON THE BOARD {}, ENABLED {}, SOFT RESET {}, PWAIT {}; SECURE RESET {}, ITS STATUS {}; TIMER {} ({})",
                Opt(v.field(CcDcPipeDis, 0, DC_DMCUB_ENABLE)), Opt(v.field(DmcubCntl, 0, DMCUB_ENABLE)), Opt(v.field(DmcubCntl, 0, DMCUB_SOFT_RESET)),
                Opt(v.field(DmcubCntl, 0, DMCUB_PWAIT_MODE_STATUS)), Opt(v.field(DmcubSecCntl, 0, DMCUB_SEC_RESET)),
                Opt(v.field(DmcubSecCntl, 0, DMCUB_SEC_RESET_STATUS)), timer, Hex32(t1)));
            out.line(format_args!("B8 DMCUB: SCRATCH0 (BOOT STATUS) {}, SCRATCH7 (GPINT ANSWER) {}, SCRATCH10 {}, GPINT IN {} OUT {}",
                Hex32(v.get(DmcubScratch, 0)), Hex32(v.get(DmcubScratch, 7)), Hex32(v.get(DmcubScratch, 10)),
                Hex32(v.get(DmcubGpintDatain1, 0)), Hex32(v.get(DmcubGpintDataout, 0))));
            out.line(format_args!("B8 DMCUB: INBOX1 AT {} OF {} BYTES, READ {} WRITE {}; INBOX0 READ {} WRITE {}; OUTBOX1 READ {} WRITE {}",
                Hex32(v.get(DmcubInbox1BaseAddress, 0)), Opt(v.get(DmcubInbox1Size, 0)), Opt(v.get(DmcubInbox1Rptr, 0)), Opt(v.get(DmcubInbox1Wptr, 0)),
                Opt(v.get(DmcubInbox0Rptr, 0)), Opt(v.get(DmcubInbox0Wptr, 0)), Opt(v.get(DmcubOutbox1Rptr, 0)), Opt(v.get(DmcubOutbox1Wptr, 0))));
        }
        _ => {}
    }
}

/// A GPU address in hex, or a dash.
struct Hex(Option<u64>);
impl fmt::Display for Hex {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { match self.0 { Some(a) => write!(f, "{:X}", a), None => f.write_str("-") } }
}

/// The timing generator's state as its registers give it, in the convention of `Timing::otg`.
pub fn otg_state(v: &[(R, u32)]) -> Option<Otg> {
    let get = |r: R| v.iter().find(|(x, _)| *x == r).map(|(_, value)| *value);
    let (low, high) = (COUNTER_LOW, COUNTER_HIGH);
    use R::*;
    Some(Otg {
        h_total: low.get(get(OtgHTotal)?),
        h_blank_start: low.get(get(OtgHBlankStartEnd)?),
        h_blank_end: high.get(get(OtgHBlankStartEnd)?),
        h_sync_end: high.get(get(OtgHSyncA)?),
        v_total: low.get(get(OtgVTotal)?),
        v_blank_start: low.get(get(OtgVBlankStartEnd)?),
        v_blank_end: high.get(get(OtgVBlankStartEnd)?),
        v_sync_end: high.get(get(OtgVSyncA)?),
        h_sync_negative: SYNC_POL.get(get(OtgHSyncACntl)?) == 1,
        v_sync_negative: SYNC_POL.get(get(OtgVSyncACntl)?) == 1,
        vcount_init: low.get(get(OtgVBlankStartEnd)?),
    })
}

/// REF_FREQ_TO_PIX_FREQ (the reference clock over the pixel clock, 19 bits of fraction) with four decimals.
pub struct Ratio19(pub Option<u32>);
impl fmt::Display for Ratio19 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 { Some(v) => { let r = v as u64 * 10_000 >> 19; write!(f, "{}.{:04}", r / 10_000, r % 10_000) } None => f.write_str("-") }
    }
}

fn summarize_ddc1(v: &Values, out: &mut dyn Out) {
    use R::*;
    let owner = |s: Option<u32>| match s { Some(0) => "NOBODY", Some(1) => "SOFTWARE", Some(2) => "THE DMCU", Some(_) => "BOTH", None => "-" };
    out.line(format_args!("B11 I2C ENGINE: HELD BY {} (SW REQ {}, DMCU REQ {}); LAST TRANSFER STATUS {}, DONE {}; DDC SELECT {}; DDC1 HW STATUS {}",
        owner(v.field(DcI2cArbitration, 0, DC_I2C_REG_RW_CNTL_STATUS)), Opt(v.field(DcI2cArbitration, 0, DC_I2C_SW_USE_I2C_REG_REQ)),
        Opt(v.field(DcI2cArbitration, 0, DC_I2C_DMCU_USE_I2C_REG_REQ)), Opt(v.field(DcI2cSwStatus, 0, DC_I2C_SW_STATUS_BITS)),
        Opt(v.field(DcI2cSwStatus, 0, DC_I2C_SW_DONE)), Opt(v.field(DcI2cControl, 0, DC_I2C_DDC_SELECT)), Opt(v.field(DcI2cDdc1HwStatus, 0, DC_I2C_DDC1_HW_STATUS_BITS))));
    out.line(format_args!("B11 DDC1: ENABLED {}, TIME LIMIT {}, PRESCALE {}, THRESHOLD {}; MICROSECOND TIME BASE {}, XTAL REF DIV {}",
        Opt(v.field(DcI2cDdc1Setup, 0, DC_I2C_DDC1_ENABLE)), Opt(v.field(DcI2cDdc1Setup, 0, DC_I2C_DDC1_TIME_LIMIT)),
        Opt(v.field(DcI2cDdc1Speed, 0, DC_I2C_DDC1_PRESCALE)), Opt(v.field(DcI2cDdc1Speed, 0, DC_I2C_DDC1_THRESHOLD)),
        Opt(v.field(MicrosecondTimeBaseDiv, 0, MICROSECOND_TIME_BASE)), Opt(v.field(MicrosecondTimeBaseDiv, 0, XTAL_REF_DIV))));
    let pad = |r: R, f: Field| Opt(v.field(r, 0, f));
    out.line(format_args!("B11 DDC1 PADS: AUX MODE {}; CLOCK MASK {} EN {} A {} Y {}; DATA MASK {} EN {} A {} Y {}",
        pad(DcGpioDdc1Mask, AUX_PAD1_MODE), pad(DcGpioDdc1Mask, DC_GPIO_DDC1CLK_MASK), pad(DcGpioDdc1En, DC_GPIO_DDC1CLK_BIT),
        pad(DcGpioDdc1A, DC_GPIO_DDC1CLK_BIT), pad(DcGpioDdc1Y, DC_GPIO_DDC1CLK_BIT), pad(DcGpioDdc1Mask, DC_GPIO_DDC1DATA_MASK),
        pad(DcGpioDdc1En, DC_GPIO_DDC1DATA_BIT), pad(DcGpioDdc1A, DC_GPIO_DDC1DATA_BIT), pad(DcGpioDdc1Y, DC_GPIO_DDC1DATA_BIT)));
}

/// Batch 12's values (213-DRV-0112) as notes: each register's raw value with the fields steps 2 to 4 would take.
fn summarize_second(v: &Values, out: &mut dyn Out) {
    use R::*;
    let g = |r: R, i: u8, f: Field| Opt(v.field(r, i, f));
    let x = |r: R, i: u8| Hex32(v.get(r, i));
    out.note(format_args!("B12 HDMI-A-2 HPD 2 (HPD1_): SENSE {}, ENABLED {}; INT_STATUS {}, CONTROL {}",
        g(HpdIntStatus, 1, DC_HPD_SENSE), g(HpdControl, 1, DC_HPD_EN), x(HpdIntStatus, 1), x(HpdControl, 1)));
    out.note(format_args!("B12 HDMI-A-2 DIG B: FE FROM OTG {}, SYMCLK_FE_ON {} ({}); BE ENABLED {}, MODE {}, FROM FE MASK {}, SYMCLK_BE_ON {} ({}, {})",
        g(DigFeCntl, 1, DIG_SOURCE_SELECT), g(DigFeCntl, 1, DIG_SYMCLK_FE_ON), x(DigFeCntl, 1), g(DigBeEnCntl, 1, DIG_ENABLE),
        g(DigBeCntl, 1, DIG_MODE), g(DigBeCntl, 1, DIG_FE_SOURCE_SELECT), g(DigBeEnCntl, 1, DIG_SYMCLK_BE_ON), x(DigBeCntl, 1), x(DigBeEnCntl, 1)));
    out.note(format_args!("B12 HDMI-A-2 PHYPLLB: PIXCLK_ENABLE {}, DEEP COLOR {} ({})",
        g(PhypllPixclkResyncCntl, 1, PHYPLL_PIXCLK_ENABLE), g(PhypllPixclkResyncCntl, 1, PHYPLL_DEEP_COLOR_CNTL), x(PhypllPixclkResyncCntl, 1)));
    out.note(format_args!("B12 HDMI-A-2 DDC2: HW STATUS {} ({}); ENABLED {}, TIME LIMIT {}, PRESCALE {}, THRESHOLD {} (SPEED {}, SETUP {})",
        g(DcI2cDdc2HwStatus, 0, DC_I2C_DDC1_HW_STATUS_BITS), x(DcI2cDdc2HwStatus, 0), g(DcI2cDdc2Setup, 0, DC_I2C_DDC1_ENABLE),
        g(DcI2cDdc2Setup, 0, DC_I2C_DDC1_TIME_LIMIT), g(DcI2cDdc2Speed, 0, DC_I2C_DDC1_PRESCALE), g(DcI2cDdc2Speed, 0, DC_I2C_DDC1_THRESHOLD),
        x(DcI2cDdc2Speed, 0), x(DcI2cDdc2Setup, 0)));
    let pad = |r: R, f: Field| g(r, 0, f);
    out.note(format_args!("B12 HDMI-A-2 DDC2 PADS: AUX MODE {}; CLOCK MASK {} EN {} A {} Y {}; DATA MASK {} EN {} A {} Y {} (MASK {})",
        pad(DcGpioDdc2Mask, AUX_PAD1_MODE), pad(DcGpioDdc2Mask, DC_GPIO_DDC1CLK_MASK), pad(DcGpioDdc2En, DC_GPIO_DDC1CLK_BIT),
        pad(DcGpioDdc2A, DC_GPIO_DDC1CLK_BIT), pad(DcGpioDdc2Y, DC_GPIO_DDC1CLK_BIT), pad(DcGpioDdc2Mask, DC_GPIO_DDC1DATA_MASK),
        pad(DcGpioDdc2En, DC_GPIO_DDC1DATA_BIT), pad(DcGpioDdc2A, DC_GPIO_DDC1DATA_BIT), pad(DcGpioDdc2Y, DC_GPIO_DDC1DATA_BIT), x(DcGpioDdc2Mask, 0)));
    out.note(format_args!("B12 THE DDC PADS' I2C MODE: DDC1 {} DDC2 {}; AUX PAD RXSEL 1:{} 2:{} (DC_GPIO_AUX_CTRL_5 {}, PHY_AUX_CNTL {})",
        g(DcGpioAuxCtrl5, 0, DDC_PAD1_I2CMODE), g(DcGpioAuxCtrl5, 0, DDC_PAD2_I2CMODE), g(PhyAuxCntl, 0, AUX1_PAD_RXSEL),
        g(PhyAuxCntl, 0, AUX2_PAD_RXSEL), x(DcGpioAuxCtrl5, 0), x(PhyAuxCntl, 0)));
}

fn summarize_reference(v: &Values, p: u8, t: u8, out: &mut dyn Out) {
    use R::*;
    let g = |r: R, f: Field| Opt(v.field(r, p, f));
    out.line(format_args!("B9 HUBP{}: VIEWPORT {}x{} AT {},{}, SW_MODE {}, SWATH {} PTE_ROW {} CHUNK {} MIN_CHUNK {}, CLK_CNTL {}", p,
        g(HubpPriViewportDimension, PRI_VIEWPORT_WIDTH), g(HubpPriViewportDimension, PRI_VIEWPORT_HEIGHT),
        g(HubpPriViewportStart, PRI_VIEWPORT_X_START), g(HubpPriViewportStart, PRI_VIEWPORT_Y_START), g(HubpTilingConfig, SW_MODE),
        g(HubpReqSizeConfig, SWATH_HEIGHT), g(HubpReqSizeConfig, PTE_ROW_HEIGHT_LINEAR), g(HubpReqSizeConfig, CHUNK_SIZE),
        g(HubpReqSizeConfig, MIN_CHUNK_SIZE), Hex32(v.get(HubpClkCntl, p))));
    out.line(format_args!("B9 HUBP{} DLG: REF/PIX {}, PER HTOTAL {}, H_BLANK_END {}, V_BLANK_END {}, NEXT START {}, AFTER SCALER X {} Y {}, PREFETCH {} VRATIO {}", p,
        Ratio19(v.field(HubpreqRefFreqToPixFreq, p, REF_FREQ_TO_PIX_FREQ)), g(HubpreqDstDimensions, REFCYC_PER_HTOTAL),
        g(HubpreqBlankOffset0, REFCYC_H_BLANK_END), g(HubpreqBlankOffset0, DLG_V_BLANK_END), g(HubpreqBlankOffset1, MIN_DST_Y_NEXT_START),
        g(HubpreqDstAfterScaler, REFCYC_X_AFTER_SCALER), g(HubpreqDstAfterScaler, DST_Y_AFTER_SCALER),
        g(HubpreqPrefetchSettings, DST_Y_PREFETCH), g(HubpreqPrefetchSettings, VRATIO_PREFETCH)));
    out.line(format_args!("B9 HUBP{} DELIVERY: LINE {} PRE {}, REQ {} PRE {}, QOS {}, MIN_TTU_VBLANK {}, DRQ LIMIT {}", p,
        g(HubpreqPerLineDelivery, REFCYC_PER_LINE_DELIVERY_L), g(HubpreqPerLineDeliveryPre, REFCYC_PER_LINE_DELIVERY_L),
        g(HubpreqSurf0TtuCntl0, REFCYC_PER_REQ_DELIVERY), g(HubpreqSurf0TtuCntl1, REFCYC_PER_REQ_DELIVERY),
        Hex32(v.get(HubpreqTtuQosWm, p)), g(HubpreqGlobalTtuCntl, MIN_TTU_VBLANK), g(HubpreqDstYDeltaDrqLimit, DST_Y_DELTA_DRQ_LIMIT)));
    out.line(format_args!("B9 OTG{}: VSTARTUP {}, VUPDATE {}, VREADY {}", t, Opt(v.field(OtgVstartupParam, t, VSTARTUP_START)),
        Hex32(v.get(OtgVupdateParam, t)), Hex32(v.get(OtgVreadyParam, t))));
}

fn summarize_path(v: &Values, p: u8, out: &mut dyn Out) {
    use R::*;
    let g = |r: R, f: Field| Opt(v.field(r, p, f));
    out.line(format_args!("B10 DPP{}: CLOCK {}, FORMAT {}, CNVC {}, DSCL_MODE {}, RECOUT {}x{} AT {},{}, MPC {}x{}, CM BYPASS {}, ICSC {} GAMUT {} DGAM {} BLNDGAM {}", p,
        g(DppControl, DPP_CLOCK_ENABLE), g(CnvcSurfacePixelFormat, CNVC_SURFACE_PIXEL_FORMAT), Hex32(v.get(CnvcFormatControl, p)),
        g(DsclSclMode, DSCL_MODE), g(DsclRecoutSize, LOW14), g(DsclRecoutSize, HIGH14), g(DsclRecoutStart, RECOUT_START_X),
        g(DsclRecoutStart, RECOUT_START_Y), g(DsclMpcSize, LOW14), g(DsclMpcSize, HIGH14), g(CmControl, CM_BYPASS),
        Opt(v.get(CmIcscControl, p)), Opt(v.get(CmGamutRemapControl, p)), Opt(v.get(CmDgamControl, p)), Opt(v.get(CmBlndgamControl, p))));
    let m = |r: R, n: u8| Opt(v.field(r, n, MPCC_SEL));
    for pair in [0u8, 2] {
        out.line(format_args!("B10 MPC: MPCC{} TOP {} BOT {} OPP {} MODE {} ALPHA {}; MPCC{} TOP {} BOT {} OPP {} MODE {} ALPHA {}",
            pair, m(MpccTopSel, pair), m(MpccBotSel, pair), m(MpccOppId, pair), Opt(v.field(MpccControl, pair, MPCC_MODE)),
            Opt(v.field(MpccControl, pair, MPCC_GLOBAL_ALPHA)), pair + 1, m(MpccTopSel, pair + 1), m(MpccBotSel, pair + 1),
            m(MpccOppId, pair + 1), Opt(v.field(MpccControl, pair + 1, MPCC_MODE)), Opt(v.field(MpccControl, pair + 1, MPCC_GLOBAL_ALPHA))));
    }
    out.line(format_args!("B10 MPC_OUT MUX: 0:{} 1:{} 2:{} 3:{} (15: NONE)", m(MpcOutMux, 0), m(MpcOutMux, 1), m(MpcOutMux, 2), m(MpcOutMux, 3)));
    let w = |i: u8| Opt(v.field(DchubbubArbWatermark, i, WATERMARK_14));
    for set in [0u8, 2] {
        let (a, b) = (5 * set, 5 * (set + 1));
        out.line(format_args!("B10 WATERMARKS {}: URGENT {} TRIP {} SR_ENTER {} SR_EXIT {} DRAM {}; {}: URGENT {} TRIP {} SR_ENTER {} SR_EXIT {} DRAM {}",
            ['A', 'B', 'C', 'D'][set as usize], w(a), w(a + 1), w(a + 2), w(a + 3), w(a + 4),
            ['A', 'B', 'C', 'D'][set as usize + 1], w(b), w(b + 1), w(b + 2), w(b + 3), w(b + 4)));
    }
    out.line(format_args!("B10 DCHUBBUB: SAT {}, DF_REQ {}, DRAM_STATE {}, CHANGE {}, TIMER {}", Hex32(v.get(DchubbubArbSatLevel, 0)),
        Hex32(v.get(DchubbubArbDfReqOutstand, 0)), Hex32(v.get(DchubbubArbDramStateCntl, 0)),
        Hex32(v.get(DchubbubArbWatermarkChangeCntl, 0)), Hex32(v.get(DchubbubGlobalTimerCntl, 0))));
    let d = |n: u8| (Opt(v.field(DppclkDtoParam, n, DTO_PHASE)), Opt(v.field(DppclkDtoParam, n, DTO_MODULO)));
    let (d0, d1, d2, d3) = (d(0), d(1), d(2), d(3));
    out.line(format_args!("B10 DCCG: DPPCLK DTO CTRL {}; DTO0 {}/{} DTO1 {}/{} DTO2 {}/{} DTO3 {}/{}; REFCLK_CNTL {}",
        Hex32(v.get(DppclkDtoCtrl, 0)), d0.0, d0.1, d1.0, d1.1, d2.0, d2.1, d3.0, d3.1, Hex32(v.get(RefclkCntl, 0))));
}

fn summarize_otg(v: &Values, n: u8, out: &mut dyn Out) {
    use R::*;
    let mut regs = [(OtgHTotal, 0u32); 8];
    let wanted = [OtgHTotal, OtgHBlankStartEnd, OtgHSyncA, OtgVTotal, OtgVBlankStartEnd, OtgVSyncA, OtgHSyncACntl, OtgVSyncACntl];
    let mut complete = true;
    for (slot, r) in regs.iter_mut().zip(wanted) {
        match v.get(r, n) { Some(x) => *slot = (r, x), None => complete = false }
    }
    let state = Opt(v.field(OtgControl, n, OTG_CURRENT_MASTER_EN_STATE));
    let clock = Opt(v.field(OtgClockControl, n, OTG_CLOCK_ON));
    let opp = Opt(v.field(OptcDataSourceSelect, n, OPTC_SEG0_SRC_SEL));
    let sync_start = |r: R| v.field(r, n, COUNTER_LOW).unwrap_or(0);
    let timing = if complete { otg_state(&regs).and_then(|o| o.timing(0)) } else { None };
    match timing {
        Some(t) => out.line(format_args!("B2 OTG{}: RUNNING {}, CLOCK_ON {}, FROM OPP {}; {}x{} TOTAL {}x{} H{} V{}{}", n, state, clock, opp,
            t.h_active, t.v_active, t.h_total(), t.v_total(), if t.h_sync_positive { '+' } else { '-' },
            if t.v_sync_positive { '+' } else { '-' },
            if sync_start(OtgHSyncA) != 0 || sync_start(OtgVSyncA) != 0 { ", SYNC NOT FROM 0" } else { "" })),
        None => out.line(format_args!("B2 OTG{}: RUNNING {}, CLOCK_ON {}, FROM OPP {}; NO TIMING IN ITS REGISTERS", n, state, clock, opp)),
    }
}
