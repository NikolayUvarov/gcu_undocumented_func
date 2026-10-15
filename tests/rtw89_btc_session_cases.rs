use super::*;
use crate::{board::Board,fwcmd::Bus,runtime_mac, runtime_mac_regs::*,registers::*};
use std::alloc::{alloc_zeroed,dealloc,Layout};

struct Region{base:*mut u8,layout:Layout}
impl Region{fn new()->Self{let layout=Layout::from_size_align(rx::DMA_BYTES,4096).unwrap();let base=unsafe{alloc_zeroed(layout)};assert!(!base.is_null());Self{base,layout}}}
impl Drop for Region{fn drop(&mut self){unsafe{dealloc(self.base,self.layout)}}}
#[derive(Clone,Copy,PartialEq)]
enum Reply{Normal,Missing,WrongSequence,Rejected,WrongChannel,Malformed}
struct Device{
    regs:Vec<u8>,xtal:[u8;256],command:u16,time:u64,frozen:bool,writes:Vec<(usize,usize,u32)>,
    tx:*mut u8,tx_bytes:usize,rx:*mut u8,packets:Vec<Vec<u8>>,reply:Reply,events:usize,
    hold:bool,busy:bool,late_index:usize,index_reads:usize,reverse_index:usize,
    bt:u32,rf:[[u32;256];2],rf_read:Option<(usize,usize)>,lte:[u32;2],lte_pending:Option<u32>,
    btc_event:Option<(usize,Vec<u8>)>,missing_at:usize,fail_at:Option<usize>,packet_us:u64,bt_release_at:Option<u64>,after_ack:Option<(usize,Vec<u8>)>,
    scheduler_reply:u8,scheduler_us:u64,delay_event:Option<(u64,Vec<u8>)>,delay_extra:u64,
}
impl Device{
    fn new(tx:*mut u8,tx_bytes:usize,index:u32)->Self{
        let mut d=Self{regs:vec![0;runtime_mac::BAR_BYTES],xtal:[0;256],command:6,time:0,frozen:false,writes:Vec::new(),
            tx,tx_bytes,rx:core::ptr::null_mut(),packets:Vec::new(),reply:Reply::Normal,events:0,hold:false,busy:false,late_index:0,index_reads:0,reverse_index:0,
            bt:0,rf:[[0;256];2],rf_read:None,lte:[0x12345678,0x0100],lte_pending:None,btc_event:None,missing_at:0,fail_at:None,packet_us:0,bt_release_at:None,after_ack:None,scheduler_reply:0,scheduler_us:0,delay_event:None,delay_extra:0};
        for (at,value)in[(0xf0,1<<12),(0x3f0,1<<8),(R_AX_PLATFORM_ENABLE,3),(download::FW_CONTROL,0xe0),
            (R_AX_CMAC_FUNC_EN,0x7000803f),(R_AX_DMAC_FUNC_EN,0x64c40000),(crate::mac::DMAC_CLOCK,0x04840000),
            (crate::mac::PLE_CONFIG,0x00400801),(crate::mac::WDE_READY,3),(crate::mac::PLE_READY,3),(crate::mac::HFC_CONTROL,8),
            (fwcmd::INIT_CONFIG,1<<11),(fwcmd::DMA_STOP,0xb0f03),(fwcmd::INDEX,index),(fwcmd::BASE_LOW,0x100000),
            (fwcmd::BDRAM,28|4<<8|1<<16),(fwcmd::NUMBER,32),(R_AX_CTN_TXEN,0xffff),(0x8a9c,446)]{d.set(at,value);}
        d.xtal[0x80]=0xc0;d.xtal[0x81]=0xb8;d
    }
    fn get(&self,at:usize)->u32{u32::from_le_bytes(self.regs[at..at+4].try_into().unwrap())}
    fn set(&mut self,at:usize,value:u32){self.regs[at..at+4].copy_from_slice(&value.to_le_bytes());}
    fn inject(&mut self,channel:usize,event:&[u8]){
        assert!(channel<2);assert!(!self.rx.is_null());assert!(20+event.len()<=rx::BUFFER_BYTES);
        let index=[0x1050,0x1054][channel];let hw=(self.get(index)>>16)as usize;assert!(hw<rx::DESCRIPTORS);
        let at=rx::BUFFERS_AT+(channel*rx::DESCRIPTORS+hw)*rx::BUFFER_STRIDE;
        let mut packet=vec![0;20+event.len()];let len=packet.len();packet[..4].copy_from_slice(&(len as u32|0xc000).to_le_bytes());
        packet[4..8].copy_from_slice(&(event.len()as u32|(rxdesc::TYPE_C2H as u32)<<24).to_le_bytes());packet[20..].copy_from_slice(event);
        unsafe{core::ptr::copy_nonoverlapping(packet.as_ptr(),self.rx.add(at),packet.len());}
        self.set(index,(self.get(index)&0xffff)|(((hw+1)%rx::DESCRIPTORS)as u32)<<16);
    }
    fn consume(&mut self,next:u32){
        assert!(next<32);let descriptor=((next as usize+31)%32)*8;
        let (size,address)=unsafe{(u32::from_le(self.tx.add(descriptor).cast::<u32>().read_unaligned()),u32::from_le(self.tx.add(descriptor+4).cast::<u32>().read_unaligned()))};
        assert_eq!(size&(1<<30),1<<30);let length=(size&0xffff)as usize;
        assert!((fwcmd::TXWD_BYTES..=fwcmd::TXWD_BYTES+fwcmd::MAX_PAYLOAD).contains(&length));
        let offset=(address as usize).checked_sub(0x100000).unwrap();assert!(offset.checked_add(length).is_some_and(|end|end<=self.tx_bytes));
        let txwd=unsafe{u32::from_le(self.tx.add(offset).cast::<u32>().read_unaligned())};assert_eq!(txwd,12<<16);
        let payload=unsafe{core::slice::from_raw_parts(self.tx.add(offset+fwcmd::TXWD_BYTES),length-fwcmd::TXWD_BYTES)}.to_vec();
        assert!(payload.len()>=8);let w0=u32::from_le_bytes(payload[..4].try_into().unwrap());let w1=u32::from_le_bytes(payload[4..8].try_into().unwrap());
        assert_eq!((w1&0x3fff)as usize,payload.len());self.packets.push(payload);self.time+=self.packet_us;
        if !self.hold{self.set(fwcmd::INDEX,next|next<<16);}
        if self.busy{self.set(fwcmd::DMA_BUSY,TX_BUSY);}
        for n in 0..self.events{self.inject(0,&c2h_bytes(2,0x12,0,&[n as u8,0xaa]));}
        if self.btc_event.as_ref().is_some_and(|e|e.0==self.packets.len()){let (_,event)=self.btc_event.take().unwrap();self.inject(0,&event);}
        if self.reply==Reply::Missing||self.packets.len()==self.missing_at{return}
        let mut seq=(w0>>24)as u8;if self.reply==Reply::WrongSequence{seq=seq.wrapping_add(1);}
        let identity=w0&0xffff;let channel=usize::from(self.reply==Reply::WrongChannel);
        if self.reply==Reply::Malformed{self.inject(0,&[0,0]);return}
        if w1&(1<<14)!=0{self.inject(channel,&c2h_bytes(1,0,0,&(identity|u32::from(seq)<<16).to_le_bytes()));}
        if w1&(1<<15)!=0{let status=if self.reply==Reply::Rejected{9}else{0};self.inject(channel,&c2h_bytes(1,0,1,&(identity|status<<16|u32::from(seq)<<24).to_le_bytes()));}
        if self.after_ack.as_ref().is_some_and(|v|v.0==self.packets.len()){let(_,event)=self.after_ack.take().unwrap();self.inject(0,&event);}
    }
    fn apply(&mut self,at:usize,width:usize,value:u32)->io::Result<()>{
        self.writes.push((at,width,value));let previous=self.get(at&!3);self.regs[at..at+width].copy_from_slice(&value.to_le_bytes()[..width]);
        if at==0x1018{self.set(0x1050,0);self.set(0x1054,0);self.set(at,0);}
        if at==R_AX_WLAN_XTAL_SI_CTRL{let offset=(value&255)as usize;if value>>24&7==XTAL_SI_NORMAL_READ{self.regs[at+1]=self.xtal[offset];}else{let mask=(value>>16)as u8;self.xtal[offset]=(self.xtal[offset]&!mask)|((value>>8)as u8&mask);}self.regs[at+3]&=!0x80;}
        if at==R_AX_H2CREG_CTRL&&value==1{assert_eq!(self.get(R_AX_H2CREG_DATA0),0x205);assert_eq!(self.get(R_AX_H2CREG_DATA0+4),0xffff);
            self.time+=self.scheduler_us;self.regs[at]=0;self.regs[R_AX_C2HREG_CTRL]=u8::from(self.scheduler_reply!=1);
            self.set(R_AX_C2HREG_DATA0,if self.scheduler_reply==2{0x205}else{0x204});self.set(R_AX_CTN_TXEN,u32::from(self.scheduler_reply==3));
            if self.scheduler_reply==4{self.set(R_AX_PTCL_TX_CTN_SEL,B_AX_PTCL_TX_ON_STAT);}}
        if at==R_AX_SS_CTRL&&value&1!=0{self.regs[at+3]|=0x80;}
        if at==R_AX_ADDR_CAM_CTRL{self.regs[at+1]&=!1;}
        if at==R_AX_RESPBA_CAM_CTRL{self.regs[at]&=!3;}
        if at==0xdaf0{assert!(self.lte_pending.is_none());assert!(matches!(value,0x800f0038|0xc00f0038|0x800f003c|0xc00f003c));self.lte_pending=Some(value);}
        if at==0x10370{let path=(value>>28&3)as usize;assert!(path<2);let register=(value>>20&255)as usize;
            let mask=if value>>31!=0{self.get(0x10374)&0xfffff}else{0xfffff};self.rf[path][register]=(self.rf[path][register]&!mask)|(value&mask);self.rf_read=None;}
        if at==0x10378{let path=(value>>8&7)as usize;assert!(path<2);self.rf_read=Some((path,(value&255)as usize));}
        if at==fwcmd::INDEX{assert_eq!(width,2);self.consume(value);}
        if [0x1050,0x1054].contains(&at)&&width==2&&!self.rx.is_null(){
            let channel=usize::from(at==0x1054);let host=(previous&0xffff)as usize;assert!(host<rx::DESCRIPTORS);
            let offset=rx::BUFFERS_AT+(channel*rx::DESCRIPTORS+host)*rx::BUFFER_STRIDE;
            unsafe{core::ptr::write_bytes(self.rx.add(offset),0xdd,rx::BUFFER_BYTES);}
        }
        if self.fail_at==Some(at){self.fail_at=None;return Err(Fault::new(FaultKind::Access,at,value))}
        Ok(())
    }
}
impl RegisterIo for Device{
    fn read8(&mut self,a:usize)->io::Result<u8>{
        if a==0xdaf3{if let Some(command)=self.lte_pending.take(){if command>>30==3{self.lte[usize::from(command&255==0x3c)]=self.get(0xdaf4);}}return Ok(0x20)}
        Ok(self.regs[a])}
    fn read16(&mut self,a:usize)->io::Result<u16>{Ok(u16::from_le_bytes(self.regs[a..a+2].try_into().unwrap()))}
    fn read32(&mut self,a:usize)->io::Result<u32>{
        if a==0xac{if self.bt_release_at.is_some_and(|at|self.time>=at){self.bt&=!0x60;self.bt_release_at=None;}return Ok(self.bt)}
        if a==0xdaf8{let command=self.lte_pending.unwrap();assert_eq!(command>>30,2);return Ok(self.lte[usize::from(command&255==0x3c)])}
        if a==0x1174c{return Ok(self.rf_read.map_or(0,|(p,r)|1<<26|self.rf[p][r]))}
        if a==fwcmd::INDEX{self.index_reads+=1;if self.late_index==self.index_reads{self.time+=100_000;}if self.reverse_index==self.index_reads{self.time=0;}}Ok(self.get(a))}
    fn write8(&mut self,a:usize,v:u8)->io::Result<()>{self.apply(a,1,v as u32)}
    fn write16(&mut self,a:usize,v:u16)->io::Result<()>{self.apply(a,2,v as u32)}
    fn write32(&mut self,a:usize,v:u32)->io::Result<()>{self.apply(a,4,v)}
    fn now_us(&mut self)->u64{self.time}
    fn delay_us(&mut self,us:u32)->io::Result<()>{if !self.frozen{self.time+=u64::from(us)+if us!=0{self.delay_extra}else{0};}
        if self.delay_event.as_ref().is_some_and(|(at,_)|self.time>=*at){let(_,event)=self.delay_event.take().unwrap();self.inject(0,&event);}Ok(())}
}
impl Bus for Device{fn pci_command(&mut self)->io::Result<u16>{Ok(self.command)}}
impl DeviceIo for Device{fn pci_id(&mut self)->io::Result<u32>{Ok(0xb85210ec)}fn bar_len(&self)->usize{self.regs.len()}}
fn c2h_bytes(category:u8,class:u8,function:u8,body:&[u8])->Vec<u8>{let mut b=Vec::new();b.extend_from_slice(&(u32::from(category)|u32::from(class)<<2|u32::from(function)<<8).to_le_bytes());b.extend_from_slice(&((8+body.len())as u32).to_le_bytes());b.extend_from_slice(body);b}
fn board()->Board{Board{identity:crate::board::Identity{cut:1,rfe:1,mac:[2,1,2,3,4,5],xtal:1,country:*b"US",channel_plan:1},maps:crate::efuse::Maps{logical:[0xff;2048],dav_logical:[0xff;16],phycap:[0xff;128]}}}
fn with_session(check:impl FnOnce(&mut Session<'_,'_,'_,Device>)){
    download::tests::with_downloaded_storage(|ready,q,tx,tx_bytes,index|{
        let mut d=Device::new(tx,tx_bytes,index);let prepared=runtime_mac::prepare(&mut d,q,&board(),runtime_mac::BAR_BYTES,&ready).unwrap();
        let region=Region::new();d.rx=region.base;d.command=2;
        let mut rings=unsafe{rx::Rings::new(&mut d,region.base,region.layout.size(),0x200000).unwrap()};rings.install(&mut d).unwrap();d.command=6;
        let receiving=post_mac::activate(&mut d,q,&mut rings,&board(),runtime_mac::BAR_BYTES,&prepared).unwrap();let mut scratch=vec![0;rx::BUFFER_BYTES];
        d.writes.clear();d.index_reads=0;
        let mut session=Session::new(&mut d,q,&mut rings,&receiving,ready,&mut scratch).unwrap();check(&mut session);drop(session);
        d.set(fwcmd::DMA_BUSY,0);rings.stop(&mut d);let stopped=download::stop(&mut d,q);assert!(stopped.dma.is_none()&&stopped.cpu.is_none());d.command=0;
    });
}
fn command()->h2c::Command{h2c::Command{category:2,class:0x11,function:7}}
fn both()->h2c::Acks{h2c::Acks{received:true,done:true}}

#[test]
fn real_download_runtime_and_rx_tokens_share_normal_sequence_and_phy_identity(){with_session(|s|{
    assert_ne!(PhyIo::command_session(s),0);assert_eq!(PhyIo::command_session(s),s.firmware.session());
    s.receive_permit(1000).unwrap();s.send_rf_page(0,0,&[1,2,3,4],1000).unwrap();s.send_rf_page(1,0,&[5,6,7,8],1000).unwrap();
    assert_eq!(s.submitted(),2);assert_eq!(s.bus.packets.len(),2);let first=&s.bus.packets[0];let second=&s.bus.packets[1];
    assert_eq!(u32::from_le_bytes(first[..4].try_into().unwrap()),2|8<<2);assert_eq!(u32::from_le_bytes(second[..4].try_into().unwrap()),2|9<<2|1<<24);
    assert_ne!(first[5]&0x40,0);assert_eq!(second[5]&0xc0,0);assert_eq!(s.queue.pending(),0);
    assert_eq!(s.bus.get(fwcmd::DMA_STOP),0xb0f00);assert_eq!(s.bus.get(R_AX_CTN_TXEN),0);
});}
#[test]
fn receipt_and_done_required_and_rejection_preserved(){with_session(|s|{
    assert_eq!(s.command(command(),&[3,4],both(),1000).unwrap(),h2c::Outcome::Done);
    s.bus.reply=Reply::Rejected;assert_eq!(s.command(command(),&[],both(),1000),Err(Error::Rejected(9)));
    assert_eq!(s.failed(),Some(Error::Rejected(9)));assert_eq!(s.submitted(),2);
});}
#[test]
fn wrong_or_missing_ack_poison_without_resending(){for reply in [Reply::WrongSequence,Reply::Missing]{with_session(|s|{
    s.bus.reply=reply;assert!(s.command(command(),&[],both(),100).is_err());assert_eq!(s.submitted(),1);assert_eq!(s.bus.packets.len(),1);
    let writes=s.bus.writes.len();assert!(s.command(command(),&[],both(),100).is_err());assert_eq!(s.bus.writes.len(),writes);
    if reply==Reply::WrongSequence{assert_eq!(s.counts().unrelated_ack,2);}
});}}
#[test]
fn failed_session_rejects_phy_stages_but_raw_register_cleanup_remains(){with_session(|s|{
    s.bus.reply=Reply::Missing;assert!(s.command(command(),&[],both(),100).is_err());
    assert_eq!(PhyIo::command_session(s),0);assert!(PhyIo::pci_id(s).is_err());assert!(PhyIo::pci_command(s).is_err());assert!(s.receive_permit(100).is_err());
    s.write32(fwcmd::DMA_STOP,0x1f0f03).unwrap();assert_eq!(s.read32(fwcmd::DMA_STOP).unwrap(),0x1f0f03);
});}
#[test]
fn btc_events_are_ordered_and_survive_dma_buffer_release(){with_session(|s|{
    s.bus.events=3;s.command(command(),&[],both(),1000).unwrap();assert_eq!(s.pending_events(),3);
    for n in 0..3{assert_eq!(s.event().unwrap().bytes[8..],[n,0xaa]);s.consume_event();}
    assert!(s.event().is_none());assert_eq!(s.pending_events(),0);assert_eq!(s.failed(),None);
});}
#[test]
fn fifth_unconsumed_btc_event_fails_instead_of_dropping_bt_state(){with_session(|s|{
    s.bus.events=5;assert_eq!(s.command(command(),&[],both(),1000),Err(Error::EventOverflow));assert_eq!(s.pending_events(),4);
    assert_eq!(s.event().unwrap().bytes[8..],[0,0xaa]);assert_eq!(s.bus.packets.len(),1);
});}
#[test]
fn rpq_cannot_inject_command_ack_and_malformed_c2h_is_terminal(){for reply in [Reply::WrongChannel,Reply::Malformed]{with_session(|s|{
    s.bus.reply=reply;assert!(s.command(command(),&[],both(),1000).is_err());assert!(s.failed().is_some());assert_eq!(s.bus.packets.len(),1);
});}}
#[test]
fn invalid_request_does_not_consume_sequence_or_poison(){with_session(|s|{
    for timeout in [0,MAX_COMMAND_US+1]{assert_eq!(s.command(command(),&[],both(),timeout),Err(Error::Request));}
    assert!(s.send_rf_page(2,0,&[1;4],1000).is_err());assert_eq!(s.failed(),None);assert_eq!(s.submitted(),0);assert!(s.bus.packets.is_empty());
    s.command(command(),&[],both(),1000).unwrap();
});}
#[test]
fn stale_live_ring_guard_refuses_before_command_publication(){with_session(|s|{
    s.bus.set(0x1108,0x300000);s.bus.writes.clear();assert!(s.command(command(),&[],both(),1000).is_err());assert_eq!(s.submitted(),0);assert!(s.bus.writes.is_empty());
});}
#[test]
fn elapsed_during_submit_index_read_prevents_doorbell(){with_session(|s|{
    s.bus.index_reads=0;s.bus.late_index=2;assert!(s.command(command(),&[],both(),1000).is_err());assert!(s.bus.packets.is_empty());assert_eq!(s.submitted(),0);
});}
#[test]
fn clock_reversal_before_publication_is_terminal(){with_session(|s|{
    s.bus.time=100;s.bus.index_reads=0;s.bus.reverse_index=2;assert!(s.command(command(),&[],both(),1000).is_err());assert!(s.bus.packets.is_empty());
});}
#[test]
fn frozen_clock_and_unconsumed_or_busy_command_are_bounded(){for mode in 0..3{with_session(|s|{
    match mode{0=>{s.bus.frozen=true;s.bus.reply=Reply::Missing;},1=>s.bus.hold=true,_=>s.bus.busy=true}
    assert!(s.command(command(),&[],both(),100).is_err());assert_eq!(s.bus.packets.len(),1);assert!(s.failed().is_some());assert!(s.bus.writes.len()<100);
});}}

#[test]
fn failed_receive_validation_prevents_later_stopped_rx_phy_bypass(){with_session(|s|{
    s.bus.set(0x1108,0x300000);assert!(s.receive_permit(1000).is_err());assert!(s.failed().is_some());
    s.write32(fwcmd::DMA_STOP,0xb0f03).unwrap();s.write32(fwcmd::INIT_CONFIG,1<<11).unwrap();
    assert!(PhyIo::pci_id(s).is_err());assert_eq!(PhyIo::command_session(s),0);assert!(s.bus.packets.is_empty());
});}


fn owner_phase(s:&mut Session<'_,'_,'_,Device>,check:impl FnOnce(&mut crate::phy::Phy<'_,Session<'_,'_,'_,Device>>)){
    let mut phy=crate::phy::Phy::new(s,board().identity);phy.baseband().unwrap();phy.radio().unwrap();check(&mut phy);
}
#[test]
fn btc_owner_exact_cold_init_commands_without_invented_tdma_report_request(){with_session(|s|owner_phase(s,|p|{
    let token=crate::btc_context::initialize(p,&board()).unwrap();token.validate(p,1000).unwrap();
    let s=p.bus_between_stages();let functions:Vec<_>=s.bus.packets[2..].iter().map(|v|v[1]).collect();
    assert_eq!(functions,[2,0,1,5,5,3,10,5]);
    let reports:Vec<_>=s.bus.packets.iter().filter(|v|v[1]==0&&v[0]==(2|16<<2)).collect();assert_eq!(reports.len(),1);
    assert_eq!(&reports[0][8..],&[5,4,0,0,0,1,0,0,0]);assert_eq!(s.bus.get(0xac)&0x00ffffff,0x4003);
    assert_eq!(s.bus.lte[0],0x77007700);assert_eq!(s.bus.get(0xc67c)&0xffff,0x166);
    assert_eq!(s.bus.get(fwcmd::DMA_STOP),0xb0f00);assert_eq!(s.bus.get(R_AX_CTN_TXEN),0);
    let writes=s.bus.writes.len();assert!(crate::btc_context::initialize(p,&board()).is_err());assert_eq!(p.bus_between_stages().bus.writes.len(),writes);
}));}
#[test]
fn btc_owner_actual_btinfo_drives_btg_gain_ignore_and_lna_commands(){with_session(|s|owner_phase(s,|p|{
    let s=p.bus_between_stages();s.bus.bt=2|1<<30;s.bus.btc_event=Some((3,c2h_bytes(2,0x12,1,&[0,6,0x20,0,0,8,0,0])));
    crate::btc_context::initialize(p,&board()).unwrap();let s=p.bus_between_stages();
    assert_eq!(s.bus.get(0x14738)&(1<<19),1<<19);assert_eq!(s.bus.rf[1][0xef],0);assert_eq!(s.bus.rf[1][0x3f],5);
    assert!(s.bus.packets.iter().any(|v|v[1]==12&&v[8..]==[0]));assert!(s.bus.packets.iter().any(|v|v[1]==14&&v[8..]==[7]));
    assert!(s.bus.packets.iter().any(|v|v[1]==0&&v[8..]==[5,12,0,0,0,1,0,0,0]));
}));}
#[test]
fn btc_owner_missing_each_requested_ack_never_publishes_initialized(){for command_index in [3,4,5,8,9]{with_session(|s|owner_phase(s,|p|{
    p.bus_between_stages().bus.missing_at=command_index;assert!(crate::btc_context::initialize(p,&board()).is_err());
    assert_eq!(p.state(),crate::phy::State::Failed);let s=p.bus_between_stages();assert!(s.btc.is_none());assert!(s.failed().is_some());
    assert_eq!(s.bus.packets.len(),command_index);assert_eq!(s.bus.regs[2]&3,0);assert_eq!(s.bus.rf[1][0xef],0);
}));}}
#[test]
fn btc_owner_whql_and_busy_bt_refuse_calibration_readiness(){for state in [2|4,2|32,2|64]{with_session(|s|owner_phase(s,|p|{
    p.bus_between_stages().bus.bt=state;assert!(crate::btc_context::initialize(p,&board()).is_err());
    assert!(p.bus_between_stages().btc.is_none());assert_eq!(p.state(),crate::phy::State::Failed);
}));}}
#[test]
fn btc_owner_applied_register_failures_restore_known_ant_and_close_lut(){for at in [0xda30,0xd200,0xc67c,0xac]{with_session(|s|owner_phase(s,|p|{
    p.bus_between_stages().bus.fail_at=Some(at);assert!(crate::btc_context::initialize(p,&board()).is_err());
    let s=p.bus_between_stages();assert!(s.btc.is_none());assert!(s.failed().is_some());assert_eq!(s.bus.regs[2]&3,0);
    assert_eq!(s.bus.rf[0][0xef],0);assert_eq!(s.bus.rf[1][0xef],0);
}));}}
#[test]
fn btc_owner_token_rejects_a_new_phy_attempt_on_the_same_session(){with_session(|s|{
    let token={let mut p=crate::phy::Phy::new(s,board().identity);p.baseband().unwrap();p.radio().unwrap();crate::btc_context::initialize(&mut p,&board()).unwrap()};
    let mut other=crate::phy::Phy::new(s,board().identity);assert!(token.validate(&mut other,1000).is_err());
});}

fn policy_report(policy:crate::btc_codec::Policy,tdma:bool)->Vec<u8>{
    let mut body=Vec::new();
    if tdma{body.extend_from_slice(&[1,12,0,3,0,0,0]);body.extend_from_slice(&policy.tdma());}
    else{body.extend_from_slice(&[2,152,0,1,18,0,0,0,0,0,0]);
        for index in 0..18{let(d,t,k)=policy.slot(index);body.extend_from_slice(&d.to_le_bytes());body.extend_from_slice(&t.to_le_bytes());body.extend_from_slice(&k.to_le_bytes());}}
    c2h_bytes(2,0x12,0,&body)
}
#[test]
fn btc_owner_pre_policy_reports_are_transition_snapshots(){for position in [0,8]{with_session(|s|owner_phase(s,|p|{
    let s=p.bus_between_stages();let report=policy_report(crate::btc_codec::Policy::Rfk,true);
    if position==0{s.bus.inject(0,&report)}else{s.bus.btc_event=Some((position,report));}
    crate::btc_context::initialize(p,&board()).unwrap();
}));}}
#[test]
fn btc_owner_post_policy_contradictions_fail_including_unmarked_slots(){for tdma in [true,false]{with_session(|s|owner_phase(s,|p|{
    p.bus_between_stages().bus.btc_event=Some((9,policy_report(crate::btc_codec::Policy::Rfk,tdma)));
    let failure=crate::btc_context::initialize(p,&board()).unwrap_err();assert_eq!(failure.cause,crate::btc_context::Error::PolicyMismatch);
    assert_eq!(p.state(),crate::phy::State::Failed);assert!(p.bus_between_stages().btc.is_none());
}));}}
#[test]
fn btc_owner_matching_tdma_and_slots_are_optional_consistent_observations(){for tdma in [true,false]{with_session(|s|owner_phase(s,|p|{
    p.bus_between_stages().bus.btc_event=Some((9,policy_report(crate::btc_codec::Policy::NoLink,tdma)));
    crate::btc_context::initialize(p,&board()).unwrap();
}));}}
#[test]
fn btc_owner_malformed_transition_report_is_not_discarded(){with_session(|s|owner_phase(s,|p|{
    let mut report=policy_report(crate::btc_codec::Policy::Rfk,true);report.pop();let length=report.len()as u32;report[4..8].copy_from_slice(&length.to_le_bytes());
    p.bus_between_stages().bus.btc_event=Some((8,report));
    let failure=crate::btc_context::initialize(p,&board()).unwrap_err();assert!(matches!(failure.cause,crate::btc_context::Error::Reports(_)));
    assert!(p.bus_between_stages().btc.is_none());
}));}

#[test]
fn btc_owner_all_report_slots_are_checked_even_with_zero_update_bitmap(){with_session(|s|owner_phase(s,|p|{
    let mut report=policy_report(crate::btc_codec::Policy::NoLink,false);let last=report.len()-1;report[last]^=1;
    p.bus_between_stages().bus.btc_event=Some((9,report));
    assert_eq!(crate::btc_context::initialize(p,&board()).unwrap_err().cause,crate::btc_context::Error::PolicyMismatch);
}));}
#[test]
fn btc_owner_later_consistent_snapshot_reconciles_before_final_drain(){with_session(|s|owner_phase(s,|p|{
    let mut reports=policy_report(crate::btc_codec::Policy::Rfk,true);
    reports.extend_from_slice(&policy_report(crate::btc_codec::Policy::NoLink,true)[8..]);
    let length=reports.len()as u32;reports[4..8].copy_from_slice(&length.to_le_bytes());
    p.bus_between_stages().bus.btc_event=Some((9,reports));crate::btc_context::initialize(p,&board()).unwrap();
}));}
#[test]
fn btc_owner_aggregate_deadline_is_not_renewed_for_each_command(){with_session(|s|owner_phase(s,|p|{
    let s=p.bus_between_stages();s.bus.packet_us=90_000;s.bus.bt=2|1<<30;
    s.bus.btc_event=Some((9,c2h_bytes(2,0x12,1,&[0,6,0x20,0,0,8,0,0])));let before=s.bus.time;
    assert!(crate::btc_context::initialize(p,&board()).is_err());let s=p.bus_between_stages();
    assert!(s.bus.time-before<1_100_000);assert!(s.bus.packets.len()>10);assert!(s.btc.is_none());assert!(s.failed().is_some());
}));}


fn rfk_notice(state:crate::dack::RfkState)->crate::dack::BtcNotice{crate::dack::BtcNotice{calibration:crate::dack::Calibration::Dack,state,phy_map:0x10}}
#[test]
fn btc_rfk_real_grant_and_stop_require_policy_scoreboard_and_owned_rx(){with_session(|s|owner_phase(s,|p|{
    p.bus_between_stages().bus.bt=2|1<<30;let initialized=crate::btc_context::initialize(p,&board()).unwrap();
    let s=p.bus_between_stages();let first=s.bus.packets.len();
    crate::dack::DackIo::btc_rfk(s,rfk_notice(crate::dack::RfkState::Start),100_000).unwrap();
    assert_eq!(s.bus.get(0xac)&(1<<11),1<<11);assert_eq!(s.bus.lte[0],0x77007700);assert_eq!(s.bus.get(0xc67c)&0xffff,0x100);
    assert!(s.bus.packets[first..].iter().any(|v|v[1]==3&&v.len()>230));assert!(initialized.validate(p,1000).is_err());
    let s=p.bus_between_stages();crate::dack::DackIo::btc_rfk(s,rfk_notice(crate::dack::RfkState::Stop),100_000).unwrap();
    assert_eq!(s.bus.get(0xac)&(1<<11),0);assert_eq!(s.bus.lte[0],0);assert_eq!(s.bus.get(0xc67c)&0xffff,0x166);
    assert_eq!(s.failed(),None);assert_eq!(s.bus.get(fwcmd::DMA_STOP),0xb0f00);initialized.validate(p,1000).unwrap();
}));}
#[test]
fn btc_rfk_bt_off_uses_wlan_only_controls_and_source_off_bt_policy(){with_session(|s|owner_phase(s,|p|{
    crate::btc_context::initialize(p,&board()).unwrap();let s=p.bus_between_stages();let first=s.bus.packets.len();
    crate::dack::DackIo::btc_rfk(s,rfk_notice(crate::dack::RfkState::Start),100_000).unwrap();
    assert_eq!(s.bus.get(0xac)&(1<<11),1<<11);assert_eq!(s.bus.lte[0],0x77007700);
    assert!(!s.bus.packets[first..].iter().any(|v|v[1]==3));
    crate::dack::DackIo::btc_rfk(s,rfk_notice(crate::dack::RfkState::Stop),100_000).unwrap();assert_eq!(s.bus.get(0xac)&(1<<11),0);
}));}
#[test]
fn btc_rfk_waits_for_actual_bt_arbitration_and_never_uses_timeout_override(){for releases in [false,true]{with_session(|s|owner_phase(s,|p|{
    p.bus_between_stages().bus.bt=2|1<<30;crate::btc_context::initialize(p,&board()).unwrap();let s=p.bus_between_stages();
    s.bus.bt|=0x60;let before=s.bus.time;let first=s.bus.packets.len();if releases{s.bus.bt_release_at=Some(before+120);}
    let result=crate::dack::DackIo::btc_rfk(s,rfk_notice(crate::dack::RfkState::Start),2000);
    if releases{result.unwrap();assert!(s.bus.time-before>=120);crate::dack::DackIo::btc_rfk(s,rfk_notice(crate::dack::RfkState::Stop),100_000).unwrap();}
    else{assert!(result.is_err());assert!(s.bus.time-before<=2000);assert_eq!(s.bus.packets.len(),first);assert_eq!(s.bus.get(0xac)&(1<<11),0);}
}));}}
#[test]
fn btc_rfk_missing_policy_ack_never_grants_and_poisoned_stop_only_cleans_raw(){with_session(|s|owner_phase(s,|p|{
    p.bus_between_stages().bus.bt=2|1<<30;crate::btc_context::initialize(p,&board()).unwrap();let s=p.bus_between_stages();
    let saved=s.bus.lte[0];s.bus.missing_at=s.bus.packets.len()+1;
    assert!(crate::dack::DackIo::btc_rfk(s,rfk_notice(crate::dack::RfkState::Start),2000).is_err());let packets=s.bus.packets.len();
    assert!(s.failed().is_some());assert!(crate::dack::DackIo::btc_rfk(s,rfk_notice(crate::dack::RfkState::Stop),100_000).is_err());
    assert_eq!(s.bus.packets.len(),packets);assert_eq!(s.bus.get(0xac)&(1<<11),0);assert_eq!(s.bus.lte[0],saved);
    let failure=crate::btc_context::last_rfk_failure(s).unwrap();assert!(failure.cleanup_attempted);assert_eq!(failure.clear_scoreboard,None);assert_eq!(failure.restore_antenna,None);
}));}
#[test]
fn btc_rfk_radio_off_stop_never_reenables_radio_or_claims_complete_policy(){with_session(|s|owner_phase(s,|p|{
    p.bus_between_stages().bus.bt=2|1<<30;crate::btc_context::initialize(p,&board()).unwrap();let s=p.bus_between_stages();
    let saved=s.bus.lte[0];crate::dack::DackIo::btc_rfk(s,rfk_notice(crate::dack::RfkState::Start),100_000).unwrap();
    s.bus.regs[2]&=!3;s.bus.set(0x2f0,0);let packets=s.bus.packets.len();s.bus.writes.clear();
    assert!(crate::dack::DackIo::btc_rfk(s,rfk_notice(crate::dack::RfkState::Stop),100_000).is_err());
    assert_eq!(s.bus.packets.len(),packets);assert_eq!(s.bus.get(0xac)&(1<<11),0);assert_eq!(s.bus.lte[0],saved);
    assert!(!s.bus.writes.iter().any(|(at,_,_)|*at==2||*at==0x2f0||*at>=0x10000));
    assert_eq!(crate::btc_context::last_rfk_failure(s).unwrap().cause,crate::btc_context::Error::RadioOff);
}));}
#[test]
fn btc_rfk_cleanup_attempts_antenna_even_when_scoreboard_clear_fails(){with_session(|s|owner_phase(s,|p|{
    p.bus_between_stages().bus.bt=2|1<<30;crate::btc_context::initialize(p,&board()).unwrap();let s=p.bus_between_stages();let saved=s.bus.lte[0];
    crate::dack::DackIo::btc_rfk(s,rfk_notice(crate::dack::RfkState::Start),100_000).unwrap();s.abort_coexistence();s.bus.fail_at=Some(0xac);
    assert!(crate::dack::DackIo::btc_rfk(s,rfk_notice(crate::dack::RfkState::Stop),100_000).is_err());
    let failure=crate::btc_context::last_rfk_failure(s).unwrap();assert!(failure.clear_scoreboard.is_some());assert_eq!(failure.restore_antenna,None);assert_eq!(s.bus.lte[0],saved);
}));}
#[test]
fn btc_rfk_requires_initialized_context_and_matching_start_stop_notice(){with_session(|s|{
    let before=s.bus.writes.len();assert!(crate::dack::DackIo::btc_rfk(s,rfk_notice(crate::dack::RfkState::Start),100_000).is_err());assert_eq!(s.bus.writes.len(),before);
    owner_phase(s,|p|{crate::btc_context::initialize(p,&board()).unwrap();let s=p.bus_between_stages();
        crate::dack::DackIo::btc_rfk(s,rfk_notice(crate::dack::RfkState::Start),100_000).unwrap();
        let before=s.bus.writes.len();let mut stop=rfk_notice(crate::dack::RfkState::Stop);stop.calibration=crate::dack::Calibration::Rxdck;
        assert!(crate::dack::DackIo::btc_rfk(s,stop,100_000).is_err());assert_eq!(s.bus.writes.len(),before);
        assert!(crate::dack::DackIo::btc_rfk(s,rfk_notice(crate::dack::RfkState::Stop),100_000).is_err());assert_eq!(s.bus.get(0xac)&(1<<11),0);
    });
});}

#[test]
fn btc_rfk_possibly_applied_grant_and_score_writes_still_release_on_stop(){for at in [0xc67c,0xac]{with_session(|s|owner_phase(s,|p|{
    p.bus_between_stages().bus.bt=2|1<<30;crate::btc_context::initialize(p,&board()).unwrap();let s=p.bus_between_stages();let saved=s.bus.lte[0];
    s.bus.fail_at=Some(at);assert!(crate::dack::DackIo::btc_rfk(s,rfk_notice(crate::dack::RfkState::Start),100_000).is_err());let packets=s.bus.packets.len();
    assert!(crate::dack::DackIo::btc_rfk(s,rfk_notice(crate::dack::RfkState::Stop),100_000).is_err());
    assert_eq!(s.bus.packets.len(),packets);assert_eq!(s.bus.get(0xac)&(1<<11),0);assert_eq!(s.bus.lte[0],saved);
    let failure=crate::btc_context::last_rfk_failure(s).unwrap();assert_eq!(failure.clear_scoreboard,None);assert_eq!(failure.restore_antenna,None);
}));}}
#[test]
fn btc_rfk_done_with_contradictory_later_report_cannot_grant(){with_session(|s|owner_phase(s,|p|{
    p.bus_between_stages().bus.bt=2|1<<30;crate::btc_context::initialize(p,&board()).unwrap();let s=p.bus_between_stages();
    s.bus.after_ack=Some((s.bus.packets.len()+1,policy_report(crate::btc_codec::Policy::NoLink,true)));
    assert!(crate::dack::DackIo::btc_rfk(s,rfk_notice(crate::dack::RfkState::Start),100_000).is_err());
    assert_eq!(crate::btc_context::last_rfk_failure(s).unwrap().cause,crate::btc_context::Error::PolicyMismatch);
    assert!(crate::dack::DackIo::btc_rfk(s,rfk_notice(crate::dack::RfkState::Stop),100_000).is_err());assert_eq!(s.bus.get(0xac)&(1<<11),0);
}));}
#[test]
fn btc_rfk_frozen_clock_and_duplicate_start_are_bounded(){for duplicate in [false,true]{with_session(|s|owner_phase(s,|p|{
    crate::btc_context::initialize(p,&board()).unwrap();let s=p.bus_between_stages();
    if duplicate{crate::dack::DackIo::btc_rfk(s,rfk_notice(crate::dack::RfkState::Start),100_000).unwrap();}
    else{s.bus.bt=0x62;s.bus.frozen=true;}
    let first=s.bus.packets.len();let writes=s.bus.writes.len();
    assert!(crate::dack::DackIo::btc_rfk(s,rfk_notice(crate::dack::RfkState::Start),100_000).is_err());
    assert_eq!(s.bus.packets.len(),first);assert_eq!(s.bus.writes.len(),writes);
    assert!(crate::dack::DackIo::btc_rfk(s,rfk_notice(crate::dack::RfkState::Stop),100_000).is_err());assert_eq!(s.bus.get(0xac)&(1<<11),0);
}));}}

#[test]
fn btc_rfk_stop_policy_timeout_reserves_real_owned_cleanup_within_caller_budget(){with_session(|s|owner_phase(s,|p|{
    p.bus_between_stages().bus.bt=2|1<<30;crate::btc_context::initialize(p,&board()).unwrap();let s=p.bus_between_stages();let saved=s.bus.lte[0];
    crate::dack::DackIo::btc_rfk(s,rfk_notice(crate::dack::RfkState::Start),100_000).unwrap();
    s.bus.missing_at=s.bus.packets.len()+1;let before=s.bus.time;s.bus.writes.clear();
    assert!(crate::dack::DackIo::btc_rfk(s,rfk_notice(crate::dack::RfkState::Stop),100_000).is_err());
    assert!(s.bus.time-before>=80_000);assert!(s.bus.time-before<100_000);
    assert!(s.bus.writes.iter().any(|(at,_,v)|*at==0xac&&v&(1<<11)==0));assert_eq!(s.bus.lte[0],saved);
    let failure=crate::btc_context::last_rfk_failure(s).unwrap();assert!(failure.cleanup_attempted);assert_eq!(failure.clear_scoreboard,None);assert_eq!(failure.restore_antenna,None);
}));}


fn iqk_notice(state:crate::dack::RfkState,band:u8)->crate::dack::BtcNotice{
    let path=if matches!(state,crate::dack::RfkState::OneshotStart|crate::dack::RfkState::OneshotStop){3}else{0};
    crate::dack::BtcNotice{calibration:crate::dack::Calibration::Iqk,state,phy_map:0x10|band<<6|path}
}
#[test]
fn btc_nested_iqk_oneshots_retain_outer_grant_without_replaying_policy(){for band in [0,1]{with_session(|s|owner_phase(s,|p|{
    p.bus_between_stages().bus.bt=2|1<<30;crate::btc_context::initialize(p,&board()).unwrap();let s=p.bus_between_stages();
    crate::dack::DackIo::btc_rfk(s,iqk_notice(crate::dack::RfkState::Start,band),100_000).unwrap();
    let packets=s.bus.packets.len();let writes=s.bus.writes.len();
    for _ in 0..2{crate::dack::DackIo::btc_rfk(s,iqk_notice(crate::dack::RfkState::OneshotStart,band),100_000).unwrap();
        crate::dack::DackIo::btc_rfk(s,iqk_notice(crate::dack::RfkState::OneshotStop,band),100_000).unwrap();}
    assert_eq!(s.bus.packets.len(),packets);assert_eq!(s.bus.writes.len(),writes);assert_eq!(s.bus.get(0xac)&(1<<11),1<<11);
    crate::dack::DackIo::btc_rfk(s,iqk_notice(crate::dack::RfkState::Stop,band),100_000).unwrap();assert_eq!(s.bus.get(0xac)&(1<<11),0);
}));}}
#[test]
fn btc_nested_orphan_start_or_stop_never_grants(){for state in [crate::dack::RfkState::OneshotStart,crate::dack::RfkState::OneshotStop]{with_session(|s|owner_phase(s,|p|{
    crate::btc_context::initialize(p,&board()).unwrap();let s=p.bus_between_stages();let writes=s.bus.writes.len();let packets=s.bus.packets.len();
    assert!(crate::dack::DackIo::btc_rfk(s,iqk_notice(state,0),100_000).is_err());assert_eq!(s.bus.writes.len(),writes);assert_eq!(s.bus.packets.len(),packets);
}));}}
#[test]
fn btc_nested_wrong_band_or_calibration_does_not_release_held_grant(){for wrong in 0..2{with_session(|s|owner_phase(s,|p|{
    crate::btc_context::initialize(p,&board()).unwrap();let s=p.bus_between_stages();
    crate::dack::DackIo::btc_rfk(s,iqk_notice(crate::dack::RfkState::Start,0),100_000).unwrap();
    let mut nested=iqk_notice(crate::dack::RfkState::OneshotStart,wrong);if wrong==0{nested.calibration=crate::dack::Calibration::Dpk;}
    let writes=s.bus.writes.len();assert!(crate::dack::DackIo::btc_rfk(s,nested,100_000).is_err());assert_eq!(s.bus.writes.len(),writes);
    let _=crate::dack::DackIo::btc_rfk(s,iqk_notice(crate::dack::RfkState::Stop,0),100_000);assert_eq!(s.bus.get(0xac)&(1<<11),0);
}));}}
#[test]
fn btc_nested_duplicate_and_mismatched_stop_keep_matching_cleanup_possible(){for duplicate in [false,true]{with_session(|s|owner_phase(s,|p|{
    crate::btc_context::initialize(p,&board()).unwrap();let s=p.bus_between_stages();
    crate::dack::DackIo::btc_rfk(s,iqk_notice(crate::dack::RfkState::Start,0),100_000).unwrap();
    crate::dack::DackIo::btc_rfk(s,iqk_notice(crate::dack::RfkState::OneshotStart,0),100_000).unwrap();
    let bad=if duplicate{iqk_notice(crate::dack::RfkState::OneshotStart,0)}else{iqk_notice(crate::dack::RfkState::OneshotStop,1)};
    assert!(crate::dack::DackIo::btc_rfk(s,bad,100_000).is_err());assert_eq!(s.bus.get(0xac)&(1<<11),1<<11);
    assert!(crate::dack::DackIo::btc_rfk(s,iqk_notice(crate::dack::RfkState::OneshotStop,0),100_000).is_err());
    assert!(crate::dack::DackIo::btc_rfk(s,iqk_notice(crate::dack::RfkState::Stop,0),100_000).is_err());assert_eq!(s.bus.get(0xac)&(1<<11),0);
}));}}
#[test]
fn btc_nested_outer_stop_cannot_silently_drop_an_active_oneshot(){with_session(|s|owner_phase(s,|p|{
    crate::btc_context::initialize(p,&board()).unwrap();let s=p.bus_between_stages();
    crate::dack::DackIo::btc_rfk(s,iqk_notice(crate::dack::RfkState::Start,0),100_000).unwrap();
    crate::dack::DackIo::btc_rfk(s,iqk_notice(crate::dack::RfkState::OneshotStart,0),100_000).unwrap();
    let writes=s.bus.writes.len();assert!(crate::dack::DackIo::btc_rfk(s,iqk_notice(crate::dack::RfkState::Stop,0),100_000).is_err());
    assert_eq!(s.bus.writes.len(),writes);assert_eq!(s.bus.get(0xac)&(1<<11),1<<11);
    let _=crate::dack::DackIo::btc_rfk(s,iqk_notice(crate::dack::RfkState::OneshotStop,0),100_000);
    let _=crate::dack::DackIo::btc_rfk(s,iqk_notice(crate::dack::RfkState::Stop,0),100_000);assert_eq!(s.bus.get(0xac)&(1<<11),0);
}));}
#[test]
fn btc_nested_failed_admission_and_radio_off_close_without_reenabling_rf(){with_session(|s|owner_phase(s,|p|{
    crate::btc_context::initialize(p,&board()).unwrap();let s=p.bus_between_stages();
    crate::dack::DackIo::btc_rfk(s,iqk_notice(crate::dack::RfkState::Start,0),100_000).unwrap();
    s.bus.bt=0x62;assert!(crate::dack::DackIo::btc_rfk(s,iqk_notice(crate::dack::RfkState::OneshotStart,0),100_000).is_err());
    s.bus.regs[2]&=!3;s.bus.set(0x2f0,0);s.bus.writes.clear();
    assert!(crate::dack::DackIo::btc_rfk(s,iqk_notice(crate::dack::RfkState::OneshotStop,0),100_000).is_err());
    assert!(crate::dack::DackIo::btc_rfk(s,iqk_notice(crate::dack::RfkState::Stop,0),100_000).is_err());
    assert!(!s.bus.writes.iter().any(|(at,_,_)|*at==2||*at==0x2f0||*at>=0x10000));assert_eq!(s.bus.get(0xac)&(1<<11),0);
}));}
#[test]
fn btc_nested_dpk_uses_real_outer_grant_and_cannot_borrow_iqk_oneshot(){with_session(|s|owner_phase(s,|p|{
    crate::btc_context::initialize(p,&board()).unwrap();let s=p.bus_between_stages();let mut start=rfk_notice(crate::dack::RfkState::Start);start.calibration=crate::dack::Calibration::Dpk;
    crate::dack::DackIo::btc_rfk(s,start,100_000).unwrap();assert_eq!(s.bus.get(0xac)&(1<<11),1<<11);
    assert!(crate::dack::DackIo::btc_rfk(s,iqk_notice(crate::dack::RfkState::OneshotStart,0),100_000).is_err());
    start.state=crate::dack::RfkState::Stop;assert!(crate::dack::DackIo::btc_rfk(s,start,100_000).is_err());assert_eq!(s.bus.get(0xac)&(1<<11),0);
}));}

#[test]
fn btc_nested_fresh_whql_state_refuses_but_keeps_matching_cleanup(){with_session(|s|owner_phase(s,|p|{
    crate::btc_context::initialize(p,&board()).unwrap();let s=p.bus_between_stages();
    crate::dack::DackIo::btc_rfk(s,iqk_notice(crate::dack::RfkState::Start,0),100_000).unwrap();s.bus.bt=2|4;
    let writes=s.bus.writes.len();assert!(crate::dack::DackIo::btc_rfk(s,iqk_notice(crate::dack::RfkState::OneshotStart,0),100_000).is_err());
    assert_eq!(s.bus.writes.len(),writes);assert_eq!(crate::btc_context::last_rfk_failure(s).unwrap().cause,crate::btc_context::Error::Whql);
    let _=crate::dack::DackIo::btc_rfk(s,iqk_notice(crate::dack::RfkState::OneshotStop,0),100_000);
    let _=crate::dack::DackIo::btc_rfk(s,iqk_notice(crate::dack::RfkState::Stop,0),100_000);assert_eq!(s.bus.get(0xac)&(1<<11),0);
}));}

#[test]
fn owned_scheduler_uses_real_register_exchange_without_consuming_packet_sequence(){with_session(|s|{
    use crate::rxdck::RxdckIo;
    let before=s.bus.regs[R_AX_UDM1+1];let command=s.command_session();
    s.scheduler_zero(1000).unwrap();s.scheduler_zero(1000).unwrap();
    let sent:Vec<_>=s.bus.writes.iter().filter(|(at,_,v)|*at==R_AX_H2CREG_CTRL&&*v==1).collect();assert_eq!(sent.len(),2);
    assert_eq!(s.bus.get(R_AX_H2CREG_DATA0),0x205);assert_eq!(s.bus.get(R_AX_H2CREG_DATA0+4),0xffff);
    assert_eq!(s.bus.get(R_AX_H2CREG_DATA0+8),0);assert_eq!(s.bus.get(R_AX_H2CREG_DATA0+12),0);
    let hmask=(B_AX_UDM1_HALMAC_H2C_DEQ_CNT_MASK>>8)as u8;let cmask=(B_AX_UDM1_HALMAC_C2H_ENQ_CNT_MASK>>8)as u8;
    for mask in [hmask,cmask]{let shift=mask.trailing_zeros();assert_eq!((s.bus.regs[R_AX_UDM1+1]&mask)>>shift,(((before&mask)>>shift)+2)&(mask>>shift));}
    assert_eq!(s.bus.regs[R_AX_C2HREG_CTRL],0);assert_eq!(s.bus.get(R_AX_CTN_TXEN),0);assert_eq!(s.bus.get(fwcmd::DMA_STOP),0xb0f00);
    assert_eq!(s.command_session(),command);assert_eq!(s.submitted(),0);assert!(s.bus.packets.is_empty());assert_eq!(s.failed(),None);
});}
#[test]
fn owned_scheduler_invalid_budget_does_not_touch_hardware_or_poison(){with_session(|s|{
    use crate::rxdck::RxdckIo;let writes=s.bus.writes.len();
    for budget in [0,1_000_001]{assert_eq!(s.scheduler_zero(budget).unwrap_err().kind,FaultKind::Access);}
    assert_eq!(s.bus.writes.len(),writes);assert_eq!(s.failed(),None);s.scheduler_zero(1000).unwrap();
});}
#[test]
fn owned_scheduler_actual_queue_and_receive_guard_precedes_register_mailbox(){for broken in [0x1108,download::FW_CONTROL,fwcmd::BASE_LOW]{with_session(|s|{
    use crate::rxdck::RxdckIo;s.bus.set(broken,0);s.bus.writes.clear();assert!(s.scheduler_zero(1000).is_err());
    assert!(s.bus.writes.is_empty());assert!(s.failed().is_some());
});}}
#[test]
fn owned_scheduler_failure_poisons_without_retry_and_retains_raw_shutdown(){for reply in [1,2,3,4]{with_session(|s|{
    use crate::rxdck::RxdckIo;s.bus.scheduler_reply=reply;assert!(s.scheduler_zero(1000).is_err());assert!(s.failed().is_some());
    assert_eq!(s.bus.writes.iter().filter(|(at,_,v)|*at==R_AX_H2CREG_CTRL&&*v==1).count(),1);
    let n=s.bus.writes.len();assert!(s.scheduler_zero(1000).is_err());assert_eq!(s.bus.writes.len(),n);assert_eq!(PhyIo::command_session(s),0);
    s.write32(fwcmd::DMA_STOP,0x1f0f03).unwrap();assert_eq!(s.bus.get(fwcmd::DMA_STOP),0x1f0f03);assert_eq!(s.submitted(),0);
});}}
#[test]
fn owned_scheduler_pending_response_is_not_discarded_or_replaced(){with_session(|s|{
    use crate::rxdck::RxdckIo;s.bus.regs[R_AX_C2HREG_CTRL]=1;let n=s.bus.writes.len();
    assert!(s.scheduler_zero(1000).is_err());assert_eq!(s.bus.writes.len(),n);assert_eq!(s.bus.regs[R_AX_C2HREG_CTRL],1);
});}
#[test]
fn owned_scheduler_absolute_budget_includes_live_validation_and_published_command(){for late_guard in [true,false]{with_session(|s|{
    use crate::rxdck::RxdckIo;
    if late_guard{s.bus.index_reads=0;s.bus.late_index=1;}else{s.bus.scheduler_us=1000;}
    assert!(s.scheduler_zero(1000).is_err());assert!(s.failed().is_some());
    assert_eq!(s.bus.writes.iter().filter(|(at,_,v)|*at==R_AX_H2CREG_CTRL&&*v==1).count(),usize::from(!late_guard));
    if !late_guard{assert_eq!(s.bus.regs[R_AX_C2HREG_CTRL],1);}
});}}
#[test]
fn owned_scheduler_applied_doorbell_failure_is_never_retried(){with_session(|s|{
    use crate::rxdck::RxdckIo;s.bus.fail_at=Some(R_AX_H2CREG_CTRL);
    assert_eq!(s.scheduler_zero(1000).unwrap_err().kind,FaultKind::Access);let n=s.bus.writes.len();
    assert!(s.scheduler_zero(1000).is_err());assert_eq!(s.bus.writes.len(),n);assert_eq!(s.bus.regs[R_AX_C2HREG_CTRL],1);
});}

#[test]
fn connection_notifications_keep_host_state_and_never_replay_start_or_end(){with_session(|s|owner_phase(s,|p|{
    crate::btc_context::initialize(p,&board()).unwrap();let s=p.bus_between_stages();let first=s.bus.packets.len();let writes=s.bus.writes.len();
    s.connection_rfk(true,1000).unwrap();s.connection_rfk(false,1000).unwrap();assert_eq!(s.bus.packets.len(),first);assert_eq!(s.bus.writes.len(),writes);
    assert!(s.connection_rfk(false,1000).is_err());assert!(s.connection_rfk(true,1000).is_err());assert_eq!(s.bus.packets.len(),first);
}));}
#[test]
fn connection_callbacks_require_actual_initialized_owner_and_order(){
    with_session(|s|{let writes=s.bus.writes.len();assert!(s.connection_rfk(true,1000).is_err());assert!(s.preserve_bt_time(1000).is_err());assert!(s.notify_mcc(6,1000).is_err());assert_eq!(s.bus.writes.len(),writes);});
    for action in 0..3{with_session(|s|owner_phase(s,|p|{crate::btc_context::initialize(p,&board()).unwrap();let s=p.bus_between_stages();let first=s.bus.packets.len();
        let result=match action{0=>s.connection_rfk(false,1000),1=>s.preserve_bt_time(1000),_=>s.notify_mcc(6,1000)};assert!(result.is_err());assert_eq!(s.bus.packets.len(),first);
    }));}
}
#[test]
fn connection_end_clears_matching_attempt_even_after_begin_loses_live_rx(){with_session(|s|owner_phase(s,|p|{
    crate::btc_context::initialize(p,&board()).unwrap();let s=p.bus_between_stages();s.bus.command=2;
    assert!(s.connection_rfk(true,1000).is_err());let first=s.bus.writes.len();let original=s.failed().unwrap();
    assert_eq!(s.connection_rfk(false,1000),Err(crate::btc_context::Error::Session(original)));
    assert_eq!(s.connection_rfk(false,1000),Err(crate::btc_context::Error::Order));assert_eq!(s.bus.writes.len(),first);assert_eq!(s.failed(),Some(original));
    s.bus.command=6;
}));}
#[test]
fn connection_preserve_waits_thirty_elapsed_ms_only_for_actual_a2dp(){for profiles in [0,0x10,0x20,0x40,0x80,0xf0]{with_session(|s|owner_phase(s,|p|{
    p.bus_between_stages().bus.bt=2|1<<30;crate::btc_context::initialize(p,&board()).unwrap();let s=p.bus_between_stages();s.connection_rfk(true,1000).unwrap();
    s.bus.inject(0,&c2h_bytes(2,0x12,1,&[0,6,profiles,0,0,0,0,0]));let before=s.bus.time;let first=s.bus.packets.len();
    s.preserve_bt_time(100_000).unwrap();assert_eq!(s.bus.time-before,if profiles&0x40!=0{30_000}else{0});assert_eq!(s.pending_events(),0);assert_eq!(s.bus.packets.len(),first);
    s.connection_rfk(false,1000).unwrap();
}));}}
#[test]
fn connection_preserve_frozen_clock_insufficient_budget_and_regression_are_errors(){for mode in 0..3{with_session(|s|owner_phase(s,|p|{
    p.bus_between_stages().bus.bt=2|1<<30;crate::btc_context::initialize(p,&board()).unwrap();let s=p.bus_between_stages();s.connection_rfk(true,1000).unwrap();
    s.bus.inject(0,&c2h_bytes(2,0x12,1,&[0,6,0x40,0,0,0,0,0]));let first=s.bus.packets.len();
    if mode==0{s.bus.frozen=true;}if mode==2{s.bus.time=0;}
    assert!(s.preserve_bt_time(if mode==1{30_000}else{100_000}).is_err());assert!(s.failed().is_some());assert_eq!(s.bus.packets.len(),first);
    assert!(s.connection_rfk(false,1000).is_err());assert_eq!(s.connection_rfk(false,1000),Err(crate::btc_context::Error::Order));s.bus.frozen=false;
}));}}
#[test]
fn connection_window_refuses_pending_rfk_and_malformed_bt_events(){for pending in [false,true]{with_session(|s|owner_phase(s,|p|{
    crate::btc_context::initialize(p,&board()).unwrap();let s=p.bus_between_stages();s.connection_rfk(true,1000).unwrap();
    if pending{crate::dack::DackIo::btc_rfk(s,rfk_notice(crate::dack::RfkState::Start),100_000).unwrap();}
    else{s.bus.inject(0,&c2h_bytes(2,0x12,1,&[0,5,0x40,0,0,0,0,0]));}
    assert!(s.preserve_bt_time(100_000).is_err());assert!(s.failed().is_some());assert!(s.connection_rfk(false,1000).is_err());assert!(s.notify_mcc(6,1000).is_err());
}));}}
#[test]
fn connection_mcc_uses_initial_52b_payload_actual_sequence_and_only_periodic_receipt(){for align in 0..4{with_session(|s|owner_phase(s,|p|{
    crate::btc_context::initialize(p,&board()).unwrap();let s=p.bus_between_stages();for _ in 0..align{s.command(command(),&[0],both(),1000).unwrap();}
    s.connection_rfk(true,1000).unwrap();s.connection_rfk(false,1000).unwrap();let sequence=s.submitted();let first=s.bus.packets.len();
    s.notify_mcc(6,100_000).unwrap();let packet=s.bus.packets.last().unwrap();assert_eq!(s.bus.packets.len(),first+1);
    assert_eq!(u32::from_le_bytes(packet[..4].try_into().unwrap()),2|0xa<<2|2<<8|u32::from(sequence)<<24);
    assert_eq!(packet[5]&0xc0,if sequence%4==0{0x40}else{0});
    assert_eq!(&packet[8..],&[6,0,0,0,6,0,0,0,0,0,0,0,0,0,0,0,6,0,0,0]);assert_eq!(s.queue.pending(),0);
    assert!(s.notify_mcc(6,1000).is_err());assert_eq!(s.bus.packets.len(),first+1);
}));}}
#[test]
fn connection_mcc_uncertain_publication_missing_receipt_and_busy_dma_never_retry(){for mode in 0..4{with_session(|s|owner_phase(s,|p|{
    crate::btc_context::initialize(p,&board()).unwrap();let s=p.bus_between_stages();while s.submitted()%4!=0{s.command(command(),&[0],both(),1000).unwrap();}
    s.connection_rfk(true,1000).unwrap();s.connection_rfk(false,1000).unwrap();let first=s.bus.packets.len();
    match mode{0=>s.bus.reply=Reply::Missing,1=>s.bus.fail_at=Some(fwcmd::INDEX),2=>s.bus.busy=true,_=>s.bus.packet_us=100_000}
    assert!(s.notify_mcc(6,1000).is_err());assert!(s.failed().is_some());assert_eq!(s.bus.packets.len(),first+1);
    assert!(s.notify_mcc(6,1000).is_err());assert_eq!(s.bus.packets.len(),first+1);s.bus.set(fwcmd::DMA_BUSY,0);
}));}}

#[test]
fn connection_preserve_consumes_later_events_but_keeps_the_requested_snapshot_window(){with_session(|s|owner_phase(s,|p|{
    p.bus_between_stages().bus.bt=2|1<<30;crate::btc_context::initialize(p,&board()).unwrap();let s=p.bus_between_stages();s.connection_rfk(true,1000).unwrap();
    s.bus.inject(0,&c2h_bytes(2,0x12,1,&[0,6,0x40,0,0,0,0,0]));let before=s.bus.time;
    s.bus.delay_event=Some((before+10_000,c2h_bytes(2,0x12,1,&[0,6,0,0,0,0,0,0])));
    s.preserve_bt_time(100_000).unwrap();assert_eq!(s.bus.time-before,30_000);assert!(s.bus.delay_event.is_none());assert_eq!(s.pending_events(),0);
    let before=s.bus.time;s.preserve_bt_time(1000).unwrap();assert_eq!(s.bus.time,before);s.connection_rfk(false,1000).unwrap();
}));}
#[test]
fn connection_preserve_does_not_accept_an_oversleep_after_requested_thirty_ms(){with_session(|s|owner_phase(s,|p|{
    p.bus_between_stages().bus.bt=2|1<<30;crate::btc_context::initialize(p,&board()).unwrap();let s=p.bus_between_stages();s.connection_rfk(true,1000).unwrap();
    s.bus.inject(0,&c2h_bytes(2,0x12,1,&[0,6,0x40,0,0,0,0,0]));s.bus.delay_extra=100_000;
    assert!(s.preserve_bt_time(100_000).is_err());assert!(s.failed().is_some());s.bus.delay_extra=0;
}));}
