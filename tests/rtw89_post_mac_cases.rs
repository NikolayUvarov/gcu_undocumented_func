use super::*;
use std::alloc::{alloc_zeroed,dealloc,Layout};

#[path="rtw89_phy_receive_cases.rs"] mod phy_receive;
#[path="rtw89_report_tail_cases.rs"]mod report_tail;

struct Region{base:*mut u8,layout:Layout}
impl Region{fn new(size:usize)->Self{let layout=Layout::from_size_align(size,4096).unwrap();let base=unsafe{alloc_zeroed(layout)};assert!(!base.is_null());Self{base,layout}}}
impl Drop for Region{fn drop(&mut self){unsafe{dealloc(self.base,self.layout)}}}
struct Device{regs:Vec<u8>,xtal:[u8;256],command:u16,now:u64,writes:Vec<(usize,usize,u32)>,fail:Option<usize>,ignore:Option<usize>,late_write:Option<usize>,late_start:bool,rx_active:bool,fail_radio:bool,fail_rx:bool,fail_cpu:bool}
impl Device{
    fn new()->Self{
        let mut d=Self{regs:vec![0;runtime_mac::BAR_BYTES],xtal:[0;256],command:2,now:0,writes:Vec::new(),fail:None,ignore:None,late_write:None,late_start:false,rx_active:false,fail_radio:false,fail_rx:false,fail_cpu:false};
        for (at,value) in [(0xf0,0x1000),(0x3f0,0x100),(0x88,3),(download::FW_CONTROL,0xe0),
            (0x8400,0xfffd0000),(0x8404,0x0f9f0000),(0xc000,0xf000003f),(0xc004,0x4000003f),
            (mac::WDE_CONFIG,510<<16),(mac::PLE_CONFIG,496<<16|4<<8|1),(mac::HFC_CONTROL,0x19),
            (fwcmd::DMA_STOP,0x1f0f03),(0x2f0,1<<17),(0x8040,14)]{d.put(at,4,value);}
        d.put(2,1,3);d.xtal[0x80]=0xc7;d.xtal[0x81]=0xc7;d
    }
    fn get(&self,at:usize,width:usize)->u32{let mut v=[0;4];v[..width].copy_from_slice(&self.regs[at..at+width]);u32::from_le_bytes(v)}
    fn put(&mut self,at:usize,width:usize,v:u32){self.regs[at..at+width].copy_from_slice(&v.to_le_bytes()[..width]);}
    fn apply(&mut self,at:usize,width:usize,v:u32)->io::Result<()>{
        self.writes.push((at,width,v));
        if self.ignore!=Some(self.writes.len()){
            let status=MASKS.iter().find(|entry|entry.0==at).map_or(0,|entry|entry.3);
            let next=(v&!status)|((self.get(at,width)&status)&!v);self.put(at,width,next);
            if at==fwcmd::CLEAR_INDEX{self.put(fwcmd::INDEX,4,0);}
            if at==0x1018{self.put(0x1050,4,0);self.put(0x1054,4,0);self.put(at,4,0);}
            if at==0x270{
                let offset=(v&255)as usize;
                if v>>24&7==1{self.regs[at+1]=self.xtal[offset];}
                else{let mask=(v>>16)as u8;self.xtal[offset]=(self.xtal[offset]&!mask)|((v>>8)as u8&mask);}
                self.regs[at+3]&=!0x80;
            }
            if at==fwcmd::DMA_STOP&&v&3==0{self.rx_active=true;}
        }
        if self.late_write==Some(self.writes.len()){self.now+=TIMEOUT_US;}
        if self.fail==Some(self.writes.len())||(self.fail_rx&&at==fwcmd::DMA_STOP&&v&3==3)||(self.fail_radio&&at==0x2f0)||(self.fail_cpu&&at==0x88){return Err(Fault::new(FaultKind::Access,at,v));}
        Ok(())
    }
    fn stopped(&self){assert_eq!(self.get(fwcmd::INIT_CONFIG,4)&HCI,0);assert_eq!(self.get(fwcmd::DMA_STOP,4)&STOP_MASK,STOP_MASK);assert_eq!(self.get(2,1)&3,0);assert_eq!(self.get(0x88,4)&2,0);}
}
impl RegisterIo for Device{
    fn read8(&mut self,at:usize)->io::Result<u8>{Ok(self.get(at,1)as u8)}
    fn read16(&mut self,at:usize)->io::Result<u16>{Ok(self.get(at,2)as u16)}
    fn read32(&mut self,at:usize)->io::Result<u32>{if self.late_start&&self.rx_active&&at==0x1054{self.now+=TIMEOUT_US;self.late_start=false;}Ok(self.get(at,4))}
    fn write8(&mut self,at:usize,v:u8)->io::Result<()>{self.apply(at,1,v as u32)}
    fn write16(&mut self,at:usize,v:u16)->io::Result<()>{self.apply(at,2,v as u32)}
    fn write32(&mut self,at:usize,v:u32)->io::Result<()>{self.apply(at,4,v)}
    fn now_us(&mut self)->u64{self.now}
    fn delay_us(&mut self,us:u32)->io::Result<()>{self.now+=u64::from(us);Ok(())}
}
impl Bus for Device{fn pci_command(&mut self)->io::Result<u16>{Ok(self.command)}}
fn board()->Board{Board{identity:crate::board::Identity{cut:1,rfe:1,mac:[2,1,2,3,4,5],xtal:1,country:*b"US",channel_plan:1},maps:crate::efuse::Maps{logical:[0xff;2048],dav_logical:[0xff;16],phycap:[0xff;128]}}}
fn prepared<'a,'b>(d:&mut Device,command:&'a mut Region,receive:&'b mut Region)->(Queue<'a>,rx::Rings<'b>){
    let mut q=unsafe{Queue::new(d,command.base,command.layout.size(),0x100000).unwrap()};q.install(d).unwrap();
    let mut r=unsafe{rx::Rings::new(d,receive.base,receive.layout.size(),0x200000).unwrap()};r.install(d).unwrap();
    d.command=6;d.put(fwcmd::INIT_CONFIG,4,1<<11);d.put(fwcmd::DMA_STOP,4,STOPPED);unsafe{q.start(d).unwrap()};d.writes.clear();(q,r)
}
fn stop(d:&mut Device,q:&mut Queue<'_>,r:&mut rx::Rings<'_>){r.stop(d);runtime_mac::disable_radio(d).unwrap();let s=download::stop(d,q);assert!(s.dma.is_none()&&s.cpu.is_none());d.command=0;}

#[test]
fn exact_poh_ltr_and_receive_only_activation_preserve_uninstalled_channels(){
    let mut d=Device::new();let mut a=Region::new(fwcmd::DMA_BYTES);let mut b=Region::new(rx::DMA_BYTES);let(mut q,mut r)=prepared(&mut d,&mut a,&mut b);
    d.put(R_AX_WDRLS_CFG,4,u32::MAX);activate_inner(&mut d,&mut q,&mut r,&board(),runtime_mac::BAR_BYTES).unwrap();
    assert_eq!(r.state(),rx::State::Running);assert_eq!(q.state(),fwcmd::State::Running);
    assert_eq!(d.get(fwcmd::DMA_STOP,4),RECEIVING);assert_eq!(d.get(fwcmd::INIT_CONFIG,4)&HCI,HCI);
    assert_eq!(d.get(R_AX_WDRLS_CFG,4)&B_AX_WDRLS_MODE_MASK,0);assert_eq!(d.get(R_AX_RLSRPT0_CFG1,4)&0xff00ff,0xff001e);
    assert_eq!(d.get(0x8410,4),0x2743);assert_eq!(d.get(0x8414,4),0x280028);assert_eq!(d.get(0x8418,4),0x90039003);assert_eq!(d.get(0x841c,4),0x880b880b);
    assert_eq!(d.get(0x8810,4)&1,1);assert_eq!(d.get(0x9a00,4)&2,0);
    assert!(d.writes.iter().all(|(at,_,_)|!HOST_MASKS.contains(at)&&*at!=0xc348&&*at!=fwcmd::INDEX&&!(0x1020..=0x1054).contains(at)&&!(0x1100..=0x11ff).contains(at)));
    let enable=d.writes.iter().position(|(at,_,v)|*at==fwcmd::INIT_CONFIG&&v&(1<<13)!=0).unwrap();
    let release=d.writes.iter().position(|(at,_,v)|*at==fwcmd::DMA_STOP&&v&3==0).unwrap();assert!(enable<release);
    stop(&mut d,&mut q,&mut r);
}
#[test]
fn mac_mask_writes_preserve_known_status_and_unrelated_register_fields(){
    let mut d=Device::new();let mut a=Region::new(fwcmd::DMA_BYTES);let mut b=Region::new(rx::DMA_BYTES);let(mut q,mut r)=prepared(&mut d,&mut a,&mut b);
    for &(at,_,_,status)in MASKS{d.put(at,4,0xa5a55a5a|status);}
    let originals:Vec<_>=MASKS.iter().map(|v|d.get(v.0,4)).collect();
    activate_inner(&mut d,&mut q,&mut r,&board(),runtime_mac::BAR_BYTES).unwrap();
    for (&(at,clear,set,status),old)in MASKS.iter().zip(originals){let actual=d.get(at,4);assert_eq!(actual&status,old&status);assert_eq!(actual&(clear|set),set);assert_eq!(actual&!(clear|set|status),old&!(clear|set|status));}
    stop(&mut d,&mut q,&mut r);
}
#[test]
fn stale_rings_or_active_unowned_channels_refuse_before_writes(){
    for variant in 0..14{
        let mut d=Device::new();let mut a=Region::new(fwcmd::DMA_BYTES);let mut b=Region::new(rx::DMA_BYTES);let(mut q,mut r)=prepared(&mut d,&mut a,&mut b);let mut board=board();let mut bytes=runtime_mac::BAR_BYTES;
        match variant{0=>d.command=0xffff,1=>d.command=2,2=>d.put(0x1100,4,0x300000),3=>d.put(0x1022,2,64),4=>d.put(0x1050,4,1<<16),5=>d.put(fwcmd::DMA_STOP,4,STOPPED&!(1<<8)),
            6=>d.put(HOST_MASKS[0],4,1),7=>d.put(0xc348,4,1),8=>d.put(0xe000,4,1<<30),9=>d.put(download::FW_CONTROL,4,0),10=>board.identity.rfe=5,11=>bytes-=1,12=>d.put(fwcmd::BASE_LOW,4,0x300000),_=>d.put(fwcmd::DMA_BUSY,4,1<<22)}
        let e=activate_inner(&mut d,&mut q,&mut r,&board,bytes).unwrap_err();assert_eq!(e.stage,Stage::Guard);assert!(d.writes.is_empty());assert_eq!(r.state(),rx::State::Installed);d.command=0;
    }
}
#[test]
fn every_applied_postinit_write_error_stops_both_receive_and_command_paths(){
    let count={let mut d=Device::new();let mut a=Region::new(fwcmd::DMA_BYTES);let mut b=Region::new(rx::DMA_BYTES);let(mut q,mut r)=prepared(&mut d,&mut a,&mut b);activate_inner(&mut d,&mut q,&mut r,&board(),runtime_mac::BAR_BYTES).unwrap();let count=d.writes.len();stop(&mut d,&mut q,&mut r);count};
    for fail in 1..=count{
        let mut d=Device::new();let mut a=Region::new(fwcmd::DMA_BYTES);let mut b=Region::new(rx::DMA_BYTES);let(mut q,mut r)=prepared(&mut d,&mut a,&mut b);d.fail=Some(fail);
        let e=activate_inner(&mut d,&mut q,&mut r,&board(),runtime_mac::BAR_BYTES).unwrap_err();assert!(matches!(e.cause,Error::Register(Fault{kind:FaultKind::Access,..})),"write{fail}: {e:?}");
        assert!(e.radio_stop.is_none());assert_eq!(r.state(),rx::State::Stopped);assert_eq!(q.state(),fwcmd::State::Faulted);d.stopped();d.command=0;
    }
}
#[test]
fn ignored_rx_activation_cannot_publish_receiving_state(){
    let release={let mut d=Device::new();let mut a=Region::new(fwcmd::DMA_BYTES);let mut b=Region::new(rx::DMA_BYTES);let(mut q,mut r)=prepared(&mut d,&mut a,&mut b);activate_inner(&mut d,&mut q,&mut r,&board(),runtime_mac::BAR_BYTES).unwrap();let n=d.writes.iter().position(|(at,_,_)|*at==fwcmd::INIT_CONFIG).unwrap()+1;stop(&mut d,&mut q,&mut r);n};
    let mut d=Device::new();let mut a=Region::new(fwcmd::DMA_BYTES);let mut b=Region::new(rx::DMA_BYTES);let(mut q,mut r)=prepared(&mut d,&mut a,&mut b);d.ignore=Some(release);
    let e=activate_inner(&mut d,&mut q,&mut r,&board(),runtime_mac::BAR_BYTES).unwrap_err();assert_eq!(e.stage,Stage::Receive);assert!(!d.rx_active);d.stopped();d.command=0;
}
#[test]
fn inaccessible_ltr_is_rejected_before_rx_enable(){
    for unavailable in [u32::MAX,0xeaeaeaea]{
        let mut d=Device::new();let mut a=Region::new(fwcmd::DMA_BYTES);let mut b=Region::new(rx::DMA_BYTES);let(mut q,mut r)=prepared(&mut d,&mut a,&mut b);d.put(0x8418,4,unavailable);
        assert_eq!(activate_inner(&mut d,&mut q,&mut r,&board(),runtime_mac::BAR_BYTES).unwrap_err().stage,Stage::Pcie);assert!(!d.rx_active);d.stopped();d.command=0;
    }
}
#[test]
fn cleanup_reports_independent_receive_radio_and_cpu_failures(){
    let mut d=Device::new();let mut a=Region::new(fwcmd::DMA_BYTES);let mut b=Region::new(rx::DMA_BYTES);let(mut q,mut r)=prepared(&mut d,&mut a,&mut b);d.fail=Some(1);d.fail_rx=true;d.fail_radio=true;d.fail_cpu=true;
    let e=activate_inner(&mut d,&mut q,&mut r,&board(),runtime_mac::BAR_BYTES).unwrap_err();
    assert!(e.receive_stop.unwrap().request.is_some());assert!(e.radio_stop.is_some());let s=e.command_stop.unwrap();assert!(s.dma.is_some()&&s.cpu.is_some());assert_eq!(q.state(),fwcmd::State::Faulted);d.command=0;
}
#[test]
fn stage_and_final_ring_read_deadlines_cannot_publish_success(){
    for final_read in [false,true]{
        let mut d=Device::new();let mut a=Region::new(fwcmd::DMA_BYTES);let mut b=Region::new(rx::DMA_BYTES);let(mut q,mut r)=prepared(&mut d,&mut a,&mut b);
        if final_read{d.late_start=true;}else{d.late_write=Some(2);}
        let e=activate_inner(&mut d,&mut q,&mut r,&board(),runtime_mac::BAR_BYTES).unwrap_err();assert!(matches!(e.cause,Error::Register(Fault{kind:FaultKind::Timeout,..})));d.stopped();d.command=0;
    }
}
#[test]
fn optional_pci_io_decode_does_not_change_memory_mastering_requirement(){
    let mut d=Device::new();let mut a=Region::new(fwcmd::DMA_BYTES);let mut b=Region::new(rx::DMA_BYTES);let(mut q,mut r)=prepared(&mut d,&mut a,&mut b);d.command=7;
    activate_inner(&mut d,&mut q,&mut r,&board(),runtime_mac::BAR_BYTES).unwrap();stop(&mut d,&mut q,&mut r);
}
#[test]
fn typed_permit_accepts_live_rx_and_pending_commands_without_hardware_writes(){
    let mut d=Device::new();let mut a=Region::new(fwcmd::DMA_BYTES);let mut b=Region::new(rx::DMA_BYTES);let(mut q,mut r)=prepared(&mut d,&mut a,&mut b);
    let token=activate_inner(&mut d,&mut q,&mut r,&board(),runtime_mac::BAR_BYTES).unwrap();
    q.submit(&mut d,&[1,2,3],false).unwrap();d.put(0x1050,4,5<<16);d.put(0x1054,4,7<<16);
    d.put(fwcmd::DMA_BUSY,4,(1<<22)|(1<<21)|(1<<20)|(1<<18)|3);d.writes.clear();
    {let _permit=token.validate(&mut d,&mut q,&r,1000).unwrap();}
    assert_eq!(q.pending(),1);assert!(d.writes.is_empty());
    d.put(fwcmd::INDEX,4,1|1<<16);
    {let _permit=token.validate(&mut d,&mut q,&r,1000).unwrap();}
    assert_eq!(q.pending(),0);assert!(d.writes.is_empty());d.put(fwcmd::DMA_BUSY,4,0);stop(&mut d,&mut q,&mut r);
}
#[test]
fn permit_rechecks_corrupted_live_registers_without_writing_or_releasing_rx(){
    for (at,width,value)in[(0x1108,4,0x300000),(0x110c,4,1),(0x1020,2,32),(0x1050,4,1),(0x1054,4,256<<16),
        (fwcmd::BASE_LOW,4,0x300000),(fwcmd::BASE_HIGH,4,1),(fwcmd::NUMBER,2,31),(fwcmd::BDRAM,4,0),
        (fwcmd::INIT_CONFIG,4,1<<11),(fwcmd::DMA_STOP,4,RECEIVING&!(1<<8)),(fwcmd::DMA_BUSY,4,1<<19),
        (0xc348,4,1),(0xc6ec,4,1<<7),(download::FW_CONTROL,4,0),(HOST_MASKS[2],4,1),(mac::HFC_CONTROL,4,0)]{
        let mut d=Device::new();let mut a=Region::new(fwcmd::DMA_BYTES);let mut b=Region::new(rx::DMA_BYTES);let(mut q,mut r)=prepared(&mut d,&mut a,&mut b);
        let token=activate_inner(&mut d,&mut q,&mut r,&board(),runtime_mac::BAR_BYTES).unwrap();d.writes.clear();d.put(at,width,value);
        assert!(token.validate(&mut d,&mut q,&r,1000).is_err(),"register {at:x}");assert!(d.writes.is_empty());assert_eq!(r.state(),rx::State::Running);d.command=0;
    }
}
#[test]
fn same_addresses_reconstructed_dma_objects_cannot_reuse_old_receiving_token(){
    let mut d=Device::new();let mut a=Region::new(fwcmd::DMA_BYTES);let mut b=Region::new(rx::DMA_BYTES);
    let old={let(mut q,mut r)=prepared(&mut d,&mut a,&mut b);let token=activate_inner(&mut d,&mut q,&mut r,&board(),runtime_mac::BAR_BYTES).unwrap();stop(&mut d,&mut q,&mut r);token};
    d=Device::new();let(mut q,mut r)=prepared(&mut d,&mut a,&mut b);
    let fresh=activate_inner(&mut d,&mut q,&mut r,&board(),runtime_mac::BAR_BYTES).unwrap();d.writes.clear();
    assert!(old.validate(&mut d,&mut q,&r,1000).is_err());assert!(d.writes.is_empty());
    {let _permit=fresh.validate(&mut d,&mut q,&r,1000).unwrap();}assert!(d.writes.is_empty());stop(&mut d,&mut q,&mut r);
}
#[test]
fn replacing_only_command_object_cannot_reuse_receive_session(){
    let mut d=Device::new();let mut a=Region::new(fwcmd::DMA_BYTES);let mut b=Region::new(rx::DMA_BYTES);let(mut q,mut r)=prepared(&mut d,&mut a,&mut b);
    let token=activate_inner(&mut d,&mut q,&mut r,&board(),runtime_mac::BAR_BYTES).unwrap();
    let original=q.session();d.command=0;drop(q);
    d.command=2;d.put(fwcmd::INIT_CONFIG,4,0);d.put(fwcmd::DMA_STOP,4,STOP_MASK);
    let mut q=unsafe{Queue::new(&mut d,a.base,a.layout.size(),0x100000).unwrap()};assert_ne!(original,q.session());q.install(&mut d).unwrap();
    d.command=6;d.put(fwcmd::INIT_CONFIG,4,1<<11);d.put(fwcmd::DMA_STOP,4,STOPPED);unsafe{q.start(&mut d).unwrap()};
    d.put(fwcmd::INIT_CONFIG,4,HCI);d.put(fwcmd::DMA_STOP,4,RECEIVING);d.writes.clear();
    assert!(token.validate(&mut d,&mut q,&r,1000).is_err());assert!(d.writes.is_empty());stop(&mut d,&mut q,&mut r);
}
#[test]
fn faulted_or_stopped_objects_and_invalid_budget_never_get_permit(){
    for variant in 0..5{
        let mut d=Device::new();let mut a=Region::new(fwcmd::DMA_BYTES);let mut b=Region::new(rx::DMA_BYTES);let(mut q,mut r)=prepared(&mut d,&mut a,&mut b);
        let token=activate_inner(&mut d,&mut q,&mut r,&board(),runtime_mac::BAR_BYTES).unwrap();let mut budget=1000;
        match variant{0=>{r.stop(&mut d);},1=>q.poison(),2=>budget=0,3=>budget=1_000_001,_=>d.command=2}
        d.writes.clear();assert!(token.validate(&mut d,&mut q,&r,budget).is_err());assert!(d.writes.is_empty());d.command=0;
    }
}
#[test]
fn late_final_receive_index_read_never_issues_typed_permit(){
    let mut d=Device::new();let mut a=Region::new(fwcmd::DMA_BYTES);let mut b=Region::new(rx::DMA_BYTES);let(mut q,mut r)=prepared(&mut d,&mut a,&mut b);
    let token=activate_inner(&mut d,&mut q,&mut r,&board(),runtime_mac::BAR_BYTES).unwrap();d.late_start=true;d.writes.clear();
    let error=token.validate(&mut d,&mut q,&r,1000).unwrap_err();assert!(matches!(error,Error::Register(Fault{kind:FaultKind::Timeout,..})|Error::Receive(rx::Error::Io(Fault{kind:FaultKind::Timeout,..}))));
    assert!(d.writes.is_empty());stop(&mut d,&mut q,&mut r);
}
#[test]
fn session_counter_is_unique_and_exhaustion_stays_latched_without_wrap(){
    use core::sync::atomic::{AtomicU64,Ordering};
    let counter=AtomicU64::new(u64::MAX-1);assert_eq!(io::next_dma_session(&counter).unwrap(),u64::MAX-1);
    for _ in 0..3{assert!(io::next_dma_session(&counter).is_err());assert_eq!(counter.load(Ordering::Relaxed),u64::MAX);}
    let zero=AtomicU64::new(0);assert!(io::next_dma_session(&zero).is_err());assert_eq!(zero.load(Ordering::Relaxed),0);
}
