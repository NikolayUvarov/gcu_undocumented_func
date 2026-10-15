use super::*;
use std::alloc::{alloc_zeroed, dealloc, Layout};

struct Region { base: *mut u8, layout: Layout }
impl Region {
    fn new() -> Self {
        let layout = Layout::from_size_align(fwcmd::DMA_BYTES, 4096).unwrap();
        let base = unsafe { alloc_zeroed(layout) };
        assert!(!base.is_null());
        Self { base, layout }
    }
    fn queue<'a>(&'a mut self, bus: &mut Device) -> Queue<'a> {
        bus.dma = self.base;
        let mut queue = unsafe { Queue::new(bus, self.base, fwcmd::DMA_BYTES, 0x100000).unwrap() };
        queue.install(bus).unwrap();
        bus.command = 6;
        bus.set(fwcmd::INIT_CONFIG, 1 << 11);
        bus.set(fwcmd::DMA_STOP, DATA_STOP);
        unsafe { queue.start(bus).unwrap(); }
        bus.writes.clear();
        queue
    }
}
impl Drop for Region { fn drop(&mut self) { unsafe { dealloc(self.base, self.layout); } } }

struct Device {
    regs: Vec<u8>, dma: *mut u8, command: u16, time: u64, frozen: bool,
    writes: Vec<(usize, u32)>, packets: Vec<(bool, Vec<u8>)>, bytes: usize,
    fail_write: Option<usize>, ignored_write: Option<usize>, slow_final: bool,
    no_h2c: bool, no_fwdl: bool, status: u8, hold: bool, hold_until: u64, slow_drain: bool,
    status_reads: usize, busy_after_error: bool, cpu_stuck_after_error: bool, error_applied: bool,
}
impl Device {
    fn new() -> Self {
        let mut d = Self { regs: vec![0; 0xc004], dma: core::ptr::null_mut(), command: 2, time: 0, frozen: false,
            writes: Vec::new(), packets: Vec::new(), bytes: 0, fail_write: None, ignored_write: None,
            slow_final: false, no_h2c: false, no_fwdl: false, status: 7, hold: false, hold_until: 0,
            slow_drain: false, status_reads: 0, busy_after_error: false, cpu_stuck_after_error: false, error_applied: false };
        for (at,value) in [(SYS_CFG1,1 << 12), (R_AX_IC_PWR_STATE,1 << 8), (R_AX_PLATFORM_ENABLE,5),
            (R_AX_CMAC_FUNC_EN,0x7000803f), (R_AX_DMAC_FUNC_EN,0x64c40000), (mac::DMAC_CLOCK,0x04840000), (mac::PLE_CONFIG,0x00400801),
            (mac::WDE_READY,3), (mac::PLE_READY,3), (mac::HFC_CONTROL,8), (mac::CHANNEL_PAGE,40 << 16),
            (fwcmd::DMA_STOP,ALL_STOP)] { d.set(at,value); }
        d
    }
    fn get(&self, at: usize) -> u32 { u32::from_le_bytes(self.regs[at..at+4].try_into().unwrap()) }
    fn set(&mut self, at: usize, value: u32) { self.regs[at..at+4].copy_from_slice(&value.to_le_bytes()); }
    fn apply(&mut self, at: usize, value: u32, width: usize) -> io::Result<()> {
        self.writes.push((at,value));
        let previous = self.get(at & !3);
        let ignored = self.ignored_write == Some(self.writes.len())
            || (self.error_applied && self.cpu_stuck_after_error && at == R_AX_PLATFORM_ENABLE && value & WCPU == 0);
        if !ignored { self.regs[at..at+width].copy_from_slice(&value.to_le_bytes()[..width]); }
        if at == fwcmd::CLEAR_INDEX { self.set(fwcmd::INDEX,0); }
        if at == R_AX_PLATFORM_ENABLE && value & WCPU != 0 && previous & WCPU == 0 && !self.no_h2c {
            self.regs[FW_CONTROL] |= 2;
        }
        if at == fwcmd::INDEX {
            assert_eq!(width,2);
            let descriptor = ((value as usize + 31) % 32) * 8;
            let (size,address) = unsafe {
                (self.dma.add(descriptor).cast::<u32>().read_unaligned().to_le(),
                 self.dma.add(descriptor+4).cast::<u32>().read_unaligned().to_le())
            };
            assert_ne!(size & (1<<30),0);
            let offset = address as usize - 0x100000;
            let txwd = unsafe { self.dma.add(offset).cast::<u32>().read_unaligned().to_le() };
            assert_eq!((txwd >> 16) & 15,12);
            let part = txwd & (1<<20) != 0;
            let length = (size & 0xffff) as usize - fwcmd::TXWD_BYTES;
            let packet = unsafe { core::slice::from_raw_parts(self.dma.add(offset + fwcmd::TXWD_BYTES),length) }.to_vec();
            if part { self.bytes += packet.len(); }
            else if !self.no_fwdl { self.regs[FW_CONTROL] |= 4; }
            self.packets.push((part,packet));
            if !self.hold && self.time >= self.hold_until { self.set(fwcmd::INDEX,value | value << 16); }
            if self.bytes == 326_472 { self.regs[FW_CONTROL] = (self.regs[FW_CONTROL] & !0xe0) | self.status << 5; }
        }
        if self.fail_write == Some(self.writes.len()) {
            self.error_applied = true;
            if self.busy_after_error { self.set(fwcmd::DMA_BUSY,1 << 18); }
            return Err(Fault::new(FaultKind::Access,at,value));
        }
        Ok(())
    }
    fn stopped(&self) {
        assert!(matches!(self.command,6|7));
        assert_eq!(self.get(R_AX_PLATFORM_ENABLE) & WCPU,0);
        assert_eq!(self.get(fwcmd::INIT_CONFIG) & (1<<11|1<<13),0);
        assert_eq!(self.get(fwcmd::DMA_STOP) & ALL_STOP,ALL_STOP);
        assert_eq!(self.regs[R_AX_SYS_FUNC_EN] & 3,0);
    }
}
impl RegisterIo for Device {
    fn read8(&mut self, at: usize) -> io::Result<u8> {
        if at == FW_CONTROL {
            self.status_reads += 1;
            if self.slow_final && self.regs[at] & 0xe0 == 0xe0 { self.time += STAGE_US; }
        }
        Ok(self.regs[at])
    }
    fn read16(&mut self, at: usize) -> io::Result<u16> { Ok(u16::from_le_bytes(self.regs[at..at+2].try_into().unwrap())) }
    fn read32(&mut self, at: usize) -> io::Result<u32> {
        if at == fwcmd::INDEX && self.slow_drain && self.bytes == 326_472 { self.time += PATH_US as u64; }
        if at == fwcmd::INDEX && self.time >= self.hold_until && !self.hold {
            let index = self.get(at) & 0xfff;
            self.set(at,index | index << 16);
        }
        Ok(self.get(at))
    }
    fn write8(&mut self, at: usize, value: u8) -> io::Result<()> { self.apply(at,value as u32,1) }
    fn write16(&mut self, at: usize, value: u16) -> io::Result<()> { self.apply(at,value as u32,2) }
    fn write32(&mut self, at: usize, value: u32) -> io::Result<()> { self.apply(at,value,4) }
    fn now_us(&mut self) -> u64 { self.time }
    fn delay_us(&mut self, us: u32) -> io::Result<()> { assert!(us <= 1000); if !self.frozen { self.time += us as u64; } Ok(()) }
}
impl Bus for Device { fn pci_command(&mut self) -> io::Result<u16> { Ok(self.command) } }

fn board() -> Board {
    Board { identity: crate::board::Identity { cut:1,rfe:1,mac:[2,1,2,3,4,5],xtal:1,country:*b"US",channel_plan:1 },
        maps: crate::efuse::Maps { logical:[0xff;2048],dav_logical:[0xff;16],phycap:[0xff;128] } }
}

// Synthetic data is admitted only through this private test module, never the production SHA-256 policy.
fn synthetic_file() -> Vec<u8> {
    let mut file = vec![0; FILE_BYTES];
    let w = |bytes: &mut [u8],at: usize,value: u32| bytes[at..at+4].copy_from_slice(&value.to_le_bytes());
    file[0] = 0xff; file[1] = 1; file[16] = 1; file[17] = 5;
    w(&mut file,20,96); w(&mut file,24,326616);
    let image = &mut file[96..96+326616];
    image[4..8].copy_from_slice(&VERSION); w(image,8,0x6fb3ec41); w(image,12,144<<16);
    w(image,24,3<<8); w(image,28,1<<16|2020); w(image,80,64);
    let mut at = 144;
    for (i,&(kind,address,length)) in SECTIONS.iter().enumerate() {
        w(image,32+i*16,address|0xa0000000); w(image,36+i*16,(kind as u32)<<24|length as u32);
        for (n,byte) in image[at..at+length].iter_mut().enumerate() { *byte = ((n*7+i*53)%251) as u8; }
        at += length;
    }
    file
}
fn fixture(file: &[u8]) -> Verified<'_> { Verified::parse_pinned(file).unwrap() }

pub(crate) fn with_downloaded_queue(check: impl FnOnce(Ready, &mut Queue<'_>)) {
    let file=synthetic_file(); let mut region=Region::new(); let mut bus=Device::new(); let mut queue=region.queue(&mut bus);
    let ready=run(&mut bus,&mut queue,fixture(&file)).unwrap();
    assert_eq!(ready.session(),queue.session());
    check(ready,&mut queue);
    assert_eq!(stop(&mut bus,&mut queue),Stop{dma:None,cpu:None});
    bus.command=0;
}

#[test]
fn public_policy_rejects_unpinned_bytes_and_unknown_board_security() {
    let mut b = board(); let file = synthetic_file();
    assert_eq!(Verified::verify(&b,&file).unwrap_err(),VerifyError::Hash);
    assert_eq!(Verified::verify(&b,&file[..file.len()-1]).unwrap_err(),VerifyError::Length);
    for value in [[0xff,0], [0,0xff], [0,0], [0x56,0x78]] {
        b.maps.phycap[0x6c..0x6e].copy_from_slice(&value);
        assert_eq!(Verified::verify(&b,&file).unwrap_err(),VerifyError::Security);
    }
    b.maps.phycap[0x6c..0x6e].fill(0xff); b.identity.cut=2;
    assert_eq!(Verified::verify(&b,&file).unwrap_err(),VerifyError::Board);
}

#[test]
fn pinned_metadata_rejects_changed_version_addresses_and_section_sizes() {
    for (at,value) in [(96+4,1),(96+8,0),(96+32,1),(96+36,1)] {
        let mut file=synthetic_file(); file[at]=value;
        assert!(Verified::parse_pinned(&file).is_err());
    }
}

#[test]
fn exact_header_and_all_section_chunks_cross_ring_wraps_then_require_ready_status() {
    let file=synthetic_file(); let fw=fixture(&file); let header=fw.header();
    assert_eq!(&header[..8], &[13,0,0,0,88,0,0,0]);
    assert_eq!(&header[8..36],&file[96..124]);
    assert_eq!(&header[36..38],&2020u16.to_le_bytes());
    assert_eq!(&header[38..],&file[126..176]);
    let mut region=Region::new(); let mut bus=Device::new(); let mut queue=region.queue(&mut bus);
    let ready=run(&mut bus,&mut queue,fw).unwrap();
    assert_eq!(ready.version(),VERSION); assert_eq!(ready.session(),queue.session());
    assert_eq!(bus.packets[0],(false,header.to_vec()));
    let mut packet=1;
    for section in fixture(&file).image.sections() {
        for bytes in section.bytes.chunks(2020) { assert_eq!(bus.packets[packet],(true,bytes.to_vec())); packet+=1; }
    }
    assert_eq!(packet,bus.packets.len()); assert!(packet>160);
    assert_eq!(queue.pending(),0); assert_eq!(queue.retained(),8); assert_eq!(queue.state(),fwcmd::State::Running);
    assert_eq!(bus.get(fwcmd::DMA_STOP),DATA_STOP); assert_eq!(bus.command,6);
    assert_eq!(bus.regs[R_AX_SYS_FUNC_EN]&3,0); assert_eq!(bus.get(R_AX_CMAC_FUNC_EN),0x7000803f); assert_eq!(bus.time,5000);
    let cpu_start=bus.writes.iter().position(|&(at,v)| at==R_AX_PLATFORM_ENABLE&&v&WCPU!=0).unwrap();
    let first_packet=bus.writes.iter().position(|&(at,_)| at==fwcmd::INDEX).unwrap(); assert!(cpu_start<first_packet);
}

#[test]
fn changed_preconditions_and_previously_used_queue_cause_no_device_writes() {
    let file=synthetic_file();
    for (at,value) in [(SYS_CFG1,2<<12),(R_AX_PLATFORM_ENABLE,7),(R_AX_DMAC_FUNC_EN,0),(mac::HFC_CONTROL,1),
        (fwcmd::BASE_LOW,0x200000),(fwcmd::DMA_STOP,0),(fwcmd::INIT_CONFIG,1<<11|1<<13),(fwcmd::DMA_BUSY,1)] {
        let mut region=Region::new(); let mut bus=Device::new(); let mut queue=region.queue(&mut bus); bus.set(at,value);
        let error=run(&mut bus,&mut queue,fixture(&file)).unwrap_err();
        assert_eq!(error.stage,Stage::Guard); assert!(bus.writes.is_empty()); assert_eq!(queue.state(),fwcmd::State::Running);
    }
    let mut region=Region::new(); let mut bus=Device::new(); let mut queue=region.queue(&mut bus);
    queue.submit(&mut bus,b"prior H2C",false).unwrap(); queue.reclaim(&mut bus).unwrap(); bus.writes.clear();
    assert_eq!(run(&mut bus,&mut queue,fixture(&file)).unwrap_err().stage,Stage::Guard); assert!(bus.writes.is_empty());
}

#[test]
fn inaccessible_pci_command_is_rejected_before_register_mutation() {
    let file=synthetic_file(); let mut region=Region::new(); let mut bus=Device::new(); let mut queue=region.queue(&mut bus);
    bus.command=0xffff; assert_eq!(run(&mut bus,&mut queue,fixture(&file)).unwrap_err().stage,Stage::Guard); assert!(bus.writes.is_empty());
}

#[test]
fn every_applied_forward_write_error_poisoned_the_queue_and_attempted_independent_cleanup() {
    let file=synthetic_file(); let mut region=Region::new(); let mut bus=Device::new(); let mut queue=region.queue(&mut bus);
    run(&mut bus,&mut queue,fixture(&file)).unwrap(); let count=bus.writes.len(); drop(queue);
    for fail in 1..=count {
        let mut region=Region::new(); let mut bus=Device::new(); let mut queue=region.queue(&mut bus); bus.fail_write=Some(fail);
        let error=run(&mut bus,&mut queue,fixture(&file)).unwrap_err();
        assert!(matches!(error.cause,Cause::Io(Fault{kind:FaultKind::Access,..})|Cause::Transport(fwcmd::Error::Io(Fault{kind:FaultKind::Access,..}))),"{fail}: {error:?}");
        assert_eq!(error.dma_stop,None,"{fail}: {error:?}"); assert_eq!(error.cpu_stop,None,"{fail}: {error:?}");
        assert_eq!(queue.state(),fwcmd::State::Faulted); assert_eq!(queue.physical(),0x100000); bus.stopped();
    }
}

#[test]
fn missing_h2c_or_download_path_is_bounded_and_never_sends_sections() {
    let file=synthetic_file();
    for h2c in [true,false] {
        let mut region=Region::new(); let mut bus=Device::new(); let mut queue=region.queue(&mut bus);
        bus.no_h2c=h2c; bus.no_fwdl=!h2c;
        let error=run(&mut bus,&mut queue,fixture(&file)).unwrap_err();
        assert_eq!(error.stage,if h2c {Stage::H2cReady} else {Stage::DownloadReady});
        assert!(matches!(error.cause,Cause::Io(Fault{kind:FaultKind::Timeout,..})));
        assert_eq!(bus.bytes,0); assert!(bus.time<=PATH_US as u64); bus.stopped();
    }
}

#[test]
fn frozen_clock_still_bounds_status_polling() {
    let file=synthetic_file(); let mut region=Region::new(); let mut bus=Device::new(); let mut queue=region.queue(&mut bus);
    bus.no_h2c=true; bus.frozen=true;
    let error=run(&mut bus,&mut queue,fixture(&file)).unwrap_err();
    assert_eq!(error.stage,Stage::H2cReady); assert!(bus.status_reads<=PATH_US as usize+1); bus.stopped();
}

#[test]
fn stalled_ring_has_bounded_backpressure_and_keeps_pending_dma_owned() {
    let file=synthetic_file(); let mut region=Region::new(); let mut bus=Device::new(); let mut queue=region.queue(&mut bus);
    bus.hold=true;
    let error=run(&mut bus,&mut queue,fixture(&file)).unwrap_err();
    assert_eq!(error.stage,Stage::Sections); assert_eq!(queue.pending(),31); assert_eq!(queue.state(),fwcmd::State::Faulted);
    assert_eq!(bus.packets.len(),31); assert!(bus.time<=PATH_US as u64); bus.stopped();
}

#[test]
fn transient_backpressure_reclaims_and_delivers_every_packet_once() {
    let file=synthetic_file(); let mut region=Region::new(); let mut bus=Device::new(); let mut queue=region.queue(&mut bus);
    bus.hold_until=250;
    run(&mut bus,&mut queue,fixture(&file)).unwrap(); assert_eq!(bus.time,5250); assert_eq!(bus.bytes,326472); assert_eq!(queue.pending(),0);
}

#[test]
fn firmware_error_status_preserves_exact_cause_and_stops_cpu_and_dma() {
    let file=synthetic_file();
    for status in [2,3,4] {
        let mut region=Region::new(); let mut bus=Device::new(); let mut queue=region.queue(&mut bus); bus.status=status;
        let error=run(&mut bus,&mut queue,fixture(&file)).unwrap_err();
        assert_eq!(error.stage,Stage::FirmwareReady); assert_eq!(error.cause,Cause::FirmwareStatus(status)); bus.stopped();
    }
}

#[test]
fn final_ready_read_that_crosses_total_deadline_is_not_success() {
    let file=synthetic_file(); let mut region=Region::new(); let mut bus=Device::new(); let mut queue=region.queue(&mut bus); bus.slow_final=true;
    let error=run(&mut bus,&mut queue,fixture(&file)).unwrap_err();
    assert_eq!(error.stage,Stage::FirmwareReady); assert!(matches!(error.cause,Cause::Io(Fault{kind:FaultKind::Timeout,..}))); bus.stopped();
}

#[test]
fn cleanup_failure_does_not_hide_primary_or_release_the_region() {
    let file=synthetic_file(); let mut region=Region::new(); let mut bus=Device::new(); let mut queue=region.queue(&mut bus);
    bus.fail_write=Some(1); bus.busy_after_error=true;
    let error=run(&mut bus,&mut queue,fixture(&file)).unwrap_err();
    assert!(matches!(error.cause,Cause::Io(Fault{kind:FaultKind::Access,..})));
    assert!(matches!(error.dma_stop,Some(Fault{kind:FaultKind::Timeout,..}))); assert_eq!(error.cpu_stop,None);
    assert_eq!(queue.physical(),0x100000); assert_eq!(queue.state(),fwcmd::State::Faulted);
}

#[test]
fn failed_wcpu_cleanup_is_reported_separately_from_download_failure() {
    let file=synthetic_file(); let mut region=Region::new(); let mut bus=Device::new(); let mut queue=region.queue(&mut bus);
    bus.fail_write=Some(21); bus.cpu_stuck_after_error=true;
    let error=run(&mut bus,&mut queue,fixture(&file)).unwrap_err();
    assert_eq!(error.dma_stop,None); assert!(error.cpu_stop.is_some()); assert_eq!(queue.state(),fwcmd::State::Faulted);
}

#[test]
#[ignore = "requires separately fetched proprietary firmware; no payload is committed"]
fn private_pinned_file_passes_real_hash_policy_and_a_mutation_fails() {
    let path=std::env::var("RTW89_FIRMWARE").expect("set RTW89_FIRMWARE to private fetched file");
    let mut file=std::fs::read(path).unwrap(); let b=board();
    assert!(Verified::verify(&b,&file).is_ok()); file[96+144]^=1;
    assert_eq!(Verified::verify(&b,&file).unwrap_err(),VerifyError::Hash);
}

#[test]
fn successful_diagnostic_stop_poisoned_queue_but_retains_completed_buffers() {
    let file=synthetic_file(); let mut region=Region::new(); let mut bus=Device::new(); let mut queue=region.queue(&mut bus);
    run(&mut bus,&mut queue,fixture(&file)).unwrap();
    assert_eq!(stop(&mut bus,&mut queue),Stop{dma:None,cpu:None});
    assert_eq!(queue.state(),fwcmd::State::Faulted); assert_eq!(queue.retained(),8); assert_eq!(queue.physical(),0x100000);
    assert_eq!(queue.submit(&mut bus,b"cannot restart",false),Err(fwcmd::Error::State)); bus.stopped();
}

#[test]
fn last_drain_index_read_must_finish_before_its_own_deadline() {
    let file=synthetic_file(); let mut region=Region::new(); let mut bus=Device::new(); let mut queue=region.queue(&mut bus); bus.slow_drain=true;
    let error=run(&mut bus,&mut queue,fixture(&file)).unwrap_err();
    assert_eq!(error.stage,Stage::Drain);
    assert!(matches!(error.cause,Cause::Transport(fwcmd::Error::Io(Fault{kind:FaultKind::Timeout,..}))));
    assert_eq!(queue.state(),fwcmd::State::Faulted); bus.stopped();
}

#[test]
fn ignored_clock_enable_write_fails_readback_before_starting_wcpu() {
    let file=synthetic_file(); let mut region=Region::new(); let mut bus=Device::new(); let mut queue=region.queue(&mut bus); bus.ignored_write=Some(14);
    let error=run(&mut bus,&mut queue,fixture(&file)).unwrap_err();
    assert_eq!(error.stage,Stage::CpuStart); assert!(bus.packets.is_empty()); bus.stopped();
}

#[test]
fn active_kernel_grant_may_decode_io_bar_without_changing_any_pci_command() {
    let file=synthetic_file(); let mut region=Region::new(); let mut bus=Device::new(); let mut queue=region.queue(&mut bus);
    bus.command=7; run(&mut bus,&mut queue,fixture(&file)).unwrap();
    assert_eq!(bus.command,7); assert_eq!(stop(&mut bus,&mut queue),Stop{dma:None,cpu:None}); bus.stopped();
    for command in 0..6 {
        let mut region=Region::new(); let mut bus=Device::new(); let mut queue=region.queue(&mut bus); bus.command=command;
        assert_eq!(run(&mut bus,&mut queue,fixture(&file)).unwrap_err().stage,Stage::Guard); assert!(bus.writes.is_empty());
    }
}
