use super::*;
use std::alloc::{alloc_zeroed,dealloc,Layout};

struct Region { base:*mut u8, layout:Layout }
impl Region {
    fn new() -> Self { let layout=Layout::from_size_align(fwcmd::DMA_BYTES,4096).unwrap(); let base=unsafe{alloc_zeroed(layout)}; assert!(!base.is_null()); Self{base,layout} }
    fn queue<'a>(&'a mut self,bus:&mut Device)->Queue<'a> {
        let mut queue=unsafe{Queue::new(bus,self.base,fwcmd::DMA_BYTES,0x100000).unwrap()};
        queue.install(bus).unwrap(); bus.command=6; bus.set(fwcmd::INIT_CONFIG,1<<11); bus.set(fwcmd::DMA_STOP,PACKET_STOP);
        unsafe{queue.start(bus).unwrap()}; bus.writes.clear(); queue
    }
}
impl Drop for Region { fn drop(&mut self){ unsafe{dealloc(self.base,self.layout)} } }

struct Device {
    regs:Vec<u8>, xtal:[u8;256], command:u16, time:u64, frozen:bool, writes:Vec<(usize,usize,u32)>,
    fail_write:Option<usize>, ignore_write:Option<usize>, failed:bool, refuse_bb_reset:bool,
    no_reply:bool, response:u32, scheduler_stuck:bool, xtal_stuck:bool, late_final:bool, idle_reads:usize,
    station_stuck:bool, cam_stuck:bool, ba_stuck:bool,
}
impl Device {
    fn new()->Self {
        let mut d=Self{regs:vec![0;BAR_BYTES],xtal:[0;256],command:2,time:0,frozen:false,writes:Vec::new(),
            fail_write:None,ignore_write:None,failed:false,refuse_bb_reset:false,no_reply:false,response:0x204,
            scheduler_stuck:false,xtal_stuck:false,late_final:false,idle_reads:0,station_stuck:false,cam_stuck:false,ba_stuck:false};
        for (at,value) in [(SYS_CFG1,1<<12),(R_AX_IC_PWR_STATE,1<<8),(R_AX_PLATFORM_ENABLE,3),
            (download::FW_CONTROL,0xe0),(R_AX_CMAC_FUNC_EN,0x7000803f),(R_AX_DMAC_FUNC_EN,0x64c40000),
            (mac::DMAC_CLOCK,0x04840000),(mac::PLE_CONFIG,0x00400801),(mac::WDE_READY,3),(mac::PLE_READY,3),
            (mac::HFC_CONTROL,8),(fwcmd::DMA_STOP,0x1f0f03),(R_AX_CTN_TXEN,0xffff),(0x8a9c,446)] {d.set(at,value);}
        d.xtal[0x80]=0xc0;d.xtal[0x81]=0xb8; d
    }
    fn get(&self,at:usize)->u32 {u32::from_le_bytes(self.regs[at..at+4].try_into().unwrap())}
    fn set(&mut self,at:usize,value:u32){self.regs[at..at+4].copy_from_slice(&value.to_le_bytes());}
    fn apply(&mut self,at:usize,width:usize,value:u32)->io::Result<()> {
        self.writes.push((at,width,value));
        let ignored=self.ignore_write==Some(self.writes.len())||(self.failed&&self.refuse_bb_reset&&at==R_AX_SYS_FUNC_EN&&value&3==0);
        if !ignored {
            self.regs[at..at+width].copy_from_slice(&value.to_le_bytes()[..width]);
            if at==fwcmd::CLEAR_INDEX {self.set(fwcmd::INDEX,0);}
            if at==R_AX_WLAN_XTAL_SI_CTRL && !self.xtal_stuck {
                let offset=(value&255)as usize;
                if value>>24&7==XTAL_SI_NORMAL_READ {self.regs[at+1]=self.xtal[offset];}
                else {let mask=(value>>16)as u8;self.xtal[offset]=(self.xtal[offset]&!mask)|((value>>8)as u8&mask);}
                self.regs[at+3]&=!0x80;
            }
            if at==R_AX_H2CREG_CTRL&&value==1&&!self.no_reply {
                assert_eq!(self.get(R_AX_H2CREG_DATA0),0x205); assert_eq!(self.get(R_AX_H2CREG_DATA0+4),0xffff);
                assert_eq!(self.get(R_AX_H2CREG_DATA0+8),0); assert_eq!(self.get(R_AX_H2CREG_DATA0+12),0);
                self.regs[at]=0;self.regs[R_AX_C2HREG_CTRL]=1;self.set(R_AX_C2HREG_DATA0,self.response);
                if !self.scheduler_stuck {self.set(R_AX_CTN_TXEN,0);}
            }
            if at==R_AX_SS_CTRL&&value&1!=0&&!self.station_stuck {self.regs[at+3]|=0x80;}
            if at==R_AX_ADDR_CAM_CTRL&&!self.cam_stuck {self.regs[at+1]&=!1;}
            if at==R_AX_RESPBA_CAM_CTRL&&!self.ba_stuck {self.regs[at]&=!3;}
        }
        if self.fail_write==Some(self.writes.len()){self.failed=true;return Err(Fault::new(FaultKind::Access,at,value));}
        Ok(())
    }
    fn stopped(&self){
        assert_eq!(self.regs[R_AX_SYS_FUNC_EN]&3,0);assert_eq!(self.get(R_AX_WLRF_CTRL)&B_AX_AFC_AFEDIG,0);
        assert_eq!(self.get(fwcmd::DMA_STOP)&0x1f0f03,0x1f0f03);assert_eq!(self.get(fwcmd::INIT_CONFIG)&(1<<11|1<<13),0);
        assert_eq!(self.get(R_AX_PLATFORM_ENABLE)&2,0);
    }
}
impl RegisterIo for Device {
    fn read8(&mut self,at:usize)->io::Result<u8>{Ok(self.regs[at])}
    fn read16(&mut self,at:usize)->io::Result<u16>{Ok(u16::from_le_bytes(self.regs[at..at+2].try_into().unwrap()))}
    fn read32(&mut self,at:usize)->io::Result<u32>{
        if at==R_AX_PTCL_TX_CTN_SEL {self.idle_reads+=1;if self.late_final&&self.idle_reads==2{self.time+=STAGE_US;}}
        Ok(self.get(at))
    }
    fn write8(&mut self,at:usize,value:u8)->io::Result<()>{self.apply(at,1,value as u32)}
    fn write16(&mut self,at:usize,value:u16)->io::Result<()>{self.apply(at,2,value as u32)}
    fn write32(&mut self,at:usize,value:u32)->io::Result<()>{self.apply(at,4,value)}
    fn now_us(&mut self)->u64{self.time}
    fn delay_us(&mut self,us:u32)->io::Result<()>{assert!(us<=1000);if !self.frozen{self.time+=u64::from(us);}Ok(())}
}
impl Bus for Device{fn pci_command(&mut self)->io::Result<u16>{Ok(self.command)}}
fn board()->Board{Board{identity:crate::board::Identity{cut:1,rfe:1,mac:[2,1,2,3,4,5],xtal:1,country:*b"US",channel_plan:1},
    maps:crate::efuse::Maps{logical:[0xff;2048],dav_logical:[0xff;16],phycap:[0xff;128]}}}

#[test]
fn source_scc_geometry_and_quiet_packet_paths_after_preparation(){
    let mut d=Device::new();let mut r=Region::new();let mut q=r.queue(&mut d);
    prepare_inner(&mut d,&mut q,&board(),BAR_BYTES).unwrap();
    assert_eq!(q.state(),fwcmd::State::Running);assert_eq!(d.get(mac::WDE_CONFIG),510<<16);assert_eq!(d.get(mac::PLE_CONFIG),496<<16|4<<8|1);
    let quotas=[(0x8c40,446),(0x8c44,48),(0x8c4c,0),(0x8c50,16)];for (at,n) in quotas{assert_eq!(d.get(at),n|n<<16);}
    assert_eq!(d.get(0x9058),89|229<<16);assert_eq!(d.get(0x8a10),5|341<<16);assert_eq!(d.get(0x8a30),4|342<<16);
    assert_eq!(d.get(R_AX_DMAC_FUNC_EN),0xfffd0000);assert_eq!(d.get(mac::DMAC_CLOCK),0x0f9f0000);
    assert_eq!(d.get(mac::HFC_CONTROL)&0xfff,0x19);assert_eq!(d.get(R_AX_CTN_TXEN)&0xffff,0);
    assert_eq!(d.get(fwcmd::DMA_STOP),PACKET_STOP);assert_eq!(d.get(fwcmd::INIT_CONFIG)&(1<<11|1<<13),1<<11);
    assert_eq!(d.xtal[0x80],0xc7);assert_eq!(d.xtal[0x81],0xc7);assert_eq!(d.regs[0x8040],0xe);
    assert_eq!(d.get(R_AX_RX_FLTR_OPT)&0x3f0000,22<<16);assert_eq!(d.get(R_AX_DLK_PROTECT_CTL&!3)>>16,0x20f0);
    assert!(!d.writes.iter().any(|(at,_,_)|*at==R_AX_CTN_TXEN||*at==R_AX_PTCL_TX_CTN_SEL));
    assert!(!d.writes.iter().any(|(at,_,_)|*at==fwcmd::INDEX||*at==fwcmd::BASE_LOW||*at==fwcmd::CLEAR_INDEX));
    d.stopped_for_drop(&mut q);
}
impl Device{fn stopped_for_drop(&mut self,q:&mut Queue<'_>){let stop=download::stop(self,q);assert!(stop.dma.is_none()&&stop.cpu.is_none());}}

#[test]
fn unknown_board_or_grant_is_read_only(){
    for variant in 0..11 {
        let mut d=Device::new();let mut r=Region::new();let mut q=r.queue(&mut d);let mut b=board();let mut len=BAR_BYTES;
        match variant {0=>b.identity.cut=2,1=>b.identity.rfe=5,2=>b.maps.phycap[0x6c]=0,3=>len-=1,4=>d.command=0xffff,
            5=>d.command=2,6=>d.set(0xe000,1<<30),7=>d.set(download::FW_CONTROL,0),8=>d.regs[R_AX_SYS_FUNC_EN]=3,9=>d.set(fwcmd::DMA_BUSY,1<<18),10=>d.set(R_AX_CMAC_FUNC_EN,0),_=>unreachable!()}
        let error=prepare_inner(&mut d,&mut q,&b,len).unwrap_err();assert_eq!(error.stage,Stage::Guard);assert!(d.writes.is_empty());
        assert_eq!(q.state(),fwcmd::State::Running);
    }
}
#[test]
fn active_io_decode_does_not_replace_memory_and_master_requirement(){
    let mut d=Device::new();let mut r=Region::new();let mut q=r.queue(&mut d);d.command=7;
    prepare_inner(&mut d,&mut q,&board(),BAR_BYTES).unwrap();d.stopped_for_drop(&mut q);
}
#[test]
fn every_possibly_applied_write_failure_disables_radio_and_command_dma(){
    let count={let mut d=Device::new();let mut r=Region::new();let mut q=r.queue(&mut d);prepare_inner(&mut d,&mut q,&board(),BAR_BYTES).unwrap();let n=d.writes.len();d.stopped_for_drop(&mut q);n};
    for fail in 1..=count {
        let mut d=Device::new();let mut r=Region::new();let mut q=r.queue(&mut d);d.fail_write=Some(fail);
        let e=prepare_inner(&mut d,&mut q,&board(),BAR_BYTES).unwrap_err();assert_eq!(e.cause.kind,FaultKind::Access,"write {fail}");
        assert_eq!(q.state(),fwcmd::State::Faulted);assert!(e.radio_stop.is_none(),"write {fail}: {e:?}");
        let stop=e.command_stop.unwrap();assert!(stop.dma.is_none()&&stop.cpu.is_none());d.stopped();
    }
}
#[test]
fn mismatched_or_malformed_mailbox_report_never_enables_baseband(){
    for response in [0x203,0x004,0x504,0xffff_ffff] {
        let mut d=Device::new();let mut r=Region::new();let mut q=r.queue(&mut d);d.response=response;
        let e=prepare_inner(&mut d,&mut q,&board(),BAR_BYTES).unwrap_err();assert_eq!(e.stage,Stage::PacketStop);
        assert!(!d.writes.iter().any(|(at,_,v)|*at==R_AX_SYS_FUNC_EN&&v&3!=0));d.stopped();
    }
}
#[test]
fn occupied_mailbox_is_not_overwritten_or_retried(){
    for at in [R_AX_H2CREG_CTRL,R_AX_C2HREG_CTRL]{
        let mut d=Device::new();let mut r=Region::new();let mut q=r.queue(&mut d);d.regs[at]=1;
        let e=prepare_inner(&mut d,&mut q,&board(),BAR_BYTES).unwrap_err();assert_eq!(e.stage,Stage::PacketStop);
        assert!(!d.writes.iter().any(|(at,_,_)|(0x8140..=0x8164).contains(at)));d.stopped();
    }
}
#[test]
fn missing_reply_and_frozen_clock_are_bounded_without_duplicate_command(){
    for frozen in [false,true] {
        let mut d=Device::new();let mut r=Region::new();let mut q=r.queue(&mut d);d.no_reply=true;d.frozen=frozen;
        let e=prepare_inner(&mut d,&mut q,&board(),BAR_BYTES).unwrap_err();assert_eq!(e.stage,Stage::PacketStop);assert_eq!(e.cause.kind,FaultKind::Timeout);
        assert_eq!(d.writes.iter().filter(|(at,_,_)|*at==R_AX_H2CREG_CTRL).count(),1);d.stopped();
    }
}
#[test]
fn ack_requires_actual_scheduler_and_protocol_idle(){
    for busy in [false,true]{
        let mut d=Device::new();let mut r=Region::new();let mut q=r.queue(&mut d);
        if busy{d.set(R_AX_PTCL_TX_CTN_SEL,0x80);}else{d.scheduler_stuck=true;}
        let e=prepare_inner(&mut d,&mut q,&board(),BAR_BYTES).unwrap_err();assert_eq!(e.stage,Stage::PacketStop);d.stopped();
    }
}
#[test]
fn diagnostic_counter_nibbles_wrap_independently(){
    let mut d=Device::new();let mut r=Region::new();let mut q=r.queue(&mut d);d.regs[R_AX_UDM1+1]=0xff;
    prepare_inner(&mut d,&mut q,&board(),BAR_BYTES).unwrap();assert_eq!(d.regs[R_AX_UDM1+1],0);d.stopped_for_drop(&mut q);
}
#[test]
fn ignored_geometry_write_is_not_success(){
    let index={let mut d=Device::new();let mut r=Region::new();let mut q=r.queue(&mut d);prepare_inner(&mut d,&mut q,&board(),BAR_BYTES).unwrap();let n=d.writes.iter().position(|(at,_,_)|*at==mac::WDE_CONFIG).unwrap()+1;d.stopped_for_drop(&mut q);n};
    let mut d=Device::new();let mut r=Region::new();let mut q=r.queue(&mut d);d.ignore_write=Some(index);
    let e=prepare_inner(&mut d,&mut q,&board(),BAR_BYTES).unwrap_err();assert_eq!(e.stage,Stage::Memory);d.stopped();
}
#[test]
fn late_final_read_cannot_report_prepared(){
    let mut d=Device::new();let mut r=Region::new();let mut q=r.queue(&mut d);d.late_final=true;
    let e=prepare_inner(&mut d,&mut q,&board(),BAR_BYTES).unwrap_err();assert_eq!(e.stage,Stage::Verify);assert_eq!(e.cause.kind,FaultKind::Timeout);d.stopped();
}
#[test]
fn scheduler_cam_and_ba_readiness_timeouts_remain_failures(){
    for variant in 0..3{
        let mut d=Device::new();let mut r=Region::new();let mut q=r.queue(&mut d);
        match variant{0=>d.station_stuck=true,1=>d.cam_stuck=true,_=>d.ba_stuck=true}
        let e=prepare_inner(&mut d,&mut q,&board(),BAR_BYTES).unwrap_err();assert_eq!(e.cause.kind,FaultKind::Timeout);d.stopped();
    }
}
#[test]
fn occupied_xtal_during_enable_is_never_replaced_by_cleanup(){
    let mut d=Device::new();let mut r=Region::new();let mut q=r.queue(&mut d);d.xtal_stuck=true;
    let e=prepare_inner(&mut d,&mut q,&board(),BAR_BYTES).unwrap_err();assert_eq!(e.stage,Stage::Baseband);assert_eq!(e.cause.kind,FaultKind::Timeout);
    assert!(e.radio_stop.is_some());assert_eq!(d.writes.iter().filter(|(at,_,_)|*at==R_AX_WLAN_XTAL_SI_CTRL).count(),1);d.stopped();
}
#[test]
fn cleanup_failure_does_not_hide_primary_error_or_stop_command_cleanup(){
    let mut d=Device::new();let mut r=Region::new();let mut q=r.queue(&mut d);d.fail_write=Some(9);d.refuse_bb_reset=true;
    let e=prepare_inner(&mut d,&mut q,&board(),BAR_BYTES).unwrap_err();assert_eq!(e.cause.kind,FaultKind::Access);assert!(e.radio_stop.is_some());
    let stop=e.command_stop.unwrap();assert!(stop.dma.is_none()&&stop.cpu.is_none());assert_eq!(d.get(fwcmd::INIT_CONFIG)&(1<<11|1<<13),0);
}
