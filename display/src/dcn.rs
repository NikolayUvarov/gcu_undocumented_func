//! The Cezanne Radeon's display engine, DCN 2.1, as `display` drives it (213-DRV-0086): where its registers are in
//! BAR 5, the fields first light writes, the VBIOS command tables it calls, and the order of it all, as data that a
//! host test checks against a model of the blocks. Nothing here touches hardware.
//!
//! Offsets, fields and parameter layouts are facts from Linux v7.0's AMD headers (`renoir_ip_offset.h`,
//! `asic_reg/dcn/dcn_2_1_0_offset.h`, `dcn_2_1_0_sh_mask.h`, `atomfirmware.h`; MIT); the order follows what Display
//! Core does for a TMDS stream (`dcn20_enable_stream_timing`, `optc1_program_timing`, `optc2_enable_crtc`,
//! `opp2_set_disp_pattern_generator`, the DIG encoders). hwdocs/cezanne-display.md has the details; no code is taken.

use crate::timing::{Otg, Timing, TimingError, DVI_SINGLE_LINK_KHZ, TMDS_NO_SCRAMBLING_KHZ};

use core::ops::Range;

/// Dword bases of the engine's register segments 0, 1 and 2 in BAR 5 (DCN_BASE).
pub const SEGMENT: [u32; 3] = [0x12, 0xC0, 0x34C0];
/// The bytes of BAR 5 that hold the engine's registers (segments 0 to 2: DCN 2.1's blocks and its DPCS).
pub const DCN_BYTES: Range<u32> = 0x48..0x1B000;
/// The SMU's display-clock mailbox, `MP1_SMN_C2PMSG_67`, `_83` and `_91` (MP1 base 0x16000 dwords): message, argument,
/// response. Not used by first light; the only bytes outside the engine `display` may name.
pub const SMU_MAILBOX: [u32; 3] = [0x58A0C, 0x58A4C, 0x58A6C];
/// The bytes of BAR 5 that `display` never touches: the device's MSI-X table (+0x42000) and pending bits (+0x43000),
/// where a write is kept by the device (KRN's rule for the grant).
pub const MSIX_BYTES: Range<u32> = 0x42000..0x44000;
/// BAR 5's size on this device.
pub const BAR5_BYTES: u32 = 0x80000;
/// Timing generators, OPPs and pipes; DIG encoders and PHYs (A to E).
pub const PIPES: u8 = 4;
pub const DIGS: u8 = 5;

// No window may reach into the MSI-X bytes, whatever the windows become.
const _: () = assert!(DCN_BYTES.end <= MSIX_BYTES.start || DCN_BYTES.start >= MSIX_BYTES.end);
const _: () = {
    let mut i = 0;
    while i < SMU_MAILBOX.len() {
        assert!(SMU_MAILBOX[i] + 4 <= MSIX_BYTES.start || SMU_MAILBOX[i] >= MSIX_BYTES.end);
        assert!(SMU_MAILBOX[i] + 4 <= BAR5_BYTES);
        i += 1;
    }
};

/// Why an offset is not a register `display` may touch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegError {
    /// In the MSI-X table or pending bits: refused before anything else.
    MsiX,
    /// Not a multiple of four.
    Unaligned,
    /// Outside the engine's bytes and the SMU mailbox.
    Outside,
}

/// A register `display` may touch, as its byte offset in BAR 5. The only ways to make one are `Reg::new` and `R::at`,
/// which goes through it, so no `Reg` names a byte of the MSI-X range or one outside the windows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Reg(u32);

impl Reg {
    /// The register at `offset`, if `display` may touch it.
    pub const fn new(offset: u32) -> Result<Reg, RegError> {
        if offset >= MSIX_BYTES.start && offset < MSIX_BYTES.end { return Err(RegError::MsiX); }
        if offset % 4 != 0 { return Err(RegError::Unaligned); }
        let in_engine = offset >= DCN_BYTES.start && offset <= DCN_BYTES.end - 4;
        let in_mailbox = offset == SMU_MAILBOX[0] || offset == SMU_MAILBOX[1] || offset == SMU_MAILBOX[2];
        if in_engine || in_mailbox { Ok(Reg(offset)) } else { Err(RegError::Outside) }
    }
    /// Its byte offset in BAR 5.
    pub const fn offset(self) -> u32 { self.0 }
    /// Its dword index in BAR 5.
    pub const fn dword(self) -> usize { (self.0 / 4) as usize }
}

/// The registers first light reads and writes, each of one instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum R {
    OtgHTotal, OtgHBlankStartEnd, OtgHSyncA, OtgHSyncACntl, OtgHTimingCntl, OtgVTotal, OtgVTotalMin, OtgVTotalMax,
    OtgVBlankStartEnd, OtgVSyncA, OtgVSyncACntl, OtgControl, OtgBlankControl, OtgInterlaceControl, OtgStatus,
    OtgStatusPosition, OtgStatusFrameCount, OtgClockControl, OtgVstartupParam, OtgVupdateParam, OtgVreadyParam,
    OptcDataSourceSelect, OptcDataFormatControl, OptcInputClockControl, VtgControl,
    OppPipeControl, DpgControl, DpgDimensions, DpgOffsetSegment,
    DigFeCntl, DigBeCntl, OtgPixelRateCntl, OtgPhypllPixelRateCntl, PhypllPixclkResyncCntl,
    HpdIntStatus, HpdControl, DentistDispclkCntl,
    // Read by step 1 only (213-DRV-0086): the rest of what `optc1_read_otg_state` reads, power, HUBP, DIG, aperture.
    OtgVTotalControl, OtgVerticalInterrupt1Position, OtgVerticalInterrupt1Control, OtgVerticalInterrupt2Position,
    OtgVerticalInterrupt2Control, OptcInputGlobalControl, DomainPgStatus,
    HubpSurfaceConfig, HubpCntl, HubpreqSurfacePitch, HubpreqPrimarySurfaceAddress, HubpreqPrimarySurfaceAddressHigh,
    DigBeEnCntl, DcnVmFbLocationBase, DcnVmFbLocationTop, DcnVmFbOffset, DcnVmAgpBot, DcnVmAgpTop, DcnVmAgpBase,
    // Read by step 1's batch 8 only: whether the display microcontroller (DMCUB) runs, as Linux's dmub code tells it.
    CcDcPipeDis, DmcubCntl, DmcubSecCntl, DmcubScratch, DmcubInbox0Wptr, DmcubInbox0Rptr, DmcubInbox1BaseAddress,
    DmcubInbox1Size, DmcubInbox1Wptr, DmcubInbox1Rptr, DmcubOutbox1Wptr, DmcubOutbox1Rptr, DmcubGpintDatain1,
    DmcubGpintDataout, DmcubTimerCurrent,
    // Read by step 1's batches 9 and 10 (213-DRV-0092): the firmware's pipe as step 4's reference; written for pipe 1 by
    // stages 11 to 15.
    HubpAddrConfig, HubpTilingConfig, HubpPriViewportStart, HubpPriViewportDimension, HubpReqSizeConfig, HubpReqSizeConfigC,
    HubpClkCntl, HubpreqSurfaceControl, HubpreqExpansionMode, HubpreqTtuQosWm, HubpreqGlobalTtuCntl, HubpreqSurf0TtuCntl0,
    HubpreqSurf0TtuCntl1, HubpreqCur0TtuCntl0, HubpreqCur0TtuCntl1, HubpreqVmMxL1TlbCntl, HubpreqBlankOffset0, HubpreqBlankOffset1,
    HubpreqDstDimensions, HubpreqDstAfterScaler, HubpreqPrefetchSettings, HubpreqVblankParameters0, HubpreqVblankParameters1, HubpreqVblankParameters2,
    HubpreqVblankParameters3, HubpreqVblankParameters4, HubpreqNomParameters0, HubpreqNomParameters1, HubpreqNomParameters4, HubpreqNomParameters5,
    HubpreqPerLineDeliveryPre, HubpreqPerLineDelivery, HubpreqCursorSettings, HubpreqRefFreqToPixFreq, HubpreqDstYDeltaDrqLimit, HubpretControl,
    DppControl, CnvcSurfacePixelFormat, CnvcFormatControl, DsclSclMode, DsclControl, DsclRecoutStart,
    DsclRecoutSize, DsclMpcSize, DsclLbDataFormat, CmControl, CmIcscControl, CmGamutRemapControl,
    CmDgamControl, CmBlndgamControl, MpccTopSel, MpccBotSel, MpccOppId, MpccControl,
    MpcOutMux, DchubbubArbWatermark, DchubbubArbDfReqOutstand, DchubbubArbSatLevel, DchubbubArbDramStateCntl, DchubbubArbWatermarkChangeCntl,
    DchubbubGlobalTimerCntl, DppclkDtoCtrl, DppclkDtoParam, RefclkCntl,
    HubpreqVmidSettings0, HubpreqFlipControl, HubpreqVmSystemApertureLow, HubpreqVmSystemApertureHigh, DsclTapControl, DsclHorzScaleRatio, DsclVertScaleRatio,
}

/// Every register of the map, for walks over all of them.
pub const ALL: [R; 142] = {
    use R::*;
    [OtgHTotal, OtgHBlankStartEnd, OtgHSyncA, OtgHSyncACntl, OtgHTimingCntl, OtgVTotal, OtgVTotalMin, OtgVTotalMax,
     OtgVBlankStartEnd, OtgVSyncA, OtgVSyncACntl, OtgControl, OtgBlankControl, OtgInterlaceControl, OtgStatus,
     OtgStatusPosition, OtgStatusFrameCount, OtgClockControl, OtgVstartupParam, OtgVupdateParam, OtgVreadyParam,
     OptcDataSourceSelect, OptcDataFormatControl, OptcInputClockControl, VtgControl,
     OppPipeControl, DpgControl, DpgDimensions, DpgOffsetSegment,
     DigFeCntl, DigBeCntl, OtgPixelRateCntl, OtgPhypllPixelRateCntl, PhypllPixclkResyncCntl,
     HpdIntStatus, HpdControl, DentistDispclkCntl,
     OtgVTotalControl, OtgVerticalInterrupt1Position, OtgVerticalInterrupt1Control, OtgVerticalInterrupt2Position,
     OtgVerticalInterrupt2Control, OptcInputGlobalControl, DomainPgStatus,
     HubpSurfaceConfig, HubpCntl, HubpreqSurfacePitch, HubpreqPrimarySurfaceAddress, HubpreqPrimarySurfaceAddressHigh,
     DigBeEnCntl, DcnVmFbLocationBase, DcnVmFbLocationTop, DcnVmFbOffset, DcnVmAgpBot, DcnVmAgpTop, DcnVmAgpBase,
     CcDcPipeDis, DmcubCntl, DmcubSecCntl, DmcubScratch, DmcubInbox0Wptr, DmcubInbox0Rptr, DmcubInbox1BaseAddress,
     DmcubInbox1Size, DmcubInbox1Wptr, DmcubInbox1Rptr, DmcubOutbox1Wptr, DmcubOutbox1Rptr, DmcubGpintDatain1,
     DmcubGpintDataout, DmcubTimerCurrent,
     HubpAddrConfig, HubpTilingConfig, HubpPriViewportStart, HubpPriViewportDimension, HubpReqSizeConfig, HubpReqSizeConfigC,
     HubpClkCntl, HubpreqSurfaceControl, HubpreqExpansionMode, HubpreqTtuQosWm, HubpreqGlobalTtuCntl, HubpreqSurf0TtuCntl0,
     HubpreqSurf0TtuCntl1, HubpreqCur0TtuCntl0, HubpreqCur0TtuCntl1, HubpreqVmMxL1TlbCntl, HubpreqBlankOffset0, HubpreqBlankOffset1,
     HubpreqDstDimensions, HubpreqDstAfterScaler, HubpreqPrefetchSettings, HubpreqVblankParameters0, HubpreqVblankParameters1, HubpreqVblankParameters2,
     HubpreqVblankParameters3, HubpreqVblankParameters4, HubpreqNomParameters0, HubpreqNomParameters1, HubpreqNomParameters4, HubpreqNomParameters5,
     HubpreqPerLineDeliveryPre, HubpreqPerLineDelivery, HubpreqCursorSettings, HubpreqRefFreqToPixFreq, HubpreqDstYDeltaDrqLimit, HubpretControl,
     DppControl, CnvcSurfacePixelFormat, CnvcFormatControl, DsclSclMode, DsclControl, DsclRecoutStart,
     DsclRecoutSize, DsclMpcSize, DsclLbDataFormat, CmControl, CmIcscControl, CmGamutRemapControl,
     CmDgamControl, CmBlndgamControl, MpccTopSel, MpccBotSel, MpccOppId, MpccControl,
     MpcOutMux, DchubbubArbWatermark, DchubbubArbDfReqOutstand, DchubbubArbSatLevel, DchubbubArbDramStateCntl, DchubbubArbWatermarkChangeCntl,
     DchubbubGlobalTimerCntl, DppclkDtoCtrl, DppclkDtoParam, RefclkCntl,
     HubpreqVmidSettings0, HubpreqFlipControl, HubpreqVmSystemApertureLow, HubpreqVmSystemApertureHigh, DsclTapControl, DsclHorzScaleRatio, DsclVertScaleRatio]
};

impl R {
    /// How many instances the block has: pipes for OTG/ODM/VTG/OPP/DPG/pixel rate, encoders for DIG, PHY PLL and HPD.
    pub const fn instances(self) -> u8 {
        use R::*;
        match self {
            DigFeCntl | DigBeCntl | DigBeEnCntl | PhypllPixclkResyncCntl | HpdIntStatus | HpdControl => DIGS,
            DomainPgStatus => 2 * PIPES, // HUBP n is domain 2n, DPP n is 2n + 1 (dcn20_hubp_pg_control)
            DentistDispclkCntl | DcnVmFbLocationBase | DcnVmFbLocationTop | DcnVmFbOffset | DcnVmAgpBot | DcnVmAgpTop
                | DcnVmAgpBase => 1,
            DmcubScratch => 16,
            DchubbubArbWatermark => 20,
            DchubbubArbDfReqOutstand => 1,
            DchubbubArbSatLevel => 1,
            DchubbubArbDramStateCntl => 1,
            DchubbubArbWatermarkChangeCntl => 1,
            DchubbubGlobalTimerCntl => 1,
            DppclkDtoCtrl => 1,
            RefclkCntl => 1,
            CcDcPipeDis | DmcubCntl | DmcubSecCntl | DmcubInbox0Wptr | DmcubInbox0Rptr | DmcubInbox1BaseAddress | DmcubInbox1Size
                | DmcubInbox1Wptr | DmcubInbox1Rptr | DmcubOutbox1Wptr | DmcubOutbox1Rptr | DmcubGpintDatain1 | DmcubGpintDataout
                | DmcubTimerCurrent => 1,
            _ => PIPES,
        }
    }

    /// The register of instance `inst`; None beyond the block's instances (or, by construction, outside the windows).
    pub fn at(self, inst: u8) -> Option<Reg> {
        use R::*;
        if inst >= self.instances() { return None; }
        let i = inst as u32;
        let otg = |off: u32| (2, 0x1B2A + off + 0x80 * i);
        let (segment, dword) = match self {
            OtgHTotal => otg(0x00), OtgHBlankStartEnd => otg(0x01), OtgHSyncA => otg(0x02), OtgHSyncACntl => otg(0x03),
            OtgHTimingCntl => otg(0x04), OtgVTotal => otg(0x05), OtgVTotalMin => otg(0x06), OtgVTotalMax => otg(0x07),
            OtgVBlankStartEnd => otg(0x0C), OtgVSyncA => otg(0x0D), OtgVSyncACntl => otg(0x0E), OtgControl => otg(0x17),
            OtgBlankControl => otg(0x18), OtgInterlaceControl => otg(0x1A), OtgStatus => otg(0x1F),
            OtgStatusPosition => otg(0x20), OtgStatusFrameCount => otg(0x22), OtgClockControl => otg(0x5C),
            OtgVstartupParam => otg(0x5D), OtgVupdateParam => otg(0x5E), OtgVreadyParam => otg(0x5F),
            OptcDataSourceSelect => (2, 0x1ACB + 0x10 * i), OptcDataFormatControl => (2, 0x1ACC + 0x10 * i),
            OptcInputClockControl => (2, 0x1ACF + 0x10 * i), VtgControl => (2, 0x0528 + i),
            OppPipeControl => (2, 0x188C + 0x5A * i), DpgControl => (2, 0x1854 + 0x5A * i),
            DpgDimensions => (2, 0x1856 + 0x5A * i), DpgOffsetSegment => (2, 0x185A + 0x5A * i),
            DigFeCntl => (2, 0x2068 + 0x100 * i), DigBeCntl => (2, 0x20AF + 0x100 * i),
            OtgPixelRateCntl => (1, 0x80 + 4 * i), OtgPhypllPixelRateCntl => (1, 0x83 + 4 * i),
            PhypllPixclkResyncCntl => (1, if i < 4 { 0x40 + i } else { 0x4C }),
            HpdIntStatus => (2, 0x1F14 + 8 * i), HpdControl => (2, 0x1F16 + 8 * i),
            DentistDispclkCntl => (1, 0x64),
            OtgVTotalControl => otg(0x09), OtgVerticalInterrupt1Position => otg(0x3A),
            OtgVerticalInterrupt1Control => otg(0x3B), OtgVerticalInterrupt2Position => otg(0x3C),
            OtgVerticalInterrupt2Control => otg(0x3D), OptcInputGlobalControl => (2, 0x1ACA + 0x10 * i),
            DomainPgStatus => (2, 0x81 + 2 * i),
            HubpSurfaceConfig => (2, 0x5E5 + 0xDC * i), HubpCntl => (2, 0x5F3 + 0xDC * i),
            HubpreqSurfacePitch => (2, 0x607 + 0xDC * i), HubpreqPrimarySurfaceAddress => (2, 0x60A + 0xDC * i),
            HubpreqPrimarySurfaceAddressHigh => (2, 0x60B + 0xDC * i),
            DigBeEnCntl => (2, 0x20B0 + 0x100 * i),
            DcnVmFbLocationBase => (2, 0x493), DcnVmFbLocationTop => (2, 0x494), DcnVmFbOffset => (2, 0x495),
            DcnVmAgpBot => (2, 0x496), DcnVmAgpTop => (2, 0x497), DcnVmAgpBase => (2, 0x498),
            CcDcPipeDis => (2, 0xCA), DmcubSecCntl => (2, 0x3278), DmcubInbox0Wptr => (2, 0x327C), DmcubInbox0Rptr => (2, 0x327D),
            DmcubInbox1BaseAddress => (2, 0x327E), DmcubInbox1Size => (2, 0x327F), DmcubInbox1Wptr => (2, 0x3280),
            DmcubInbox1Rptr => (2, 0x3281), DmcubOutbox1Wptr => (2, 0x3288), DmcubOutbox1Rptr => (2, 0x3289),
            DmcubScratch => (2, 0x328D + i), DmcubCntl => (2, 0x32A0), DmcubGpintDatain1 => (2, 0x32A2),
            DmcubGpintDataout => (2, 0x32A3), DmcubTimerCurrent => (2, 0x32A7),
            HubpAddrConfig => (2, 0x5E6 + 0xDC * i),
            HubpTilingConfig => (2, 0x5E7 + 0xDC * i),
            HubpPriViewportStart => (2, 0x5E9 + 0xDC * i),
            HubpPriViewportDimension => (2, 0x5EA + 0xDC * i),
            HubpReqSizeConfig => (2, 0x5F1 + 0xDC * i),
            HubpReqSizeConfigC => (2, 0x5F2 + 0xDC * i),
            HubpClkCntl => (2, 0x5F4 + 0xDC * i),
            HubpreqSurfaceControl => (2, 0x61A + 0xDC * i),
            HubpreqExpansionMode => (2, 0x62C + 0xDC * i),
            HubpreqTtuQosWm => (2, 0x62D + 0xDC * i),
            HubpreqGlobalTtuCntl => (2, 0x62E + 0xDC * i),
            HubpreqSurf0TtuCntl0 => (2, 0x62F + 0xDC * i),
            HubpreqSurf0TtuCntl1 => (2, 0x630 + 0xDC * i),
            HubpreqCur0TtuCntl0 => (2, 0x633 + 0xDC * i),
            HubpreqCur0TtuCntl1 => (2, 0x634 + 0xDC * i),
            HubpreqVmMxL1TlbCntl => (2, 0x645 + 0xDC * i),
            HubpreqBlankOffset0 => (2, 0x646 + 0xDC * i),
            HubpreqBlankOffset1 => (2, 0x647 + 0xDC * i),
            HubpreqDstDimensions => (2, 0x648 + 0xDC * i),
            HubpreqDstAfterScaler => (2, 0x649 + 0xDC * i),
            HubpreqPrefetchSettings => (2, 0x64A + 0xDC * i),
            HubpreqVblankParameters0 => (2, 0x64C + 0xDC * i),
            HubpreqVblankParameters1 => (2, 0x64D + 0xDC * i),
            HubpreqVblankParameters2 => (2, 0x64E + 0xDC * i),
            HubpreqVblankParameters3 => (2, 0x64F + 0xDC * i),
            HubpreqVblankParameters4 => (2, 0x650 + 0xDC * i),
            HubpreqNomParameters0 => (2, 0x654 + 0xDC * i),
            HubpreqNomParameters1 => (2, 0x655 + 0xDC * i),
            HubpreqNomParameters4 => (2, 0x658 + 0xDC * i),
            HubpreqNomParameters5 => (2, 0x659 + 0xDC * i),
            HubpreqPerLineDeliveryPre => (2, 0x65C + 0xDC * i),
            HubpreqPerLineDelivery => (2, 0x65D + 0xDC * i),
            HubpreqCursorSettings => (2, 0x65E + 0xDC * i),
            HubpreqRefFreqToPixFreq => (2, 0x65F + 0xDC * i),
            HubpreqDstYDeltaDrqLimit => (2, 0x660 + 0xDC * i),
            HubpretControl => (2, 0x66C + 0xDC * i),
            DppControl => (2, 0xCC5 + 0x16B * i),
            CnvcSurfacePixelFormat => (2, 0xCCF + 0x16B * i),
            CnvcFormatControl => (2, 0xCD0 + 0x16B * i),
            DsclSclMode => (2, 0xCEC + 0x16B * i),
            DsclControl => (2, 0xCEE + 0x16B * i),
            DsclRecoutStart => (2, 0xD02 + 0x16B * i),
            DsclRecoutSize => (2, 0xD03 + 0x16B * i),
            DsclMpcSize => (2, 0xD04 + 0x16B * i),
            DsclLbDataFormat => (2, 0xD05 + 0x16B * i),
            CmControl => (2, 0xD1A + 0x16B * i),
            CmIcscControl => (2, 0xD1B + 0x16B * i),
            CmGamutRemapControl => (2, 0xD28 + 0x16B * i),
            CmDgamControl => (2, 0xD37 + 0x16B * i),
            CmBlndgamControl => (2, 0xD63 + 0x16B * i),
            MpccTopSel => (2, 0x1271 + 0x1B * i),
            MpccBotSel => (2, 0x1272 + 0x1B * i),
            MpccOppId => (2, 0x1273 + 0x1B * i),
            MpccControl => (2, 0x1274 + 0x1B * i),
            MpcOutMux => (2, 0x1385 + 0x4 * i),
            DchubbubArbWatermark => (2, 0x509 + i),
            DchubbubArbDfReqOutstand => (2, 0x505),
            DchubbubArbSatLevel => (2, 0x506),
            DchubbubArbDramStateCntl => (2, 0x508),
            DchubbubArbWatermarkChangeCntl => (2, 0x51D),
            DchubbubGlobalTimerCntl => (2, 0x51F),
            DppclkDtoCtrl => (1, 0xB6),
            DppclkDtoParam => (1, 0x99 + i),
            RefclkCntl => (1, 0x49),
            HubpreqVmidSettings0 => (2, 0x609 + 0xDC * i),
            HubpreqFlipControl => (2, 0x61B + 0xDC * i),
            HubpreqVmSystemApertureLow => (2, 0x637 + 0xDC * i),
            HubpreqVmSystemApertureHigh => (2, 0x638 + 0xDC * i),
            DsclTapControl => (2, 0xCED + 0x16B * i),
            DsclHorzScaleRatio => (2, 0xCF1 + 0x16B * i),
            DsclVertScaleRatio => (2, 0xCF5 + 0x16B * i),
        };
        Reg::new((SEGMENT[segment] + dword) * 4).ok()
    }
}

impl R {
    /// The register's name in `dcn_2_1_0_offset.h`, without its block's instance prefix (OTG0_, HUBPREQ1_, ...).
    pub const fn name(self) -> &'static str {
        use R::*;
        match self {
            OtgHTotal => "OTG_H_TOTAL", OtgHBlankStartEnd => "OTG_H_BLANK_START_END", OtgHSyncA => "OTG_H_SYNC_A",
            OtgHSyncACntl => "OTG_H_SYNC_A_CNTL", OtgHTimingCntl => "OTG_H_TIMING_CNTL", OtgVTotal => "OTG_V_TOTAL",
            OtgVTotalMin => "OTG_V_TOTAL_MIN", OtgVTotalMax => "OTG_V_TOTAL_MAX", OtgVBlankStartEnd => "OTG_V_BLANK_START_END",
            OtgVSyncA => "OTG_V_SYNC_A", OtgVSyncACntl => "OTG_V_SYNC_A_CNTL", OtgControl => "OTG_CONTROL",
            OtgBlankControl => "OTG_BLANK_CONTROL", OtgInterlaceControl => "OTG_INTERLACE_CONTROL", OtgStatus => "OTG_STATUS",
            OtgStatusPosition => "OTG_STATUS_POSITION", OtgStatusFrameCount => "OTG_STATUS_FRAME_COUNT",
            OtgClockControl => "OTG_CLOCK_CONTROL", OtgVstartupParam => "OTG_VSTARTUP_PARAM", OtgVupdateParam => "OTG_VUPDATE_PARAM",
            OtgVreadyParam => "OTG_VREADY_PARAM", OptcDataSourceSelect => "OPTC_DATA_SOURCE_SELECT",
            OptcDataFormatControl => "OPTC_DATA_FORMAT_CONTROL", OptcInputClockControl => "OPTC_INPUT_CLOCK_CONTROL",
            VtgControl => "VTG_CONTROL", OppPipeControl => "OPP_PIPE_CONTROL", DpgControl => "DPG_CONTROL",
            DpgDimensions => "DPG_DIMENSIONS", DpgOffsetSegment => "DPG_OFFSET_SEGMENT", DigFeCntl => "DIG_FE_CNTL",
            DigBeCntl => "DIG_BE_CNTL", OtgPixelRateCntl => "PIXEL_RATE_CNTL", OtgPhypllPixelRateCntl => "PHYPLL_PIXEL_RATE_CNTL",
            PhypllPixclkResyncCntl => "PIXCLK_RESYNC_CNTL", HpdIntStatus => "DC_HPD_INT_STATUS", HpdControl => "DC_HPD_CONTROL",
            DentistDispclkCntl => "DENTIST_DISPCLK_CNTL", OtgVTotalControl => "OTG_V_TOTAL_CONTROL",
            OtgVerticalInterrupt1Position => "OTG_VERTICAL_INTERRUPT1_POSITION",
            OtgVerticalInterrupt1Control => "OTG_VERTICAL_INTERRUPT1_CONTROL",
            OtgVerticalInterrupt2Position => "OTG_VERTICAL_INTERRUPT2_POSITION",
            OtgVerticalInterrupt2Control => "OTG_VERTICAL_INTERRUPT2_CONTROL",
            OptcInputGlobalControl => "OPTC_INPUT_GLOBAL_CONTROL", DomainPgStatus => "PG_STATUS",
            HubpSurfaceConfig => "DCSURF_SURFACE_CONFIG", HubpCntl => "DCHUBP_CNTL", HubpreqSurfacePitch => "DCSURF_SURFACE_PITCH",
            HubpreqPrimarySurfaceAddress => "DCSURF_PRIMARY_SURFACE_ADDRESS",
            HubpreqPrimarySurfaceAddressHigh => "DCSURF_PRIMARY_SURFACE_ADDRESS_HIGH", DigBeEnCntl => "DIG_BE_EN_CNTL",
            DcnVmFbLocationBase => "DCN_VM_FB_LOCATION_BASE", DcnVmFbLocationTop => "DCN_VM_FB_LOCATION_TOP",
            DcnVmFbOffset => "DCN_VM_FB_OFFSET", DcnVmAgpBot => "DCN_VM_AGP_BOT", DcnVmAgpTop => "DCN_VM_AGP_TOP",
            DcnVmAgpBase => "DCN_VM_AGP_BASE", CcDcPipeDis => "CC_DC_PIPE_DIS", DmcubCntl => "DMCUB_CNTL",
            DmcubSecCntl => "DMCUB_SEC_CNTL", DmcubScratch => "DMCUB_SCRATCH", DmcubInbox0Wptr => "DMCUB_INBOX0_WPTR",
            DmcubInbox0Rptr => "DMCUB_INBOX0_RPTR", DmcubInbox1BaseAddress => "DMCUB_INBOX1_BASE_ADDRESS",
            DmcubInbox1Size => "DMCUB_INBOX1_SIZE", DmcubInbox1Wptr => "DMCUB_INBOX1_WPTR", DmcubInbox1Rptr => "DMCUB_INBOX1_RPTR",
            DmcubOutbox1Wptr => "DMCUB_OUTBOX1_WPTR", DmcubOutbox1Rptr => "DMCUB_OUTBOX1_RPTR",
            DmcubGpintDatain1 => "DMCUB_GPINT_DATAIN1", DmcubGpintDataout => "DMCUB_GPINT_DATAOUT",
            DmcubTimerCurrent => "DMCUB_TIMER_CURRENT",
            HubpAddrConfig => "DCSURF_ADDR_CONFIG",
            HubpTilingConfig => "DCSURF_TILING_CONFIG",
            HubpPriViewportStart => "DCSURF_PRI_VIEWPORT_START",
            HubpPriViewportDimension => "DCSURF_PRI_VIEWPORT_DIMENSION",
            HubpReqSizeConfig => "DCHUBP_REQ_SIZE_CONFIG",
            HubpReqSizeConfigC => "DCHUBP_REQ_SIZE_CONFIG_C",
            HubpClkCntl => "HUBP_CLK_CNTL",
            HubpreqSurfaceControl => "DCSURF_SURFACE_CONTROL",
            HubpreqExpansionMode => "DCN_EXPANSION_MODE",
            HubpreqTtuQosWm => "DCN_TTU_QOS_WM",
            HubpreqGlobalTtuCntl => "DCN_GLOBAL_TTU_CNTL",
            HubpreqSurf0TtuCntl0 => "DCN_SURF0_TTU_CNTL0",
            HubpreqSurf0TtuCntl1 => "DCN_SURF0_TTU_CNTL1",
            HubpreqCur0TtuCntl0 => "DCN_CUR0_TTU_CNTL0",
            HubpreqCur0TtuCntl1 => "DCN_CUR0_TTU_CNTL1",
            HubpreqVmMxL1TlbCntl => "DCN_VM_MX_L1_TLB_CNTL",
            HubpreqBlankOffset0 => "BLANK_OFFSET_0",
            HubpreqBlankOffset1 => "BLANK_OFFSET_1",
            HubpreqDstDimensions => "DST_DIMENSIONS",
            HubpreqDstAfterScaler => "DST_AFTER_SCALER",
            HubpreqPrefetchSettings => "PREFETCH_SETTINGS",
            HubpreqVblankParameters0 => "VBLANK_PARAMETERS_0",
            HubpreqVblankParameters1 => "VBLANK_PARAMETERS_1",
            HubpreqVblankParameters2 => "VBLANK_PARAMETERS_2",
            HubpreqVblankParameters3 => "VBLANK_PARAMETERS_3",
            HubpreqVblankParameters4 => "VBLANK_PARAMETERS_4",
            HubpreqNomParameters0 => "NOM_PARAMETERS_0",
            HubpreqNomParameters1 => "NOM_PARAMETERS_1",
            HubpreqNomParameters4 => "NOM_PARAMETERS_4",
            HubpreqNomParameters5 => "NOM_PARAMETERS_5",
            HubpreqPerLineDeliveryPre => "PER_LINE_DELIVERY_PRE",
            HubpreqPerLineDelivery => "PER_LINE_DELIVERY",
            HubpreqCursorSettings => "CURSOR_SETTINGS",
            HubpreqRefFreqToPixFreq => "REF_FREQ_TO_PIX_FREQ",
            HubpreqDstYDeltaDrqLimit => "DST_Y_DELTA_DRQ_LIMIT",
            HubpretControl => "HUBPRET_CONTROL",
            DppControl => "DPP_CONTROL",
            CnvcSurfacePixelFormat => "CNVC_SURFACE_PIXEL_FORMAT",
            CnvcFormatControl => "FORMAT_CONTROL",
            DsclSclMode => "SCL_MODE",
            DsclControl => "DSCL_CONTROL",
            DsclRecoutStart => "RECOUT_START",
            DsclRecoutSize => "RECOUT_SIZE",
            DsclMpcSize => "MPC_SIZE",
            DsclLbDataFormat => "LB_DATA_FORMAT",
            CmControl => "CM_CONTROL",
            CmIcscControl => "CM_ICSC_CONTROL",
            CmGamutRemapControl => "CM_GAMUT_REMAP_CONTROL",
            CmDgamControl => "CM_DGAM_CONTROL",
            CmBlndgamControl => "CM_BLNDGAM_CONTROL",
            MpccTopSel => "MPCC_TOP_SEL",
            MpccBotSel => "MPCC_BOT_SEL",
            MpccOppId => "MPCC_OPP_ID",
            MpccControl => "MPCC_CONTROL",
            MpcOutMux => "MUX",
            DchubbubArbWatermark => "DCHUBBUB_ARB_WATERMARK",
            DchubbubArbDfReqOutstand => "DCHUBBUB_ARB_DF_REQ_OUTSTAND",
            DchubbubArbSatLevel => "DCHUBBUB_ARB_SAT_LEVEL",
            DchubbubArbDramStateCntl => "DCHUBBUB_ARB_DRAM_STATE_CNTL",
            DchubbubArbWatermarkChangeCntl => "DCHUBBUB_ARB_WATERMARK_CHANGE_CNTL",
            DchubbubGlobalTimerCntl => "DCHUBBUB_GLOBAL_TIMER_CNTL",
            DppclkDtoCtrl => "DPPCLK_DTO_CTRL",
            DppclkDtoParam => "DTO_PARAM",
            RefclkCntl => "REFCLK_CNTL",
            HubpreqVmidSettings0 => "VMID_SETTINGS_0",
            HubpreqFlipControl => "DCSURF_FLIP_CONTROL",
            HubpreqVmSystemApertureLow => "DCN_VM_SYSTEM_APERTURE_LOW_ADDR",
            HubpreqVmSystemApertureHigh => "DCN_VM_SYSTEM_APERTURE_HIGH_ADDR",
            DsclTapControl => "SCL_TAP_CONTROL",
            DsclHorzScaleRatio => "SCL_HORZ_FILTER_SCALE_RATIO",
            DsclVertScaleRatio => "SCL_VERT_FILTER_SCALE_RATIO",
        }
    }

    /// The block's instance prefix, as in the header: OTG, ODM, VTG, OPP_PIPE, DPG, DIG, OTG (DCCG), PHYPLL, HPD,
    /// DOMAIN, HUBP, HUBPREQ; empty for a register of one instance.
    pub const fn block(self) -> &'static str {
        use R::*;
        match self {
            OptcDataSourceSelect | OptcDataFormatControl | OptcInputClockControl | OptcInputGlobalControl => "ODM",
            VtgControl => "VTG", OppPipeControl => "OPP_PIPE", DpgControl | DpgDimensions | DpgOffsetSegment => "DPG",
            DigFeCntl | DigBeCntl | DigBeEnCntl => "DIG", PhypllPixclkResyncCntl => "PHYPLL", HpdIntStatus | HpdControl => "HPD",
            DomainPgStatus => "DOMAIN", HubpSurfaceConfig | HubpCntl => "HUBP",
            HubpreqSurfacePitch | HubpreqPrimarySurfaceAddress | HubpreqPrimarySurfaceAddressHigh | HubpreqVmidSettings0 | HubpreqFlipControl | HubpreqVmSystemApertureLow | HubpreqVmSystemApertureHigh => "HUBPREQ",
            DentistDispclkCntl | DcnVmFbLocationBase | DcnVmFbLocationTop | DcnVmFbOffset | DcnVmAgpBot | DcnVmAgpTop
                | DcnVmAgpBase => "",
            CcDcPipeDis | DmcubCntl | DmcubSecCntl | DmcubScratch | DmcubInbox0Wptr | DmcubInbox0Rptr | DmcubInbox1BaseAddress
                | DmcubInbox1Size | DmcubInbox1Wptr | DmcubInbox1Rptr | DmcubOutbox1Wptr | DmcubOutbox1Rptr | DmcubGpintDatain1
                | DmcubGpintDataout | DmcubTimerCurrent => "",
            HubpAddrConfig | HubpTilingConfig | HubpPriViewportStart | HubpPriViewportDimension | HubpReqSizeConfig | HubpReqSizeConfigC | HubpClkCntl => "HUBP",
            HubpreqSurfaceControl | HubpreqExpansionMode | HubpreqTtuQosWm | HubpreqGlobalTtuCntl | HubpreqSurf0TtuCntl0 | HubpreqSurf0TtuCntl1 | HubpreqCur0TtuCntl0 | HubpreqCur0TtuCntl1 | HubpreqVmMxL1TlbCntl | HubpreqBlankOffset0 | HubpreqBlankOffset1 | HubpreqDstDimensions | HubpreqDstAfterScaler | HubpreqPrefetchSettings | HubpreqVblankParameters0 | HubpreqVblankParameters1 | HubpreqVblankParameters2 | HubpreqVblankParameters3 | HubpreqVblankParameters4 | HubpreqNomParameters0 | HubpreqNomParameters1 | HubpreqNomParameters4 | HubpreqNomParameters5 | HubpreqPerLineDeliveryPre | HubpreqPerLineDelivery | HubpreqCursorSettings | HubpreqRefFreqToPixFreq | HubpreqDstYDeltaDrqLimit => "HUBPREQ",
            HubpretControl => "HUBPRET",
            DppControl => "DPP_TOP",
            CnvcSurfacePixelFormat | CnvcFormatControl => "CNVC_CFG",
            DsclSclMode | DsclControl | DsclRecoutStart | DsclRecoutSize | DsclMpcSize | DsclLbDataFormat | DsclTapControl | DsclHorzScaleRatio | DsclVertScaleRatio => "DSCL",
            CmControl | CmIcscControl | CmGamutRemapControl | CmDgamControl | CmBlndgamControl => "CM",
            MpccTopSel | MpccBotSel | MpccOppId | MpccControl => "MPCC",
            MpcOutMux => "MPC_OUT",
            DchubbubArbWatermark | DchubbubArbDfReqOutstand | DchubbubArbSatLevel | DchubbubArbDramStateCntl | DchubbubArbWatermarkChangeCntl | DchubbubGlobalTimerCntl | DppclkDtoCtrl | RefclkCntl => "",
            DppclkDtoParam => "DPPCLK",
            _ => "OTG",
        }
    }
}

/// A field of a register: its lowest bit and width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Field { pub shift: u8, pub width: u8 }

impl Field {
    pub const fn mask(self) -> u32 { (((1u64 << self.width) - 1) as u32) << self.shift }
    pub const fn put(self, value: u32) -> u32 { (value << self.shift) & self.mask() }
    pub const fn get(self, reg: u32) -> u32 { (reg & self.mask()) >> self.shift }
}

const fn f(shift: u8, width: u8) -> Field { Field { shift, width } }

/// The fields first light uses (dcn_2_1_0_sh_mask.h).
pub mod field {
    use super::{f, Field};
    pub const COUNTER_LOW: Field = f(0, 15); // H_TOTAL, V_TOTAL(_MIN/_MAX), blank and sync starts
    pub const COUNTER_HIGH: Field = f(16, 15); // blank and sync ends
    pub const SYNC_POL: Field = f(0, 1);
    pub const OTG_MASTER_EN: Field = f(0, 1);
    pub const OTG_DISABLE_POINT_CNTL: Field = f(8, 2);
    pub const OTG_START_POINT_CNTL: Field = f(12, 1);
    pub const OTG_FIELD_NUMBER_CNTL: Field = f(13, 1);
    pub const OTG_CURRENT_MASTER_EN_STATE: Field = f(16, 1);
    pub const OTG_INTERLACE_ENABLE: Field = f(0, 1);
    pub const OTG_H_TIMING_DIV_BY2: Field = f(0, 1);
    pub const OTG_CLOCK_EN: Field = f(0, 1);
    pub const OTG_CLOCK_GATE_DIS: Field = f(1, 1);
    pub const OTG_CLOCK_ON: Field = f(8, 1);
    pub const OTG_FRAME_COUNT: Field = f(0, 24);
    pub const VSTARTUP_START: Field = f(0, 10);
    pub const VUPDATE_OFFSET: Field = f(0, 16);
    pub const VUPDATE_WIDTH: Field = f(16, 10);
    pub const VREADY_OFFSET: Field = f(0, 16);
    pub const OPTC_INPUT_CLK_GATE_DIS: Field = f(0, 1);
    pub const OPTC_INPUT_CLK_EN: Field = f(1, 1);
    pub const OPTC_INPUT_CLK_ON: Field = f(2, 1);
    pub const OPTC_SEG0_SRC_SEL: Field = f(8, 4);
    pub const OPTC_DATA_FORMAT: Field = f(0, 2);
    pub const VTG0_FP2: Field = f(0, 15);
    pub const VTG0_VCOUNT_INIT: Field = f(16, 15);
    pub const VTG0_ENABLE: Field = f(31, 1);
    pub const OPP_PIPE_CLOCK_EN: Field = f(0, 1);
    pub const DPG_EN: Field = f(0, 1);
    pub const DPG_MODE: Field = f(4, 3);
    pub const DPG_DYNAMIC_RANGE: Field = f(8, 1);
    pub const DPG_BIT_DEPTH: Field = f(12, 2);
    pub const DPG_VRES: Field = f(16, 4);
    pub const DPG_HRES: Field = f(20, 4);
    pub const DPG_ACTIVE_HEIGHT: Field = f(0, 14);
    pub const DPG_ACTIVE_WIDTH: Field = f(16, 14);
    pub const WHOLE: Field = f(0, 32);
    pub const DIG_SOURCE_SELECT: Field = f(0, 3);
    pub const DIG_START: Field = f(10, 1);
    pub const TMDS_PIXEL_ENCODING: Field = f(28, 1);
    pub const TMDS_COLOR_FORMAT: Field = f(30, 2);
    pub const DIG_FE_SOURCE_SELECT: Field = f(8, 7);
    pub const DIG_MODE: Field = f(16, 3);
    pub const PHYPLL_DEEP_COLOR_CNTL: Field = f(4, 2);
    pub const DC_HPD_SENSE: Field = f(1, 1);
    pub const DENTIST_DISPCLK_WDIVIDER: Field = f(0, 7);
    // Read by step 1.
    pub const DOMAIN_PGFSM_PWR_STATUS: Field = f(30, 2); // 0 on, 2 off (PGFSM_POWER_ON, _OFF)
    pub const DC_HPD_EN: Field = f(28, 1);
    pub const DIG_SYMCLK_FE_ON: Field = f(24, 1);
    pub const DIG_ENABLE: Field = f(0, 1);
    pub const DIG_SYMCLK_BE_ON: Field = f(8, 1);
    pub const HUBP_BLANK_EN: Field = f(0, 1);
    pub const HUBP_TTU_DISABLE: Field = f(12, 1);
    pub const HUBP_VTG_SEL: Field = f(4, 4); // the timing generator the HUBP follows
    pub const SURFACE_PIXEL_FORMAT: Field = f(0, 7);
    pub const ROTATION_ANGLE: Field = f(8, 2);
    pub const PITCH: Field = f(0, 14);
    pub const PRIMARY_SURFACE_ADDRESS_HIGH: Field = f(0, 16);
    pub const VM_16MIB: Field = f(0, 24); // FB_BASE, FB_TOP, FB_OFFSET, AGP_BOT, AGP_TOP, AGP_BASE: bits 47:24
    pub const OTG_PIXEL_RATE_SOURCE: Field = f(0, 2);
    pub const DP_DTO_ENABLE: Field = f(4, 1);
    pub const OTG_PHYPLL_PIXEL_RATE_SOURCE: Field = f(0, 3);
    pub const OTG_PIXEL_RATE_PLL_SOURCE: Field = f(4, 1);
    pub const PHYPLL_PIXCLK_ENABLE: Field = f(8, 1);
    pub const DENTIST_DISPCLK_RDIVIDER: Field = f(8, 7);
    // The DMCUB (dcn_2_1_0_sh_mask.h): whether the board has it, whether it is enabled or held in reset.
    pub const DC_DMCUB_ENABLE: Field = f(16, 1); // CC_DC_PIPE_DIS
    pub const DMCUB_ENABLE: Field = f(16, 1); // DMCUB_CNTL
    pub const DMCUB_SOFT_RESET: Field = f(17, 1);
    pub const DMCUB_PWAIT_MODE_STATUS: Field = f(20, 1);
    pub const DMCUB_SEC_RESET: Field = f(16, 1); // DMCUB_SEC_CNTL
    pub const DMCUB_SEC_RESET_STATUS: Field = f(21, 1);
    // The firmware's pipe as step 4's reference (batches 9 and 10, dcn_2_1_0_sh_mask.h).
    pub const PRI_VIEWPORT_X_START: Field = f(0, 14);
    pub const PRI_VIEWPORT_Y_START: Field = f(16, 14);
    pub const PRI_VIEWPORT_WIDTH: Field = f(0, 14);
    pub const PRI_VIEWPORT_HEIGHT: Field = f(16, 14);
    pub const SW_MODE: Field = f(0, 5);
    pub const SWATH_HEIGHT: Field = f(0, 3);
    pub const PTE_ROW_HEIGHT_LINEAR: Field = f(4, 3);
    pub const CHUNK_SIZE: Field = f(8, 3);
    pub const MIN_CHUNK_SIZE: Field = f(11, 2);
    pub const REFCYC_H_BLANK_END: Field = f(0, 13);
    pub const DLG_V_BLANK_END: Field = f(16, 15);
    pub const MIN_DST_Y_NEXT_START: Field = f(0, 18);
    pub const REFCYC_PER_HTOTAL: Field = f(0, 21);
    pub const REFCYC_X_AFTER_SCALER: Field = f(0, 13);
    pub const DST_Y_AFTER_SCALER: Field = f(16, 3);
    pub const VRATIO_PREFETCH: Field = f(0, 22);
    pub const DST_Y_PREFETCH: Field = f(24, 8);
    pub const REFCYC_PER_LINE_DELIVERY_L: Field = f(0, 13);
    pub const REF_FREQ_TO_PIX_FREQ: Field = f(0, 21);
    pub const DST_Y_DELTA_DRQ_LIMIT: Field = f(0, 15);
    pub const MIN_TTU_VBLANK: Field = f(0, 24);
    pub const REFCYC_PER_REQ_DELIVERY: Field = f(0, 23);
    pub const DPP_CLOCK_ENABLE: Field = f(4, 1);
    pub const CNVC_SURFACE_PIXEL_FORMAT: Field = f(0, 7);
    pub const DSCL_MODE: Field = f(0, 3);
    pub const RECOUT_START_X: Field = f(0, 13);
    pub const RECOUT_START_Y: Field = f(16, 13);
    pub const LOW14: Field = f(0, 14); // RECOUT_WIDTH, MPC_WIDTH
    pub const HIGH14: Field = f(16, 14); // RECOUT_HEIGHT, MPC_HEIGHT
    pub const CM_BYPASS: Field = f(0, 1);
    pub const MPCC_SEL: Field = f(0, 4); // MPCC_TOP_SEL, MPCC_BOT_SEL, MPCC_OPP_ID, MPC_OUT_MUX; 0xF: none
    pub const MPCC_MODE: Field = f(0, 2);
    pub const MPCC_GLOBAL_ALPHA: Field = f(16, 8);
    pub const DTO_PHASE: Field = f(0, 8); // DPPCLKn_DTO_PHASE
    pub const DTO_MODULO: Field = f(16, 8);
    pub const WATERMARK_14: Field = f(0, 14);
    pub const HUBP_CLOCK_ENABLE: Field = f(0, 1);
    pub const HUBP_VREADY_AT_OR_AFTER_VSYNC: Field = f(8, 1);
    pub const HUBP_UNDERFLOW_STATUS: Field = f(28, 3);
    pub const OPTC_UNDERFLOW_OCCURRED_STATUS: Field = f(10, 1);
    pub const PGFSM_POWER_ON: u32 = 0;
}

/// VBIOS command tables first light calls, with the parameter revision DCN 2.1's VBIOS has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Table {
    /// `setpixelclock` 1.7: the PHY's PLL for a TMDS pixel clock, for one timing generator.
    /// `misc` is `set_pixel_clock_parameter_v1_7`'s miscinfo (0x01 forces the PLL to be programmed, as its power-down does).
    SetPixelClock { pixclk_100hz: u32, pll_id: u8, encoder_id: u8, encoder_mode: u8, crtc_id: u8, deep_color: u8, misc: u8 },
    /// `digxencodercontrol` 1.5, stream setup: the DIG front end for the signal.
    DigEncoderStreamSetup { dig: u8, mode: u8, lanes: u8, pclk_10khz: u32, bpc: u8 },
    /// `dig1transmittercontrol` 1.6: a PHY (UNIPHY A-E) on or off.
    DigTransmitter { phy: u8, action: u8, mode: u8, lanes: u8, symclk_10khz: u32, hpd_sel: u8, dig_fe_sel: u8, connector_id: u8 },
}

/// The tables' indices in the master command table (`atom_master_list_of_command_functions_v2_1`).
pub const TABLE_DIGXENCODERCONTROL: u8 = 4;
pub const TABLE_SETPIXELCLOCK: u8 = 12;
pub const TABLE_DIG1TRANSMITTERCONTROL: u8 = 76;
pub const ENCODER_MODE_DVI: u8 = 2; // atom_encode_mode_def
pub const ENCODER_MODE_HDMI: u8 = 3;
pub const ENCODER_CMD_STREAM_SETUP: u8 = 0x0F; // atom_dig_encoder_control_action
pub const TRANSMITTER_DISABLE: u8 = 0; // atom_dig_transmitter_control_action
pub const TRANSMITTER_ENABLE: u8 = 1;
pub const PANEL_8BIT_PER_COLOR: u8 = 2; // atom_panel_bit_per_color
pub const COMBOPHY_PLL0: u8 = 20; // atom_ppll_def: ATOM_COMBOPHY_PLL0..4 are PHY A..E's PLLs

impl Table {
    /// The table's index in the master command table.
    pub fn index(&self) -> u8 {
        match self {
            Table::SetPixelClock { .. } => TABLE_SETPIXELCLOCK,
            Table::DigEncoderStreamSetup { .. } => TABLE_DIGXENCODERCONTROL,
            Table::DigTransmitter { .. } => TABLE_DIG1TRANSMITTERCONTROL,
        }
    }

    /// The parameter space the table is called with, little-endian, as `atomfirmware.h` lays it out
    /// (`set_pixel_clock_parameter_v1_7`, `dig_encoder_stream_setup_parameters_v1_5`,
    /// `dig_transmitter_control_ps_allocation_v1_6`): the bytes and their length.
    pub fn parameters(&self) -> ([u8; 32], usize) {
        let mut p = [0u8; 32];
        match *self {
            Table::SetPixelClock { pixclk_100hz, pll_id, encoder_id, encoder_mode, crtc_id, deep_color, misc } => {
                p[0..4].copy_from_slice(&pixclk_100hz.to_le_bytes());
                p[4] = pll_id; p[5] = encoder_id; p[6] = encoder_mode; p[7] = misc; p[8] = crtc_id; p[9] = deep_color;
                (p, 16)
            }
            Table::DigEncoderStreamSetup { dig, mode, lanes, pclk_10khz, bpc } => {
                p[0] = dig; p[1] = ENCODER_CMD_STREAM_SETUP; p[2] = mode; p[3] = lanes;
                p[4..8].copy_from_slice(&pclk_10khz.to_le_bytes());
                p[8] = bpc;
                (p, 12)
            }
            Table::DigTransmitter { phy, action, mode, lanes, symclk_10khz, hpd_sel, dig_fe_sel, connector_id } => {
                p[0] = phy; p[1] = action; p[2] = mode; p[3] = lanes;
                p[4..8].copy_from_slice(&symclk_10khz.to_le_bytes());
                p[8] = hpd_sel; p[9] = dig_fe_sel; p[10] = connector_id;
                (p, 32)
            }
        }
    }
}

/// One step of a plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Read the register, replace the bits of `mask` with `value`'s, write it back.
    Update { reg: Reg, mask: u32, value: u32 },
    /// Wait until `(register & mask) == value`, polling, for at most `timeout_us`.
    Wait { reg: Reg, mask: u32, value: u32, timeout_us: u32 },
    /// Run a VBIOS command table with these parameters.
    Call(Table),
    /// Wait for OTG `otg`'s frame counter to move `frames` times, for at most `timeout_us`: the generator runs.
    Frames { otg: u8, frames: u32, timeout_us: u32 },
    /// Stage `n` (1 to `STAGES`) begins here: the steps up to the next marker are one stage.
    Begin(u8),
    /// Step 4's reference (stage 9): the firmware's pipe measured and pipe 1's request, DLG and TTU values derived from
    /// it and from step 1's batches 9 and 10; nothing written.
    Reference,
    /// Step 4's picture (stage 10): drawn into `display`'s own surface (BAR 0's slot 5); no register written.
    Draw,
    /// Step 4's stages 11 to 15: their writes are built from stage 9's reference when it has run.
    Late,
    /// No new underflow on the firmware's pipe or on pipe 1 since stage 9 (their HUBPs' and ODMs' statuses).
    NoUnderflow,
}

/// First light's stages, in order: what `Step::Begin(n)` opens (step 3 of 213-DRV-0086 lets them go one at a time).
/// Step 3's stages 1 to 8 (first light), then step 4's from 9 (213-DRV-0092: MIND Core's picture on the second output).
pub const STAGES: u8 = 15;
pub const FIRST_LIGHT: u8 = 8;
/// The first stage that writes pipe 1's HUBP, DPP or MPC (the guard lets them through from here).
pub const PIPE_WRITES_FROM: u8 = 11;
pub const STAGE_NAMES: [&str; STAGES as usize] =
    ["OTG CLOCKS", "PIXEL CLOCK", "TIMING", "TEST PATTERN", "START", "FRONT END", "BACK END", "PHY", "REFERENCE", "DRAW",
     "PIPE CLOCKS", "DPP", "HUBP", "MPC", "UNBLANK"];

/// The pipe a plan drives: its timing generator, OPP and DIG front end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pipe { pub otg: u8, pub opp: u8, pub dig_fe: u8 }

/// The output: its DIG back end and PHY (the same letter on DCN 2.1), its HPD pin and its VBIOS object ids.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Output {
    /// The PHY and DIG back end, 0 to 4 for A to E.
    pub phy: u8,
    /// The HPD pin, 0 to 4 (the table's hpd_sel is this plus one).
    pub hpd: u8,
    /// The encoder's object id byte (0x1E UNIPHY, 0x20 UNIPHY1, 0x21 UNIPHY2).
    pub encoder_id: u8,
    /// The connector's object id byte (0x0C HDMI type A).
    pub connector_id: u8,
}

/// The signal on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
    /// DVI signalling, which every HDMI sink takes: no infoframes, no audio, at most 165 MHz on one link.
    Dvi,
    /// HDMI signalling without scrambling (up to 340 MHz).
    Hdmi,
}

/// The engine's test pattern: the OPP's display pattern generator (DPG; DCN 2.1's OTG has none of its own).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pattern { ColorSquares, VerticalBars, HorizontalBars }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanError {
    Timing(TimingError),
    /// The plan would touch the pipe or output the firmware drives.
    Protected,
    /// An instance beyond the engine's.
    NoSuchInstance,
    /// A pixel clock beyond what the signal carries.
    ClockTooHigh,
    /// More steps than a plan holds.
    Full,
}

pub const MAX_STEPS: usize = 192;

/// An ordered list of steps.
#[derive(Clone, Copy, Debug)]
pub struct Plan { steps: [Option<Step>; MAX_STEPS], len: usize }

impl Default for Plan { fn default() -> Self { Plan { steps: [None; MAX_STEPS], len: 0 } } }

impl Plan {
    pub fn steps(&self) -> impl Iterator<Item = Step> + '_ { self.steps[..self.len].iter().flatten().copied() }
    /// The `i`-th step.
    pub fn get(&self, i: usize) -> Option<Step> { self.steps[..self.len].get(i).copied().flatten() }
    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    pub(crate) fn push(&mut self, step: Step) -> Result<(), PlanError> {
        if self.len == MAX_STEPS { return Err(PlanError::Full); }
        self.steps[self.len] = Some(step);
        self.len += 1;
        Ok(())
    }
    pub(crate) fn update(&mut self, reg: Reg, fields: &[(Field, u32)]) -> Result<(), PlanError> {
        let (mask, value) = fields.iter().fold((0, 0), |(m, v), (field, x)| (m | field.mask(), v | field.put(*x)));
        self.push(Step::Update { reg, mask, value })
    }
    fn wait(&mut self, reg: Reg, field: Field, value: u32, timeout_us: u32) -> Result<(), PlanError> {
        self.push(Step::Wait { reg, mask: field.mask(), value: field.put(value), timeout_us })
    }
    /// The registers the plan writes.
    pub fn written(&self) -> impl Iterator<Item = Reg> + '_ {
        self.steps().filter_map(|s| match s { Step::Update { reg, .. } => Some(reg), _ => None })
    }
}

/// What the firmware drives and must not be touched: its timing generator, OPP, DIG front end and PHY.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Protected { pub otg: u8, pub opp: u8, pub dig_fe: u8, pub phy: u8 }

impl Protected {
    /// The registers of the protected instances among those first light uses: read before and after, never written.
    pub fn registers(&self) -> impl Iterator<Item = Reg> + '_ {
        use R::*;
        const BY_PIPE: [R; 24] = [OtgHTotal, OtgHBlankStartEnd, OtgHSyncA, OtgHSyncACntl, OtgHTimingCntl, OtgVTotal,
            OtgVTotalMin, OtgVTotalMax, OtgVBlankStartEnd, OtgVSyncA, OtgVSyncACntl, OtgControl, OtgBlankControl,
            OtgInterlaceControl, OtgClockControl, OtgVstartupParam, OtgVupdateParam, OtgVreadyParam, OptcDataSourceSelect,
            OptcDataFormatControl, OptcInputClockControl, VtgControl, OtgPixelRateCntl, OtgPhypllPixelRateCntl];
        let pipe = BY_PIPE.iter().filter_map(move |r| r.at(self.otg));
        let opp = [OppPipeControl, DpgControl, DpgDimensions, DpgOffsetSegment].into_iter().filter_map(move |r| r.at(self.opp));
        let dig = [DigFeCntl.at(self.dig_fe), DigBeCntl.at(self.phy), PhypllPixclkResyncCntl.at(self.phy)];
        pipe.chain(opp).chain(dig.into_iter().flatten())
    }
}

/// The register of an instance, for a plan: an instance beyond the block's is refused.
fn reg(r: R, inst: u8) -> Result<Reg, PlanError> { r.at(inst).ok_or(PlanError::NoSuchInstance) }

/// First light: the engine's own test pattern on `output` through `pipe` at `timing`, the firmware's pipe and PHY left
/// alone. The order is Display Core's for a TMDS stream: the timing generator's clocks, the pixel clock, the timing,
/// the OPP and its pattern, the generator started and seen running, then the front end, the back end and the PHY.
/// It is a plan, not a guarantee (MC-12.3): it has run nowhere yet.
pub fn first_light(pipe: Pipe, output: Output, timing: &Timing, signal: Signal, pattern: Pattern, protected: Protected) -> Result<Plan, PlanError> {
    use field::*;
    use R::*;
    if pipe.otg >= PIPES || pipe.opp >= PIPES || pipe.dig_fe >= DIGS || output.phy >= DIGS || output.hpd >= DIGS {
        return Err(PlanError::NoSuchInstance);
    }
    if pipe.otg == protected.otg || pipe.opp == protected.opp || pipe.dig_fe == protected.dig_fe || output.phy == protected.phy {
        return Err(PlanError::Protected);
    }
    let otg = timing.otg().map_err(PlanError::Timing)?;
    let limit = match signal { Signal::Dvi => DVI_SINGLE_LINK_KHZ, Signal::Hdmi => TMDS_NO_SCRAMBLING_KHZ };
    if timing.pixel_khz > limit { return Err(PlanError::ClockTooHigh); }
    let mode = match signal { Signal::Dvi => ENCODER_MODE_DVI, Signal::Hdmi => ENCODER_MODE_HDMI };
    let (t, o, d) = (pipe.otg, pipe.opp, pipe.dig_fe);
    let mut plan = Plan::default();

    // The timing generator's clocks (optc1_enable_optc_clock).
    plan.push(Step::Begin(1))?;
    plan.update(reg(OptcInputClockControl, t)?, &[(OPTC_INPUT_CLK_EN, 1), (OPTC_INPUT_CLK_GATE_DIS, 1)])?;
    plan.wait(reg(OptcInputClockControl, t)?, OPTC_INPUT_CLK_ON, 1, 1000)?;
    plan.update(reg(OtgClockControl, t)?, &[(OTG_CLOCK_EN, 1), (OTG_CLOCK_GATE_DIS, 1)])?;
    plan.wait(reg(OtgClockControl, t)?, OTG_CLOCK_ON, 1, 1000)?;

    // The pixel clock: the PHY's PLL through the VBIOS, then 8-bit colour on its resync (dce112_program_pix_clk).
    plan.push(Step::Begin(2))?;
    plan.push(Step::Call(Table::SetPixelClock {
        pixclk_100hz: timing.pixel_khz * 10, pll_id: COMBOPHY_PLL0 + output.phy, encoder_id: output.encoder_id,
        encoder_mode: mode, crtc_id: t, deep_color: 0, misc: 0,
    }))?;
    plan.update(reg(PhypllPixclkResyncCntl, output.phy)?, &[(PHYPLL_DEEP_COLOR_CNTL, 0)])?;

    // The timing (optc1_program_timing), the generator stopped.
    plan.push(Step::Begin(3))?;
    write_timing(&mut plan, t, &otg)?;

    // The OPP's clock and its pattern generator, before the generator starts (opp2_set_disp_pattern_generator).
    plan.push(Step::Begin(4))?;
    plan.update(reg(OppPipeControl, o)?, &[(OPP_PIPE_CLOCK_EN, 1)])?;
    plan.update(reg(DpgDimensions, o)?, &[(DPG_ACTIVE_WIDTH, timing.h_active as u32), (DPG_ACTIVE_HEIGHT, timing.v_active as u32)])?;
    plan.update(reg(DpgOffsetSegment, o)?, &[(WHOLE, 0)])?;
    let (dpg_mode, res) = match pattern { Pattern::ColorSquares => (0, 6), Pattern::VerticalBars => (3, 0), Pattern::HorizontalBars => (4, 0) };
    plan.update(reg(DpgControl, o)?, &[(DPG_EN, 1), (DPG_MODE, dpg_mode), (DPG_DYNAMIC_RANGE, 0), (DPG_BIT_DEPTH, 1), (DPG_VRES, res), (DPG_HRES, res)])?;

    // The generator started on its OPP (optc2_enable_crtc) and seen counting frames.
    plan.push(Step::Begin(5))?;
    plan.update(reg(OptcDataSourceSelect, t)?, &[(OPTC_SEG0_SRC_SEL, o as u32)])?;
    plan.update(reg(VtgControl, t)?, &[(VTG0_ENABLE, 1)])?;
    plan.update(reg(OtgControl, t)?, &[(OTG_DISABLE_POINT_CNTL, 3), (OTG_MASTER_EN, 1)])?;
    plan.wait(reg(OtgControl, t)?, OTG_CURRENT_MASTER_EN_STATE, 1, 50_000)?;
    plan.push(Step::Frames { otg: t, frames: 2, timeout_us: 200_000 })?;

    // The DIG front end on that generator, RGB 8 bits (enc1_stream_encoder_dvi_set_stream_attribute).
    plan.push(Step::Begin(6))?;
    plan.push(Step::Call(Table::DigEncoderStreamSetup { dig: d, mode, lanes: 4, pclk_10khz: timing.pixel_khz / 10, bpc: PANEL_8BIT_PER_COLOR }))?;
    plan.update(reg(DigFeCntl, d)?, &[(DIG_SOURCE_SELECT, t as u32), (TMDS_PIXEL_ENCODING, 0), (TMDS_COLOR_FORMAT, 0)])?;

    // The back end's mode and front end (dcn10_link_encoder_setup, connect_dig_be_to_fe), then the PHY.
    plan.push(Step::Begin(7))?;
    plan.update(reg(DigBeCntl, output.phy)?, &[(DIG_MODE, mode as u32), (DIG_FE_SOURCE_SELECT, 1 << d)])?;
    plan.push(Step::Begin(8))?;
    plan.push(Step::Call(Table::DigTransmitter {
        phy: output.phy, action: TRANSMITTER_ENABLE, mode, lanes: 4, symclk_10khz: timing.pixel_khz / 10,
        hpd_sel: output.hpd + 1, dig_fe_sel: 1 << d, connector_id: output.connector_id,
    }))?;
    // The front end's FIFO started once its clock runs (the DIG_START pulse of the DCN 1 stream encoder).
    plan.update(reg(DigFeCntl, d)?, &[(DIG_START, 1)])?;
    plan.update(reg(DigFeCntl, d)?, &[(DIG_START, 0)])?;
    Ok(plan)
}

/// Step 4's stages after first light's (213-DRV-0092): 9, the reference; 10, the picture drawn; 11 to 15, built from
/// the reference when stage 9 has run.
pub fn second_pipe(plan: &mut Plan) -> Result<(), PlanError> {
    plan.push(Step::Begin(9))?;
    plan.push(Step::Reference)?;
    plan.push(Step::Begin(10))?;
    plan.push(Step::Draw)?;
    for n in PIPE_WRITES_FROM..=STAGES { plan.push(Step::Begin(n))?; plan.push(Step::Late)?; }
    Ok(())
}

/// The steps of stage `n` in `plan` (its markers left out).
pub fn steps_in(plan: &Plan, n: u8) -> impl Iterator<Item = Step> + '_ {
    let mut current = 0;
    plan.steps().filter(move |s| if let Step::Begin(m) = s { current = *m; false } else { current == n })
}

fn write_timing(plan: &mut Plan, t: u8, otg: &Otg) -> Result<(), PlanError> {
    use field::*;
    use R::*;
    plan.update(reg(OtgControl, t)?, &[(OTG_MASTER_EN, 0)])?;
    plan.update(reg(OtgHTotal, t)?, &[(COUNTER_LOW, otg.h_total)])?;
    plan.update(reg(OtgHSyncA, t)?, &[(COUNTER_LOW, 0), (COUNTER_HIGH, otg.h_sync_end)])?;
    plan.update(reg(OtgHBlankStartEnd, t)?, &[(COUNTER_LOW, otg.h_blank_start), (COUNTER_HIGH, otg.h_blank_end)])?;
    plan.update(reg(OtgHSyncACntl, t)?, &[(SYNC_POL, otg.h_sync_negative as u32)])?;
    plan.update(reg(OtgVTotal, t)?, &[(COUNTER_LOW, otg.v_total)])?;
    plan.update(reg(OtgVTotalMin, t)?, &[(COUNTER_LOW, otg.v_total)])?;
    plan.update(reg(OtgVTotalMax, t)?, &[(COUNTER_LOW, otg.v_total)])?;
    plan.update(reg(OtgVSyncA, t)?, &[(COUNTER_LOW, 0), (COUNTER_HIGH, otg.v_sync_end)])?;
    plan.update(reg(OtgVBlankStartEnd, t)?, &[(COUNTER_LOW, otg.v_blank_start), (COUNTER_HIGH, otg.v_blank_end)])?;
    plan.update(reg(OtgVSyncACntl, t)?, &[(SYNC_POL, otg.v_sync_negative as u32)])?;
    plan.update(reg(OtgInterlaceControl, t)?, &[(OTG_INTERLACE_ENABLE, 0)])?;
    plan.update(reg(VtgControl, t)?, &[(VTG0_ENABLE, 0)])?;
    plan.update(reg(OtgControl, t)?, &[(OTG_START_POINT_CNTL, 0), (OTG_FIELD_NUMBER_CNTL, 0)])?;
    // Global sync with no plane behind the pipe: VSTARTUP inside the vertical blank, after the sync.
    let vstartup = (otg.v_blank_end - otg.v_sync_end).clamp(1, 0x3FF);
    plan.update(reg(OtgVstartupParam, t)?, &[(VSTARTUP_START, vstartup)])?;
    plan.update(reg(OtgVupdateParam, t)?, &[(VUPDATE_OFFSET, 0), (VUPDATE_WIDTH, 1)])?;
    plan.update(reg(OtgVreadyParam, t)?, &[(VREADY_OFFSET, 1)])?;
    let fp2 = (vstartup as i64 - 1 - otg.v_blank_end as i64).max(0) as u32;
    plan.update(reg(VtgControl, t)?, &[(VTG0_FP2, fp2), (VTG0_VCOUNT_INIT, otg.vcount_init)])?;
    plan.update(reg(OptcDataFormatControl, t)?, &[(OPTC_DATA_FORMAT, 0)])?;
    plan.update(reg(OtgHTimingCntl, t)?, &[(OTG_H_TIMING_DIV_BY2, 0)])?;
    Ok(())
}
