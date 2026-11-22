// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! Owned initial packet mode; startup PHY permits remain invalid after this consuming transition.

use super::{DeviceIo,Session,Window,inbox};
use core::sync::atomic::{fence,Ordering};
use crate::{board::Board,btc_context::Initialized,channel_rfk,fwcmd,h2c,
    io::{self,Fault,FaultKind,RegisterIo},phy::{Phy,State,channel},runtime_mac,rx,rxdesc,station,tx,txdesc};

const ACTIVATE_US:u64=1_000_000;
// DEBUG (the Ryzen, 2026-11-08: everything runs first, hardening later): after a CMAC error the firmware's own L0
// recovery holds the BB in reset and zeroes the scheduler, which Linux never reads back. The data mode gives the firmware
// SER_WAIT_US to bring both back, restores the scheduler itself if only it is still off after a recovery SER_DBG_INFO
// counted, and goes on. False: the permit and coexistence checks stop the data mode.
pub(crate) const OUTLAST_SER:bool=true;
const SER_WAIT_US:u64=60_000;
// What an operation and a pending frame are given beyond a recovery's own wait: the 100 ms of a frame sent as one began
// ran out in the station's check on the Ryzen (2026-11-08, the first TLS traffic over the Wi-Fi alone).
const SER_GRACE_US:u64=20_000;
// DEBUG (the same): an operation keeps its own deadline while a frame is pending. A frame that never ends is the
// service's to time out (FRAME_MARGIN_MS) and the queue's budget's; capped by the frame's deadline, the operations near
// it ran out in whichever register they read (the Ryzen, 2026-11-08). False: the frame's deadline caps them.
const OWN_DEADLINES:bool=true;
const SER_DBG_INFO:usize=0x8424;
// 213-DRV-0091: a station register check that met an unreadable CMAC domain (DEADBEEF, as the Ryzen's boot 62 read the
// CMAC table's window twice in 98 minutes once the firmware knew the link) reads again UNREADABLE_TRIES times,
// UNREADABLE_GAP_US apart, and outlasts a firmware recovery that SER_DBG_INFO or the BB and the scheduler show meanwhile;
// only a mismatch with a readable value, or no answer at all after it, stops the data mode.
// The gap is at most 1 ms: io::short_delay refuses more, and 2 ms stopped the data mode 5 times (the Ryzen's boots 64-65).
const UNREADABLE:u32=0xdead_beef;
const UNREADABLE_TRIES:u32=16;
const UNREADABLE_GAP_US:u32=1_000;
// 213-DRV-0091: the firmware's error status, taken as Linux's rtw89_mac_get_err_status takes it (mac.c): HALT_C2H_CTRL
// nonzero, then HALT_C2H read and HALT_C2H_CTRL cleared. Read at each data-mode check; nothing is answered to the firmware.
const HALT_C2H_CTRL:usize=0x164;
const HALT_C2H:usize=0x16c;
const FIRMWARE_ERRORS_KEPT:usize=4;
// 213-DRV-0091: HISR0's halt bits (pci.h), read at each check as rtw89_pci_recognize_intrs reads them and cleared by writing
// them back; this driver masks HIMR0, so the status shows whether the firmware raises them at all. Nothing is answered.
const HISR0:usize=0x1a4;
const HISR0_HALT_C2H:u32=1<<21;
const HISR0_WDT_TIMEOUT:u32=1<<22;
/// A firmware error code's level, as ser.c's rtw89_ser_notify reads it (213-DRV-0091).
#[derive(Clone,Copy,Debug,Default,Eq,PartialEq)]
pub enum Level{L0,L1,L2,#[default]Unknown}
/// HALT_C2H's code after mac.c's scenario mapping, by mac.h's mac_ax_err_info name, with its level and ser.c's event.
pub fn firmware_error(raw:u32)->(u32,&'static str,Level,&'static str){
    let code=match raw>>28{1=>0x5000,2=>0x3000,3=>0x4000,_=>raw};
    let(name,level,event)=match code{
        0x0001=>("L0_ERR_CMAC0",Level::L0,"SER_EV_L0_RESET"),
        0x0002=>("L0_ERR_CMAC1",Level::L0,"SER_EV_L0_RESET"),
        0x0003=>("L0_RESET_DONE",Level::L0,"SER_EV_L0_RESET"),
        0x0010=>("L0_PROMOTE_TO_L1",Level::L1,"SER_EV_L1_RESET (M1)"),
        0x0999=>("L1_PREERR_DMAC",Level::L1,"SER_EV_L1_RESET_PREPARE (PRE-M0)"),
        0x1000=>("L1_ERR_DMAC",Level::L1,"SER_EV_L1_RESET (M1)"),
        0x1001=>("L1_RESET_DISABLE_DMAC_DONE",Level::L1,"SER_EV_DO_RECOVERY (M3)"),
        0x1002=>("L1_RESET_RECOVERY_DONE",Level::L1,"SER_EV_MAC_RESET_DONE (M5)"),
        0x1010=>("L1_PROMOTE_TO_L2",Level::L2,"SER_EV_L2_RESET"),
        0x1011=>("L1_RCVY_STOP_DONE",Level::L1,"NONE"),
        0x2400=>("L2_RESET_DONE",Level::L2,"SER_EV_L2_RESET"),
        0x2599=>("L2_ERR_WDT_TIMEOUT_INT",Level::L2,"SER_EV_L2_RESET"),
        0x3000=>("CPU_EXCEPTION",Level::L2,"SER_EV_L2_RESET"),
        0x4000=>("ASSERTION",Level::L2,"SER_EV_L2_RESET"),
        0x5000=>("RXI300",Level::L2,"SER_EV_L2_RESET"),
        0x2000..=0x5001=>("L2_ERR",Level::L2,"SER_EV_L2_RESET"),
        _=>("UNKNOWN",Level::Unknown,"NONE"),
    };(code,name,level,event)
}
/// The firmware's n-th error code, as read (213-DRV-0091): HALT_C2H_CTRL and HALT_C2H, SER_DBG_INFO then, and the code by
/// mac.h's name with its level and ser.c's event.
#[derive(Clone,Copy,Debug,Default,Eq,PartialEq)]
pub(crate) struct FirmwareError{pub count:u32,pub ctrl:u32,pub raw:u32,pub ser:u32,pub code:u32,pub name:&'static str,pub level:Level,pub event:&'static str}
/// The firmware's error codes by level since the data mode began (213-DRV-0091).
#[derive(Clone,Copy,Debug,Default,Eq,PartialEq)]
pub struct Errors{pub l0:u32,pub l1:u32,pub l2:u32,pub unknown:u32,pub hisr_halt:u32,pub hisr_wdt:u32}
/// The n-th step of SER_DBG_INFO a data-mode check saw (213-DRV-0091): from and to, the time since the check before, and
/// RXQ's and RPQ's polls so far this minute.
#[derive(Clone,Copy,Debug,Default,Eq,PartialEq)]
pub(crate) struct SerStep{pub count:u32,pub from:u32,pub to:u32,pub since_us:u64,pub polls:[crate::rx::Polls;2]}
/// The n-th check that found HISR0's halt bits set (213-DRV-0091): HISR0, HALT_C2H_CTRL and SER_DBG_INFO then.
#[derive(Clone,Copy,Debug,Default,Eq,PartialEq)]
pub(crate) struct HaltStatus{pub count:u32,pub hisr:u32,pub ctrl:u32,pub ser:u32,pub intx:Option<bool>}
const CMAC_ERR_ISR:usize=0xc164;
const LIVE_US:u32=10_000;
pub(crate) const SCHEDULER:u16=0x010f;
pub(crate) const STOPPED_UNUSED:u32=1<<17;
const OWN_DMA:u32=0x0001_0f00|1<<19;

#[derive(Clone,Copy,Debug)]
pub(super) enum Mode{Startup,Data(Layout)}
#[derive(Clone,Copy,Debug)]
pub(crate) struct Layout{command:u64,receive:u64,transmit:u64,physical:u64}
impl Layout{
    pub(crate) fn matches(&self,queue:&tx::Queue<'_>)->bool{self.transmit==queue.session()&&self.physical==queue.physical()}
    pub(crate) fn matches_receive(&self,command:u64,receive:u64)->bool{self.command==command&&self.receive==receive}
    pub(crate) fn check_registers(&self,bus:&mut impl RegisterIo)->io::Result<()>{tx::check_layout(bus,self.physical)}
}

#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) enum Stage{Guard,Station,Dma,Scheduler,Verify,Runtime,Filter(station::Stage)}
#[derive(Debug)]
pub(crate) enum Cause{Coexistence(crate::btc_context::Error),Identity,State,Register(Fault),Session(super::Error),Station(station::CheckFailure),Tx(tx::Error)}
impl From<Fault> for Cause{fn from(v:Fault)->Self{Self::Register(v)}}
impl From<super::Error> for Cause{fn from(v:super::Error)->Self{Self::Session(v)}}
impl From<tx::Error> for Cause{fn from(v:tx::Error)->Self{Self::Tx(v)}}
#[derive(Clone,Copy,Debug)]
pub(crate) enum Shutdown{NotActivated,Attempted{radio:Option<Fault>,transmit:tx::Stop}}
#[derive(Debug)]
pub(crate) struct Failure{pub stage:Stage,pub cause:Cause,pub shutdown:Option<Shutdown>}
/// What the first shutdown found, before it changed anything: the firmware's error status (HALT_C2H_CTRL, HALT_C2H,
/// HALT_H2C_CTRL, as Linux's rtw89_mac_get_err_status), SER_DBG_INFO, the DMAC and CMAC error ISRs, DLE_CTRL and
/// PHYINFO_ERR_IMR (what Linux dumps for a CMAC DMA error on the 8852B), SYS_FUNC_EN in 0x0's byte 2, the scheduler,
/// DMA_STOP and INIT_CONFIG. All ones: not read.
pub const SNAPSHOT:[usize;12]=[0x164,0x16c,0x160,0x8424,0x8524,0xc164,0xc800,0xccfc,0,0xc348,fwcmd::DMA_STOP,fwcmd::INIT_CONFIG];
/// One firmware recovery the data mode outlasted (OUTLAST_SER): the n-th, SER_DBG_INFO and CMAC_ERR_ISR then, when the
/// BB was out of reset (0: at once), whether the driver restored the scheduler after `waited_us` or the firmware did, and
/// whether it was found right after a coexistence reconcile (which may send H2C) or before it.
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) struct Recovery{pub count:u32,pub ser:u32,pub cmac:u32,pub bb_us:u64,pub restored:bool,pub waited_us:u64,pub after_reconcile:bool}
#[derive(Clone,Copy,Eq,PartialEq)]
pub struct Snapshot(pub [u32;SNAPSHOT.len()]);
impl core::fmt::Debug for Snapshot{
    fn fmt(&self,f:&mut core::fmt::Formatter<'_>)->core::fmt::Result{
        for(at,value)in SNAPSHOT.iter().zip(self.0){write!(f,"{:X}={:08X} ",at,value)?;}Ok(())
    }
}

/// What a hole in the reception is read with (213-DRV-0090), and a good link for comparison; reads only, but the LTE
/// window's indirect read, as the coexistence reconcile makes it:
/// - MAC: SYS_FUNC_EN in 0x0, the coexistence control byte in 0x73, the scoreboard; HALT_H2C/C2H and their controls,
///   WCPU_FW_CTRL; the RXQ and RPQ rings' indexes (host in bits 0-11, hardware in 16-27: equal, nothing was fed);
///   SER_DBG_INFO, DLE_EMPTY0/1, DMAC_ERR_IMR and _ISR (and CMAC_ERR_IMR, RX_ERR_FLAG_IMR below: what the MAC raises to
///   the firmware, 213-DRV-0091); MACID pause and sleep, CTN_TXEN (transmit paused); port 0's PORT_CFG
///   (net type, BSSID fit, TSF update); CMAC_ERR_ISR, RX_ERR_FLAG, RX_FLTR_OPT, the CTRL/MGNT/DATA type filters (two
///   bits a subtype: 1 to the host, 2 to the WLAN CPU) and BCN_PSR_RPT; the pollution 0xC67C; Linux's coexistence
///   monitor set for the 8852B
///   (rtw89_btc_8852b_mon_reg: 0xCEF4, 0xD200, 0xD220, 0xDA10-0xDA4C); DMA_STOP and INIT_CONFIG.
/// - BB: the same monitor set's 0x980, 0x4738, 0x4688, 0x4694; DIG's PD lower bound (0x4860, 0x4B64) and initial
///   LNA/TIA/RXB gains (0x472C, 0x4A80, 0x473C, 0x46A8); the EDCCA level (0x4884) and its reports (0x1738, 0x173C);
///   TX collision (0xC70); CCK (0x700) and the CCK path (0x2344), as the channel stage sets them.
/// - RF of both paths: mode (0x00), channel (0x18), VCO (0xB2) and the synthesizer's lock (0xC5 bit 15).
/// - The LTE window's SW_CFG_1: the grants the coexistence writes. All ones: not read.
pub const PROBE_MAC:[usize;41]=[0,0x70,0xac,0x160,0x164,0x168,0x16c,0x1e0,0x1050,0x1054,0x8424,0x8430,0x8434,0x8520,0x8524,
    0x9eb0,0xc2c0,0xc348,0xc400,0xc160,0xc164,0xc800,0xc804,0xce20,0xce24,0xce28,0xce2c,0xce84,0xc67c,0xcef4,0xd200,0xd220,0xda10,
    0xda20,0xda24,0xda28,0xda2c,0xda30,0xda34,0xda4c,fwcmd::DMA_STOP];
pub const PROBE_BB:[usize;17]=[0x980,0x4738,0x4688,0x4694,0x4860,0x4b64,0x4b74,0x472c,0x4a80,0x473c,0x46a8,0x4884,0x1738,0x173c,
    0xc70,0x700,0x2344];
pub const PROBE_RF:[u32;4]=[0,0x18,0xb2,0xc5];
#[derive(Clone,Copy,Eq,PartialEq)]
pub struct Probe{pub mac:[u32;PROBE_MAC.len()],pub bb:[u32;PROBE_BB.len()],pub rf:[[u32;PROBE_RF.len()];2],pub grants:u32}
impl core::fmt::Debug for Probe{
    fn fmt(&self,f:&mut core::fmt::Formatter<'_>)->core::fmt::Result{
        for(at,value)in PROBE_MAC.iter().zip(self.mac){write!(f,"{:X}={:X} ",at,value)?;}
        for(at,value)in PROBE_BB.iter().zip(self.bb){write!(f,"B{:X}={:X} ",at,value)?;}
        for(path,values)in self.rf.iter().enumerate(){for(at,value)in PROBE_RF.iter().zip(values){write!(f,"{}{:X}={:X} ",['A','B'][path],at,value)?;}}
        write!(f,"LTE38={:X}",self.grants)
    }
}

// The station check's fault when it read an unreadable CMAC domain (213-DRV-0091).
fn unreadable(result:&Result<(),Cause>)->Option<Fault>{
    match result{Err(Cause::Station(failure)) if failure.cause.value==UNREADABLE=>Some(failure.cause),_=>None}
}

/// Actual station context is consumed; the calibrated policy and board remain immutably borrowed throughout data mode.
pub(crate) struct PreparedActivation<'p>{board:&'p Board,tuned:&'p channel::Tuned,calibrated:&'p channel_rfk::Completed,
    owner:&'p Initialized,station:station::Prepared,command:u64,transmit:u64,physical:u64}

pub(crate) fn prepare<'p,B:DeviceIo>(phy:&mut Phy<'_,Session<'_,'_,'_,B>>,board:&'p Board,tuned:&'p channel::Tuned,
    calibrated:&'p channel_rfk::Completed,owner:&'p Initialized,station:station::Prepared,queue:&tx::Queue<'_>)
    ->Result<PreparedActivation<'p>,station::Failure>{
    let command=phy.bus_between_stages().command_session();
    // A refusal names each condition that failed (bits 0-10, in this order) and the PHY state (bits 16 and up).
    let refused=[phy.state()!=State::ChannelTuned,phy.attempt()!=tuned.attempt,command!=tuned.session,
        phy.identity()!=board.identity,tuned.access!=channel::Access::InitiateRadiation,!calibrated.matches(tuned),
        !owner.matches(tuned.attempt,command),!station.matches(tuned,board.identity.mac),station.current_filter().is_none(),
        queue.state()!=tx::State::Installed||queue.session()==0,!phy.bus_between_stages().retains_packets()]
        .iter().enumerate().fold(0u32,|bits,(i,&failed)|bits|u32::from(failed)<<i);
    if refused!=0{
        return Err(station::Failure{stage:station::Stage::Guard,
            cause:station::Cause::Register(Fault::new(FaultKind::PowerState,0,refused|(phy.state() as u32)<<16)),
            radio_stop:None,port_stop:None,selector_restore:None});
    }
    station.validate(phy,board,tuned,owner,LIVE_US)?;
    Ok(PreparedActivation{board,tuned,calibrated,owner,station,command,transmit:queue.session(),physical:queue.physical()})
}

/// Consumes the startup owner; forgetting this wrapper cannot yield an active startup Session or PHY interface.
/// The TX region remains exclusively borrowed and no failure, shutdown or Drop releases any DMA allocation.
pub(crate) struct DataSession<'q,'memory,'p,'a,'command,'receive,B:DeviceIo>{
    session:Session<'a,'command,'receive,B>,queue:&'q mut tx::Queue<'memory>,prepared:PreparedActivation<'p>,stopped:bool,
    cleanup_allowed:bool,last_shutdown:Option<Shutdown>,counts:Counts,tx_deadline:Option<u64>,snapshot:Option<Snapshot>,
    ser_seen:u32,recoveries:u32,recovery:Option<Recovery>,grace_us:u64,
    // 213-DRV-0091: the station checks that met an unreadable CMAC domain and went on, and the last, for the service's line.
    unreadables:u32,unreadable:Option<Unreadable>,
    // 213-DRV-0091: the firmware's error codes read and not yet taken, oldest first.
    firmware_errors:[FirmwareError;FIRMWARE_ERRORS_KEPT],firmware_pending:usize,halts:u32,halt_status:Option<HaltStatus>,
    // 213-DRV-0091: SER_DBG_INFO as the last check read it and when, its steps so far, and the last not yet taken.
    ser_now:u32,ser_checked:u64,ser_steps:u32,ser_step:Option<SerStep>,
}
/// A station register check that met an unreadable CMAC domain and went on (213-DRV-0091): the n-th, the register and
/// the value it read first, the reads until it answered and over how long, SER_DBG_INFO before and after, and whether a
/// firmware recovery was outlasted meanwhile.
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) struct Unreadable{pub count:u32,pub register:usize,pub value:u32,pub reads:u32,pub waited_us:u64,pub ser_before:u32,pub ser_after:u32,pub recovered:bool}
pub(crate) struct ActivationFailure<'q,'memory,'p,'a,'command,'receive,B:DeviceIo>{
    pub owner:DataSession<'q,'memory,'p,'a,'command,'receive,B>,pub failure:Failure,
}

/// `wifi`: every Wi-Fi frame the receive ring gave, kept or not, by which a stalled receive is seen (801-DRV-0071).
#[derive(Clone,Copy,Debug,Default,Eq,PartialEq)]
pub(crate) struct Counts{pub filtered:u64,pub invalid:u64,pub pressure:u64,pub previous_epoch:u64,pub btc_events:u64,pub btc_deferred:u64,pub ppdu_reports:u64,pub wifi:u64,
    pub signals:crate::ppdu::Signals,pub beacons:u64,pub other_beacons:u64,pub refused:Refused,pub errors:Errors,
    // 213-DRV-0091: when the joined BSS's last beacon was taken from the ring (the bus clock, µs), and its interval.
    pub beacon_at_us:u64,pub beacon_interval_us:u32,
    // 213-DRV-0102: every PPDU report by its signal in 5 dB bins from -20 dBm down (the last below -100), and those
    // that do not parse; the joined BSS's beacons measured and their signals' sum; the beacon just taken (its number and
    // count and rate) whose report is awaited. The reports with no RPL (OFDM not to this radio) by the header's paths
    // uncorrected, apart; the header's average over the RPL (half-dB summed, and how many) in the OFDM reports to us.
    pub ppdu_rssi:[u32;17],pub ppdu_unparsed:u32,pub bss_measured:u32,pub bss_signal_sum:i64,pub bss_beacon:Option<(u64,(u8,u16))>,
    pub beacon_now:bool,pub ppdu_no_rpl:[u32;17],pub ofdm_header_over:(i64,u32)}
/// 213-DRV-0102: the joined access point's mean signal, once some of its beacons were measured.
impl Counts{pub(crate) fn bss_dbm(&self)->Option<i8>{(self.bss_measured!=0).then(||(self.bss_signal_sum/i64::from(self.bss_measured))as i8)}}
/// 213-DRV-0102: a minute's PPDU reports by signal (5 dB bins from -20 dBm down) and the detection floor's registers.
#[derive(Clone,Copy,Debug,Default,Eq,PartialEq)]
pub struct FloorMinute{pub ppdu_rssi:[u32;17],pub unparsed:u32,pub registers:[u32;4],pub bss_dbm:Option<i8>,pub measured:u32,pub set:bool,
    pub kind:FloorKind,pub switch_failures:u32,pub written:FloorWrite,pub no_rpl:[u32;17],pub header_over:(i64,u32)}
/// 213-DRV-0102: the detection floor's registers, in detection_floor::REGISTERS' order.
pub(crate) const FLOOR_REGISTERS:[usize;4]=[0x4860,0x4b64,0x4b74,0x4884];
/// 213-DRV-0102: a floor's write: the registers (bits in FLOOR_REGISTERS' order) not read back as written, and of those
/// the ones that read as before the write.
#[derive(Clone,Copy,Debug,Default,Eq,PartialEq)]
pub struct FloorWrite{pub unread:u8,pub unchanged:u8}
/// 213-DRV-0102: what a minute held when the floor alternates (DETECTION_FLOOR_ALTERNATES): not yet alternating, the
/// values found, or Linux's.
#[derive(Clone,Copy,Debug,Default,Eq,PartialEq)]
pub enum FloorKind{#[default]None,Off,On}
impl FloorMinute{
    /// The minute held Linux's floor throughout, or the values found (DETECTION_FLOOR_ALTERNATES).
    pub fn on(&self)->bool{self.kind==FloorKind::On}
    pub fn off(&self)->bool{self.kind==FloorKind::Off}
}
/// 213-DRV-0091: what may hold a fetched frame in the MAC, read directly at a late send (no RF, no indirect window, no
/// H2C): the coexistence control byte and the scoreboard; MACID pause (SS_MACID_PAUSE_0) and sleep (MACID_SLEEP_0);
/// CTN_TXEN; port 0's PORT_CFG, TBTT_PROHIB, BCN_AREA, the early-interrupt times, BCN_SPACE, BCN_CNT_TMR and the TSF;
/// PTCL_COMMON_SETTING_0 and PTCL_TX_CTN_SEL; Linux's coexistence monitor set (0xD200, 0xD220, 0xDA10-0xDA34); the RX
/// rings' indexes. BB: the monitor set's 0x980, the EDCCA level and its reports, TX collision.
pub const HELD_MAC:[usize;26]=[0x70,0xac,0x9eb0,0xc2c0,0xc348,0xc400,0xc404,0xc408,0xc40c,0xc414,0xc434,0xc438,0xc43c,0xc600,
    0xc6ec,0xd200,0xd220,0xda10,0xda20,0xda24,0xda28,0xda2c,0xda30,0xda34,0x1050,0x1054];
pub const HELD_BB:[usize;8]=[0x980,0x1738,0x173c,0x4884,0xc70,0x4860,0x4b64,0x4b74];
#[derive(Clone,Copy,Eq,PartialEq)]
pub struct Held{pub mac:[u32;HELD_MAC.len()],pub bb:[u32;HELD_BB.len()]}
impl core::fmt::Debug for Held{
    fn fmt(&self,f:&mut core::fmt::Formatter<'_>)->core::fmt::Result{
        for(at,value)in HELD_MAC.iter().zip(self.mac){write!(f,"{:X}={:X} ",at,value)?;}
        for(at,value)in HELD_BB.iter().zip(self.bb){write!(f,"B{:X}={:X} ",at,value)?;}Ok(())
    }
}
/// 213-DRV-0091: where in the beacon interval a moment is: by our clock since the last beacon taken from the ring, and by
/// port 0's TSF; the interval as the last beacon gave it (0: none heard yet).
#[derive(Clone,Copy,Debug,Default,Eq,PartialEq)]
pub struct Phase{pub clock_us:Option<u64>,pub tsf_us:Option<u64>,pub interval_us:u32}
/// 213-DRV-0091: a minute's sends by how they ended. Releases by the beacon interval's phase in 10 ms bins, by our clock
/// against the last beacon taken and by the port's TSF; the late frames released, those acknowledged at the first try and
/// those never, and their median hold; the times the service's frame buffer was full and the longest.
#[derive(Clone,Copy,Debug,Default,Eq,PartialEq)]
pub struct TxMinute{pub clock:[u32;11],pub tsf:[u32;11],pub late:u32,pub first_try:u32,pub not_acked:u32,pub median_hold_ms:u64,
    pub full_times:u32,pub full_longest_ms:u64,
    // 213-DRV-0104: sends answered at their deadline, refused busy and parked; parked frames released and of them
    // acknowledged; what the minute held when SEND_BEHIND_LATE alternates (None: it does not), and the switch otherwise.
    pub answered_late:u32,pub busy:u32,pub parked:u32,pub parked_released:u32,pub parked_acked:u32,pub behind:FloorKind,pub behind_fixed:bool}
impl TxMinute{
    /// 213-DRV-0104: what the minute held of SEND_BEHIND_LATE, for its line.
    pub fn behind_label(&self)->&'static str{
        match(self.behind,self.behind_fixed){
            (FloorKind::On,_)=>"THIS MINUTE BEHIND LATE ON",(FloorKind::Off,_)=>"THIS MINUTE BEHIND LATE OFF",
            (FloorKind::None,true)=>"BEHIND LATE ALWAYS",(FloorKind::None,false)=>"BEHIND LATE NEVER",
        }
    }
}
/// The first frames the station's filter refused (213-DRV-0090): each one's length and first 24 bytes.
#[derive(Clone,Copy,Debug,Default,Eq,PartialEq)]
pub struct Refused{pub frames:[(u16,[u8;24]);8],pub count:usize}
struct Dispatcher<'q,'memory>{queue:&'q mut tx::Queue<'memory>,filter:Option<station::Filter>,counts:&'q mut Counts,error:Option<tx::Error>}
impl super::PacketHandler for Dispatcher<'_,'_>{
    fn packet(&mut self,now:u64,channel:rx::Channel,packet:rxdesc::Packet,buffer:&[u8],full:bool)->Result<bool,super::Error>{
        if channel==rx::Channel::Rxq&&packet.kind==rxdesc::TYPE_WIFI{
            self.counts.wifi=self.counts.wifi.saturating_add(1);
            let Some(filter)=self.filter else{self.counts.previous_epoch=self.counts.previous_epoch.saturating_add(1);return Ok(true)};
            let frame=match crate::frames::from_packet(packet,buffer){Ok(frame)=>frame,Err(_)=>{
                self.counts.invalid=self.counts.invalid.saturating_add(1);return Ok(true)}};
            // 213-DRV-0090: beacons as the ring gives them, read by the station or not: the joined BSS's, and the others the
            // station's filter refuses.
            let m=frame.mpdu();let beacon=m.len()>=24&&m[0]==0x80;
            if !filter.accepts(m){
                self.counts.filtered=self.counts.filtered.saturating_add(1);
                let r=&mut self.counts.refused;
                if r.count<r.frames.len()&&m.len()>=24{r.frames[r.count]=(m.len() as u16,m[..24].try_into().unwrap());r.count+=1;}
                if beacon{self.counts.other_beacons=self.counts.other_beacons.saturating_add(1);}return Ok(true)
            }
            if beacon&&filter.bssid()!=[0;6]&&m[16..22]==filter.bssid(){
                self.counts.beacons=self.counts.beacons.saturating_add(1);self.counts.beacon_at_us=now;self.counts.beacon_now=true;
                if m.len()>=34{self.counts.beacon_interval_us=u32::from(u16::from_le_bytes([m[32],m[33]]))*1024;}
            }
            if full{self.counts.pressure=self.counts.pressure.saturating_add(1);return Ok(true)}
            return Ok(false)
        }
        let is_report=channel==rx::Channel::Rpq||packet.kind==rxdesc::TYPE_C2H&&{
            let bytes=packet.bytes(buffer).ok_or(super::Error::Malformed)?;
            let event=crate::c2h::parse(bytes).map_err(super::Error::Event)?;
            (event.category,event.class,event.function)==(1,9,1)
        };
        if !is_report{return Ok(false)}
        match self.queue.ingest(now,channel,rx::Outcome::Packet(packet),buffer){
            Ok(tx::Receive::Rpq|tx::Receive::TxReport{..})=>Ok(true),
            Ok(_)=>Err(super::Error::Malformed),
            Err(error)=>{self.error=Some(error);Err(super::Error::PacketHandler)},
        }
    }
    // 213-DRV-0102: the joined BSS's beacon taken just now is remembered by its number, count and rate, for its report.
    fn frame(&mut self,now:u64,number:u64,packet:rxdesc::Packet,buffer:&[u8],full:bool)->Result<bool,super::Error>{
        self.counts.beacon_now=false;let taken=self.packet(now,rx::Channel::Rxq,packet,buffer,full);
        if core::mem::take(&mut self.counts.beacon_now){self.counts.bss_beacon=crate::ppdu::key(buffer).map(|key|(number,key));}
        taken
    }
    // 213-DRV-0102: every report by its signal; the one after the BSS's beacon, of its count and rate, measures the BSS.
    // A report with no RPL is binned apart by the header's paths: Linux's moved signal for it means nothing.
    fn report(&mut self,frame:Option<u64>,packet:rxdesc::Packet,buffer:&[u8]){
        use crate::ppdu::Conversion;
        let c=&mut *self.counts;
        let Some(report)=crate::ppdu::from_packet(packet,buffer) else{c.ppdu_unparsed=c.ppdu_unparsed.saturating_add(1);return};
        let bin=|dbm:i8|((-20-i16::from(dbm)).max(0)/5).min(16)as usize;
        match report.conversion{
            Conversion::NoRpl=>{let b=bin(report.header_dbm);c.ppdu_no_rpl[b]=c.ppdu_no_rpl[b].saturating_add(1);}
            conversion=>{
                let b=bin(report.signal_dbm);c.ppdu_rssi[b]=c.ppdu_rssi[b].saturating_add(1);
                if let Conversion::Rpl{header_over,ofdm:true}=conversion{
                    let o=&mut c.ofdm_header_over;o.0=o.0.saturating_add(i64::from(header_over));o.1=o.1.saturating_add(1);
                }
            }
        }
        if let Some((number,key))=c.bss_beacon.take(){
            if frame==Some(number)&&(report.count,report.rate)==key{
                c.bss_measured=c.bss_measured.saturating_add(1);c.bss_signal_sum+=i64::from(report.signal_dbm);
            }
        }
    }
}

impl<'q,'memory,'p,'a,'command,'receive,B:DeviceIo> DataSession<'q,'memory,'p,'a,'command,'receive,B>{
    pub(crate) fn activate(session:Session<'a,'command,'receive,B>,queue:&'q mut tx::Queue<'memory>,prepared:PreparedActivation<'p>)
        ->Result<Self,ActivationFailure<'q,'memory,'p,'a,'command,'receive,B>>{
        let mut own=Self{session,queue,prepared,stopped:false,cleanup_allowed:false,last_shutdown:None,counts:Counts::default(),tx_deadline:None,snapshot:None,
            ser_seen:0,recoveries:0,recovery:None,grace_us:0,unreadables:0,unreadable:None,
            firmware_errors:[FirmwareError::default();FIRMWARE_ERRORS_KEPT],firmware_pending:0,halts:0,halt_status:None,
            ser_now:0,ser_checked:0,ser_steps:0,ser_step:None};
        if let Err(failure)=own.activate_inner(){return Err(ActivationFailure{owner:own,failure})}
        own.ser_seen=io::read32(own.session.bus,SER_DBG_INFO).unwrap_or(u32::MAX);own.ser_now=own.ser_seen;
        own.ser_checked=own.session.bus.now_us();Ok(own)
    }
    fn identity(&self)->bool{
        let p=&self.prepared;
        p.command==self.session.command_session()&&p.command==p.tuned.session&&p.transmit==self.queue.session()
            &&p.physical==self.queue.physical()&&p.calibrated.matches(p.tuned)&&p.owner.matches(p.tuned.attempt,p.command)
            &&p.station.matches(p.tuned,p.board.identity.mac)&&p.station.current_filter().is_some()&&self.session.retains_packets()
    }
    fn activate_inner(&mut self)->Result<(),Failure>{
        let mut stage=Stage::Guard;let mut modified=false;
        let result=(||->Result<(),Cause>{
            if !self.identity()||!matches!(self.session.mode,Mode::Startup)||self.session.failed.is_some(){return Err(Cause::Identity)}
            let deadline=self.session.now()?.checked_add(ACTIVATE_US).ok_or(super::Error::Clock)?;
            let left=self.session.left(deadline)?;self.session.live(left.min(LIVE_US))?;
            self.drain_previous_epoch(deadline)?;
            let layout=Layout{command:self.session.queue.session(),receive:self.session.rings.session(),transmit:self.queue.session(),physical:self.queue.physical()};
            let mut window=Window{bus:self.session.bus,deadline,last:self.session.last,clock_bad:false};
            let outcome=(||->Result<(),Cause>{
                self.queue.installed_for_data(&mut window)?;
                self.cleanup_allowed=true;stage=Stage::Station;modified=true;
                self.prepared.station.check_registers(&mut window).map_err(Cause::Station)?;
                radio(&mut window,self.prepared.tuned.channel)?;
                require(&mut window,0xc348,0xffff,0)?;
                stage=Stage::Dma;modified=true;
                fence(Ordering::Release);
                io::modify32(&mut window,fwcmd::DMA_STOP,OWN_DMA,0)?;
                require(&mut window,fwcmd::DMA_STOP,0x001f0f00,STOPPED_UNUSED)?;
                stage=Stage::Scheduler;
                runtime_mac::exchange_register(&mut window,[0x205|u32::from(SCHEDULER)<<16,0xffff,0,0],4)?;
                require(&mut window,0xc348,0xffff,u32::from(SCHEDULER))?;
                self.queue.start_data(&mut window,self.session.rings,&layout)?;window.delay_us(0)?;Ok(())
            })();
            self.session.last=window.last;outcome?;
            self.session.mode=Mode::Data(layout);stage=Stage::Verify;
            let left=self.session.left(deadline)?;self.session.live(left.min(LIVE_US))?;
            self.enable_runtime_commands(deadline)?;self.session.left(deadline)?;Ok(())
        })();
        if let Err(cause)=result{
            let shutdown=modified.then(||self.shutdown());
            let _=self.session.fail::<()>(super::Error::Session);
            return Err(Failure{stage,cause,shutdown});
        }Ok(())
    }
    fn enable_runtime_commands(&mut self,deadline:u64)->Result<(),Cause>{
        let mut window=Window{bus:self.session.bus,deadline,last:self.session.last,clock_bad:false};
        let observed=(||{
            self.session.queue.reclaim(&mut window).map_err(super::Error::Dma)?;
            let busy=io::read32(&mut window,fwcmd::DMA_BUSY)?;window.delay_us(0)?;
            if self.session.queue.pending()!=0||busy&super::TX_BUSY!=0{return Err(Cause::State)}
            Ok(())
        })();self.session.last=window.last;observed?;
        self.session.left(deadline)?;
        self.session.engine.enable_serial(self.session.last).map_err(super::Error::Command)?;Ok(())
    }
    fn drain_previous_epoch(&mut self,deadline:u64)->Result<(),Cause>{
        while let Some(record)=self.session.packet(){
            if record.metadata().channel!=rx::Channel::Rxq||record.metadata().packet.kind!=rxdesc::TYPE_WIFI{return Err(Cause::State)}
            self.session.consume_packet();self.counts.previous_epoch=self.counts.previous_epoch.saturating_add(1);
        }
        for _ in 0..=rx::DESCRIPTORS*2{
            let mut handler=Dispatcher{queue:self.queue,filter:None,counts:&mut self.counts,error:None};
            let read=self.session.poll_before_with(deadline,&mut handler)
                .map_err(|e|handler.error.map_or(Cause::Session(e),Cause::Tx))?;
            if self.session.pending_packets()!=0{return Err(Cause::State)}
            if read==[false,false]{return Ok(())}
        }Err(Cause::State)
    }
    // OUTLAST_SER: a BB in reset or a scheduler off is waited for, before and after each reconcile. SER_DBG_INFO counts the firmware's recovery only once
    // it is over (the Ryzen); a scheduler still off after it is restored. Without a counted recovery, it is refused.
    // The wait has its own time, and the deadline returned, the pending frame's and the operation's, move by it and SER_GRACE_US.
    fn outlast_ser(&mut self,deadline:u64,after_reconcile:bool)->Result<u64,Cause>{
        if !OUTLAST_SER{return Ok(deadline)}
        let seen=self.ser_seen;let own=self.session.now()?.saturating_add(SER_WAIT_US+SER_GRACE_US);
        let mut window=Window{bus:self.session.bus,deadline:deadline.max(own),last:self.session.last,clock_bad:false};
        let result=(||->io::Result<Option<Recovery>>{
            let bb_on=|bb:u8|bb!=u8::MAX&&bb&3==3;
            let bb=window.read8(crate::registers::R_AX_SYS_FUNC_EN)?;
            if bb_on(bb)&&io::read32(&mut window,0xc348)?&0xffff==u32::from(SCHEDULER){return Ok(None)}
            let cmac=io::read32(&mut window,CMAC_ERR_ISR)?;let start=window.now_us();let mut bb_us=None;
            loop{
                let waited=window.now_us().saturating_sub(start);
                if bb_us.is_none()&&bb_on(window.read8(crate::registers::R_AX_SYS_FUNC_EN)?){bb_us=Some(waited)}
                let scheduler=io::read32(&mut window,0xc348)?&0xffff==u32::from(SCHEDULER);
                if let(Some(bb_us),true)=(bb_us,scheduler){
                    let ser=io::read32(&mut window,SER_DBG_INFO)?;
                    return Ok(Some(Recovery{count:0,ser,cmac,bb_us,restored:false,waited_us:waited,after_reconcile}))
                }
                if waited>=SER_WAIT_US{break}
                window.delay_us(500)?;
            }
            let waited=window.now_us().saturating_sub(start);let ser=io::read32(&mut window,SER_DBG_INFO)?;
            let Some(bb_us)=bb_us else{return Err(Fault::new(FaultKind::PowerState,crate::registers::R_AX_SYS_FUNC_EN,ser))};
            if ser==seen{return Err(Fault::new(FaultKind::PowerState,0xc348,ser))}
            runtime_mac::exchange_register(&mut window,[0x205|u32::from(SCHEDULER)<<16,0xffff,0,0],4)?;
            require(&mut window,0xc348,0xffff,u32::from(SCHEDULER))?;
            Ok(Some(Recovery{count:0,ser,cmac,bb_us,restored:true,waited_us:waited,after_reconcile}))
        })();
        self.session.last=window.last;
        let Some(mut recovery)=result? else{return Ok(deadline)};
        self.recoveries=self.recoveries.saturating_add(1);recovery.count=self.recoveries;
        self.ser_seen=recovery.ser;self.recovery=Some(recovery);
        let grace=recovery.waited_us.saturating_add(SER_GRACE_US);self.grace_us=self.grace_us.saturating_add(grace);
        self.tx_deadline=self.tx_deadline.map(|at|at.saturating_add(grace));self.queue.extend(grace);
        Ok(deadline.saturating_add(grace))
    }
    // The deadline the operation goes on with: later by an outlasted recovery's time (OUTLAST_SER).
    fn live(&mut self,deadline:u64)->Result<u64,Cause>{
        if self.stopped||!self.identity()||!matches!(self.session.mode,Mode::Data(_)){return Err(Cause::State)}
        if let Some(error)=self.session.failed{return Err(Cause::Session(error))}
        self.firmware_status(deadline)?;
        let mut deadline=self.outlast_ser(deadline,false)?;
        let mut result=self.checks(deadline).and_then(|()|self.permits(deadline));
        // 213-DRV-0091: a station check that met an unreadable CMAC domain, and what it was then.
        let first=unreadable(&result);let(ser_before,start,recoveries)=(self.ser_seen,self.session.now()?,self.recoveries);
        // OUTLAST_SER: a recovery that began during the checks or the permits' (the Ryzen, 2026-11-08: the scheduler read
        // zero there after 42 minutes) is outlasted, and they are made once more.
        if result.is_err()&&OUTLAST_SER{
            let before=self.recoveries;let after=self.outlast_ser(deadline,false)?;
            if self.recoveries!=before{deadline=after;result=self.checks(deadline).and_then(|()|self.permits(deadline));}
        }
        // An unreadable domain is read again; whichever way it answers, the meeting is counted.
        if let Some(first)=first{
            // The checks made so far: the first, and once more if a recovery was outlasted above.
            let mut reads=if self.recoveries!=recoveries{2}else{1};
            if unreadable(&result).is_some(){(deadline,reads)=self.read_again(deadline,reads)?;}else{result?;}
            let mut window=Window{bus:self.session.bus,deadline,last:self.session.last,clock_bad:false};
            let ser_after=io::read32(&mut window,SER_DBG_INFO);self.session.last=window.last;
            self.unreadables=self.unreadables.saturating_add(1);
            self.unreadable=Some(Unreadable{count:self.unreadables,register:first.register,value:first.value,reads,
                waited_us:self.session.now()?.saturating_sub(start),ser_before,ser_after:ser_after.map_err(Cause::Register)?,
                recovered:self.recoveries!=recoveries});
            result=Ok(());
        }
        result?;self.session.left(deadline)?;Ok(deadline)
    }
    // 213-DRV-0091: the checks made again while they meet an unreadable CMAC domain, UNREADABLE_GAP_US apart, a firmware
    // recovery that SER_DBG_INFO shows meanwhile outlasted first. A readable mismatch, or no answer after
    // UNREADABLE_TRIES, ends as the check's failure. Returns the deadline and the checks made in all.
    fn read_again(&mut self,deadline:u64,made:u32)->Result<(u64,u32),Cause>{
        let mut deadline=deadline;let mut reads=made;
        loop{
            let mut window=Window{bus:self.session.bus,deadline,last:self.session.last,clock_bad:false};
            let ser=window.delay_us(UNREADABLE_GAP_US).and_then(|()|io::read32(&mut window,SER_DBG_INFO));
            self.session.last=window.last;
            if ser.map_err(Cause::Register)?!=self.ser_seen{deadline=self.outlast_ser(deadline,false)?;}
            reads+=1;
            match self.checks(deadline).and_then(|()|self.permits(deadline)){
                Ok(())=>return Ok((deadline,reads)),
                Err(Cause::Station(failure)) if failure.cause.value==UNREADABLE&&reads<UNREADABLE_TRIES=>continue,
                Err(cause)=>return Err(cause),
            }
        }
    }
    // 213-DRV-0091: the firmware's error status, as mac.c's rtw89_mac_get_err_status takes it, counted and kept for the line.
    fn firmware_status(&mut self,deadline:u64)->Result<(),Cause>{
        let mut window=Window{bus:self.session.bus,deadline,last:self.session.last,clock_bad:false};
        let read=(||->io::Result<(u32,u32,Option<u32>,u32,Option<bool>)>{
            // All ones is a device that does not answer, which the checks report.
            let isr=io::read32(&mut window,HISR0)?;let halt=if isr==u32::MAX{0}else{isr&(HISR0_HALT_C2H|HISR0_WDT_TIMEOUT)};
            // PCI Status bit 3 before the bits are cleared: whether the device asserts INTx for them.
            let intx=if halt!=0{window.bus.pci_status().ok().map(|status|status&8!=0)}else{None};
            if halt!=0{window.write32(HISR0,halt)?;}
            let ctrl=io::read32(&mut window,HALT_C2H_CTRL)?;let set=ctrl!=0&&ctrl!=u32::MAX;
            let raw=if set{let raw=io::read32(&mut window,HALT_C2H)?;window.write32(HALT_C2H_CTRL,0)?;Some(raw)}else{None};
            // Read at every check (213-DRV-0091): a silent recovery is placed at the check that sees its step.
            let ser=io::read32(&mut window,SER_DBG_INFO)?;
            Ok((if halt!=0{isr}else{0},ctrl,raw,ser,intx))
        })();
        self.session.last=window.last;
        let(isr,ctrl,raw,ser,intx)=read.map_err(Cause::Register)?;
        let now=window.last;
        if ser!=self.ser_now&&ser!=u32::MAX{
            self.ser_steps=self.ser_steps.saturating_add(1);
            self.ser_step=Some(SerStep{count:self.ser_steps,from:self.ser_now,to:ser,since_us:now.saturating_sub(self.ser_checked),polls:self.session.rings.polls()});
            self.ser_now=ser;
        }
        self.ser_checked=now;
        if isr!=0{
            let e=&mut self.counts.errors;
            if isr&HISR0_HALT_C2H!=0{e.hisr_halt=e.hisr_halt.saturating_add(1)}
            if isr&HISR0_WDT_TIMEOUT!=0{e.hisr_wdt=e.hisr_wdt.saturating_add(1)}
            self.halts=self.halts.saturating_add(1);self.halt_status=Some(HaltStatus{count:self.halts,hisr:isr,ctrl,ser,intx});
        }
        let Some(raw)=raw else{return Ok(())};
        let(code,name,level,event)=firmware_error(raw);let e=&mut self.counts.errors;
        match level{Level::L0=>e.l0=e.l0.saturating_add(1),Level::L1=>e.l1=e.l1.saturating_add(1),Level::L2=>e.l2=e.l2.saturating_add(1),
            Level::Unknown=>e.unknown=e.unknown.saturating_add(1)}
        let count=e.l0.saturating_add(e.l1).saturating_add(e.l2).saturating_add(e.unknown);
        // Kept until taken; beyond FIRMWARE_ERRORS_KEPT only counted, the line's n showing the gap.
        if self.firmware_pending<FIRMWARE_ERRORS_KEPT{
            self.firmware_errors[self.firmware_pending]=FirmwareError{count,ctrl,raw,ser,code,name,level,event};self.firmware_pending+=1;}
        Ok(())
    }
    fn permits(&mut self,deadline:u64)->Result<(),Cause>{let left=self.session.left(deadline)?;self.session.live(left.min(LIVE_US))?;Ok(())}
    fn checks(&mut self,deadline:u64)->Result<(),Cause>{
        let mut window=Window{bus:self.session.bus,deadline,last:self.session.last,clock_bad:false};
        let result=(||{self.queue.validate_data(&mut window)?;
            self.prepared.station.check_registers(&mut window).map_err(Cause::Station)?;
                radio(&mut window,self.prepared.tuned.channel)?;Ok::<_,Cause>(())})();
        self.session.last=window.last;result
    }
    fn deadline(&mut self,timeout_us:u32)->Result<u64,Cause>{
        if timeout_us==0||timeout_us>super::MAX_COMMAND_US{return Err(Cause::Session(super::Error::Request))}
        let own=self.session.now()?.checked_add(u64::from(timeout_us)).ok_or(Cause::Session(super::Error::Clock))?;
        Ok(if OWN_DEADLINES{own}else{self.tx_deadline.map_or(own,|tx|tx.min(own))})
    }
    fn runtime_failure(&mut self,cause:Cause)->Failure{
        let cause=self.session.btc_event_error.map_or(cause,Cause::Coexistence);
        let shutdown=Some(self.shutdown());Failure{stage:Stage::Runtime,cause,shutdown}
    }
    fn request(timeout_us:u32)->Result<(),Failure>{
        if timeout_us==0||timeout_us>super::MAX_COMMAND_US{return Err(Failure{stage:Stage::Runtime,cause:Cause::Session(super::Error::Request),shutdown:None})}Ok(())
    }
    pub(crate) fn command(&mut self,command:h2c::Command,body:&[u8],acks:h2c::Acks,timeout_us:u32)->Result<h2c::Outcome,Failure>{
        Self::request(timeout_us)?;
        if body.len()>h2c::MAX_BODY{return Err(Failure{stage:Stage::Runtime,cause:Cause::Session(super::Error::Request),shutdown:None})}
        let result=(||{let deadline=self.deadline(timeout_us)?;let deadline=self.live(deadline)?;self.reconcile(deadline)?;let deadline=self.outlast_ser(deadline,true)?;
            let mut handler=Dispatcher{queue:self.queue,filter:self.prepared.station.current_filter(),counts:&mut self.counts,error:None};
            let outcome=self.session.command_deadline_with(command,body,acks,deadline,&mut handler)
                .map_err(|e|handler.error.map_or(Cause::Session(e),Cause::Tx))?;
            self.reconcile(deadline)?;self.session.left(deadline)?;Ok(outcome)})();
        result.map_err(|cause|self.runtime_failure(cause))
    }
    pub(crate) fn poll_packets(&mut self,timeout_us:u32)->Result<[bool;2],Failure>{
        Self::request(timeout_us)?;
        let result=(||{let deadline=self.deadline(timeout_us)?;let deadline=self.live(deadline)?;self.reconcile(deadline)?;let deadline=self.outlast_ser(deadline,true)?;
            let mut handler=Dispatcher{queue:self.queue,filter:self.prepared.station.current_filter(),counts:&mut self.counts,error:None};
            let outcome=self.session.poll_before_with(deadline,&mut handler)
                .map_err(|e|handler.error.map_or(Cause::Session(e),Cause::Tx))?;
            self.reconcile(deadline)?;self.session.left(deadline)?;Ok(outcome)})();
        result.map_err(|cause|self.runtime_failure(cause))
    }
    /// A poll without the liveness checks and the coexistence reconcile, for the polls that follow a `poll_packets`
    /// within the service's CHECK_MS: it made them, and the next makes them again. They cost six times the receive, and
    /// made for every packet they left the driver no time on a crowded channel, whose foreign frames took 0.9 ms each,
    /// about a thousand a second (the Ryzen, 2026-11-21, 213-DRV-0032). `rxq` false: the release reports alone.
    pub(crate) fn poll_more(&mut self,rxq:bool,timeout_us:u32)->Result<[bool;2],Failure>{
        Self::request(timeout_us)?;
        let result=(||{let deadline=self.deadline(timeout_us)?;
            let mut handler=Dispatcher{queue:self.queue,filter:self.prepared.station.current_filter(),counts:&mut self.counts,error:None};
            let outcome=self.session.poll_some(deadline,&mut handler,false,rxq)
                .map_err(|e|handler.error.map_or(Cause::Session(e),Cause::Tx))?;
            self.session.left(deadline)?;Ok(outcome)})();
        result.map_err(|cause|self.runtime_failure(cause))
    }
    pub(crate) fn begin(&mut self,frame:&[u8],rate:u8,max_submissions:u8,timeout_us:u32)->Result<(),Failure>{
        Self::request(timeout_us)?;
        if self.queue.pending_request(){return Err(Failure{stage:Stage::Runtime,cause:Cause::Tx(tx::Error::Busy),shutdown:None})}
        if frame.get(10..16)!=self.prepared.station.current_filter().map(|f|f.own()).as_ref().map(|v|v.as_slice()){
            return Err(Failure{stage:Stage::Runtime,cause:Cause::Tx(tx::Error::Input(txdesc::Error::Address)),shutdown:None})}
        if let Err(e)=txdesc::frame(frame,rate,max_submissions){return Err(Failure{stage:Stage::Runtime,cause:Cause::Tx(tx::Error::Input(e)),shutdown:None})}
        let result=(||{let deadline=self.deadline(timeout_us)?;let deadline=self.live(deadline)?;self.reconcile(deadline)?;let deadline=self.outlast_ser(deadline,true)?;let left=self.session.left(deadline)?;
            let mut window=Window{bus:self.session.bus,deadline,last:self.session.last,clock_bad:false};
            let result=self.queue.begin(&mut window,frame,rate,max_submissions,self.prepared.station.mac_id(),left);
            self.session.last=window.last;result?;self.session.left(deadline)?;self.tx_deadline=Some(deadline);Ok(())})();
        result.map_err(|cause|self.runtime_failure(cause))
    }
    pub(crate) fn advance(&mut self,timeout_us:u32)->Result<Option<tx::Completion>,Failure>{
        Self::request(timeout_us)?;
        let result=(||{let deadline=self.deadline(timeout_us)?;let deadline=self.live(deadline)?;self.reconcile(deadline)?;let deadline=self.outlast_ser(deadline,true)?;if !self.queue.pending_request(){return Ok(None)}
            let mut handler=Dispatcher{queue:self.queue,filter:self.prepared.station.current_filter(),counts:&mut self.counts,error:None};
            self.session.poll_before_with(deadline,&mut handler).map_err(|e|handler.error.map_or(Cause::Session(e),Cause::Tx))?;
            self.reconcile(deadline)?;
            let mut window=Window{bus:self.session.bus,deadline,last:self.session.last,clock_bad:false};
            let result=self.queue.advance(&mut window);self.session.last=window.last;
            let outcome=result?;self.session.left(deadline)?;if outcome.is_some(){self.tx_deadline=None;}Ok(outcome)})();
        result.map_err(|cause|self.runtime_failure(cause))
    }
    /// RPQ_ALONE in a host model (tx.rs), whose default is the strict path.
    #[cfg(test)]
    pub(crate) fn rpq_alone(&mut self){self.queue.rpq_alone()}
    /// 213-DRV-0104 (SEND_BEHIND_LATE): the pending send, answered late, parked on the ring so that the next can begin
    /// behind it; its release is awaited by its sequence and it is not resubmitted. Refused for a frame that cannot be.
    pub(crate) fn park(&mut self)->Result<(),Failure>{
        self.queue.park().map_err(|e|Failure{stage:Stage::Runtime,cause:Cause::Tx(e),shutdown:None})?;self.tx_deadline=None;Ok(())
    }
    /// 213-DRV-0104: the parked frame's release, if it came (the rings are read by the steps' polls, as for any send).
    /// With a send pending, `advance` made the checks and the reconcile this step, and only a recovery is looked for.
    pub(crate) fn parked_release(&mut self,timeout_us:u32)->Result<Option<tx::Completion>,Failure>{
        Self::request(timeout_us)?;
        let result=(||{let deadline=self.deadline(timeout_us)?;
            let deadline=if self.queue.pending_request(){deadline}else{let d=self.live(deadline)?;self.reconcile(d)?;d};
            let deadline=self.outlast_ser(deadline,true)?;
            if !self.queue.parked(){return Ok(None)}
            let mut window=Window{bus:self.session.bus,deadline,last:self.session.last,clock_bad:false};
            let result=self.queue.parked_release(&mut window);self.session.last=window.last;
            let outcome=result?;self.session.left(deadline)?;Ok(outcome)})();
        result.map_err(|cause|self.runtime_failure(cause))
    }
    pub(crate) fn channel(&self)->u8{self.prepared.tuned.channel}
    pub(crate) fn mac(&self)->[u8;6]{self.prepared.board.identity.mac}
    pub(crate) fn current_filter(&self)->station::Filter{self.prepared.station.current_filter().unwrap()}
    /// The station's filter, None if a failed change left none (the next operation then refuses).
    pub(crate) fn filter(&self)->Option<station::Filter>{self.prepared.station.current_filter()}
    pub(crate) fn counts(&self)->Counts{Counts{btc_events:self.counts.btc_events.saturating_add(self.session.counts.btc_events),ppdu_reports:self.session.counts.ppdu_reports,
        signals:self.session.signals(),..self.counts}}
    /// The oldest inbox record is a frame whose PPDU status report may still follow (213-DRV-0087).
    pub(crate) fn awaits_signal(&self)->bool{self.session.awaits_signal()}
    pub(crate) fn packet(&self)->Option<inbox::Record<'_>>{self.session.packet()}
    pub(crate) fn consume_packet(&mut self){self.session.consume_packet()}
    pub(crate) fn terminal_packet(&self)->Option<inbox::Record<'_>>{self.session.terminal_packet()}
    pub(crate) fn pending_packets(&self)->usize{self.session.pending_packets()}
    /// The registers the first shutdown found (SNAPSHOT), if it ran on an activated data mode.
    pub(crate) fn snapshot(&self)->Option<Snapshot>{self.snapshot}
    /// What the pending send has seen (a timeout's diagnostic).
    pub(crate) fn transmit_seen(&self)->tx::Seen{self.queue.seen()}
    /// More time for the pending send (service.rs's KEEP_LATE).
    pub(crate) fn extend_send(&mut self,us:u64){self.queue.extend(us);self.tx_deadline=self.tx_deadline.map(|at|at.saturating_add(us));}
    /// The time outlasted recoveries added to pending frames so far (OUTLAST_SER).
    pub(crate) fn grace_us(&self)->u64{self.grace_us}
    /// The firmware recovery outlasted since the last call, if any (OUTLAST_SER).
    pub(crate) fn take_recovery(&mut self)->Option<Recovery>{self.recovery.take()}
    /// The last station check that met an unreadable CMAC domain and went on (213-DRV-0091), once.
    pub(crate) fn take_unreadable(&mut self)->Option<Unreadable>{self.unreadable.take()}
    /// The last check that found HISR0's halt bits set, not yet taken (213-DRV-0091).
    pub(crate) fn take_halt_status(&mut self)->Option<HaltStatus>{self.halt_status.take()}
    /// The last step of SER_DBG_INFO a check saw, not yet taken (213-DRV-0091).
    pub(crate) fn take_ser_step(&mut self)->Option<SerStep>{self.ser_step.take()}
    /// RXQ's and RPQ's polls since the last minute line, taken (213-DRV-0091).
    pub(crate) fn take_polls(&mut self)->[crate::rx::Polls;2]{self.session.rings.take_polls()}
    /// The oldest firmware error code read and not yet taken (213-DRV-0091).
    pub(crate) fn take_firmware_error(&mut self)->Option<FirmwareError>{
        if self.firmware_pending==0{return None}
        let first=self.firmware_errors[0];self.firmware_errors.copy_within(1..,0);self.firmware_pending-=1;Some(first)
    }
    /// The SNAPSHOT registers now, read only.
    pub(crate) fn registers(&mut self)->Snapshot{
        let bus=&mut *self.session.bus;Snapshot(SNAPSHOT.map(|at|io::read32(bus,at).unwrap_or(u32::MAX)))
    }
    /// PROBE's registers as they are now (213-DRV-0090).
    /// 213-DRV-0091: HELD_MAC and HELD_BB, read directly.
    pub(crate) fn held(&mut self)->Held{
        let bus=&mut *self.session.bus;
        Held{mac:HELD_MAC.map(|at|io::read32(bus,at).unwrap_or(u32::MAX)),bb:HELD_BB.map(|at|io::read32(bus,0x10000+at).unwrap_or(u32::MAX))}
    }
    /// 213-DRV-0102: the detection floor's four BB registers (detection_floor::REGISTERS order).
    pub(crate) fn floor_registers(&mut self)->[u32;4]{
        let bus=&mut *self.session.bus;
        FLOOR_REGISTERS.map(|at|io::read32(bus,0x10000+at).unwrap_or(u32::MAX))
    }
    /// 213-DRV-0102: fields of the detection floor written: (register, mask, value), every one, each read back on its own.
    /// The registers (bits in FLOOR_REGISTERS' order) whose field reads otherwise, and of those the ones that read as
    /// before the write; only an access fault ends the writes (boot 93: one field that did not read back left the rest).
    pub(crate) fn write_floor(&mut self,fields:&[(usize,u32,u32)])->io::Result<FloorWrite>{
        let bus=&mut *self.session.bus;let mut done=FloorWrite::default();
        for &(at,mask,value) in fields{
            let before=io::read32(bus,0x10000+at)?;bus.write32(0x10000+at,(before&!mask)|(value&mask))?;
            let after=io::read32(bus,0x10000+at)?;
            if after&mask!=value&mask{
                let bit=FLOOR_REGISTERS.iter().position(|r|*r==at).map_or(0,|i|1u8<<i);
                done.unread|=bit;if after&mask==before&mask{done.unchanged|=bit;}
            }
        }
        Ok(done)
    }
    /// 213-DRV-0102: the minute's PPDU reports by signal, those with no RPL, the unparsed and the header over the RPL,
    /// taken: the next minute's start at zero.
    pub(crate) fn take_ppdu_rssi(&mut self)->([u32;17],[u32;17],u32,(i64,u32)){
        let c=&mut self.counts;
        (core::mem::take(&mut c.ppdu_rssi),core::mem::take(&mut c.ppdu_no_rpl),core::mem::take(&mut c.ppdu_unparsed),core::mem::take(&mut c.ofdm_header_over))
    }
    /// 213-DRV-0091: port 0's TSF, its high word read before and after the low one.
    pub(crate) fn tsf(&mut self)->Option<u64>{
        let bus=&mut *self.session.bus;
        let high=io::read32(bus,0xc43c).ok()?;let low=io::read32(bus,0xc438).ok()?;
        (io::read32(bus,0xc43c).ok()?==high).then_some(u64::from(high)<<32|u64::from(low))
    }
    pub(crate) fn probe(&mut self)->Probe{
        let bus=&mut *self.session.bus;
        let mac=PROBE_MAC.map(|at|io::read32(bus,at).unwrap_or(u32::MAX));
        let bb=PROBE_BB.map(|at|io::read32(bus,0x10000+at).unwrap_or(u32::MAX));
        let rf=[0u8,1].map(|path|PROBE_RF.map(|at|crate::phy::rf_read(bus,path,at,0xfffff).unwrap_or(u32::MAX)));
        let grants=crate::btc_control::lte_grants(bus).unwrap_or(u32::MAX);
        Probe{mac,bb,rf,grants}
    }
    /// Independently stop radiation and owned packet DMA; caller still verifies kernel PCI quiescence and keeps all memory.
    pub(crate) fn shutdown(&mut self)->Shutdown{
        self.stopped=true;let _=self.session.fail::<()>(super::Error::Session);
        if let Some(previous)=self.last_shutdown{return previous}
        if !self.cleanup_allowed{self.last_shutdown=Some(Shutdown::NotActivated);return Shutdown::NotActivated}
        self.snapshot=Some(self.registers());
        let radio=runtime_mac::disable_radio(self.session.bus).err();let transmit=self.queue.stop(self.session.bus);
        let result=Shutdown::Attempted{radio,transmit};self.last_shutdown=Some(result);result
    }
}
impl<B:DeviceIo> Drop for DataSession<'_,'_,'_,'_,'_,'_,B>{
    fn drop(&mut self){let _=self.session.fail::<()>(super::Error::Session);self.queue.poison();}
}
fn require(bus:&mut impl RegisterIo,at:usize,mask:u32,wanted:u32)->io::Result<()>{
    let actual=io::read32(bus,at)?;if actual&mask!=wanted{return Err(Fault::new(FaultKind::PowerState,at,actual))}Ok(())
}

impl<B:fwcmd::Bus> fwcmd::Bus for Window<'_,B>{
    fn pci_command(&mut self)->io::Result<u16>{self.check()?;let result=self.bus.pci_command()?;self.check()?;Ok(result)}
}

fn radio(bus:&mut impl RegisterIo,channel:u8)->io::Result<()>{
    use crate::registers::*;
    let bb=bus.read8(R_AX_SYS_FUNC_EN)?;
    if bb==u8::MAX||bb&3!=3{return Err(Fault::new(FaultKind::PowerState,R_AX_SYS_FUNC_EN,u32::from(bb)))}
    require(bus,R_AX_SPS_DIG_ON_CTRL0,B_AX_REG_ZCDC_H_MASK,1<<17)?;
    require(bus,R_AX_WLRF_CTRL,B_AX_AFC_AFEDIG,B_AX_AFC_AFEDIG)?;
    let cycle=bus.read8(0x8040)?;
    if cycle==u8::MAX||cycle&15!=15{return Err(Fault::new(FaultKind::PowerState,0x8040,u32::from(cycle)))}
    require(bus,crate::phy::PHY_BASE+0x734,0x00ff0000,crate::phy::channel::channel_index(channel)<<16)?;
    // RF18's direct copy reads back only its CH field on the Ryzen (2026-11-04); on 5 GHz RF18 has its band fields too.
    let band=if channel>14{0x10100}else{0};
    for path in 0..2{for (at,mask,expected) in [(0x18,0x3ffff,0x1c00|band|u32::from(channel)),(0x10018,255,u32::from(channel))]{
        let actual=crate::phy::rf_read(bus,path,at,mask)?;
        if actual!=expected{return Err(Fault::new(FaultKind::PowerState,at as usize,actual))}
    }}Ok(())
}

#[path="active_control.rs"]mod control;
pub(crate) use control::FilterChange;

#[path="runtime_control.rs"]mod coexistence;
