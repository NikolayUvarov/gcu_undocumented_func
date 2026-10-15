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
}
impl Device{
    fn new(tx:*mut u8,tx_bytes:usize,index:u32)->Self{
        let mut d=Self{regs:vec![0;runtime_mac::BAR_BYTES],xtal:[0;256],command:6,time:0,frozen:false,writes:Vec::new(),
            tx,tx_bytes,rx:core::ptr::null_mut(),packets:Vec::new(),reply:Reply::Normal,events:0,hold:false,busy:false,late_index:0,index_reads:0,reverse_index:0};
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
        assert_eq!((w1&0x3fff)as usize,payload.len());self.packets.push(payload);
        if !self.hold{self.set(fwcmd::INDEX,next|next<<16);}
        if self.busy{self.set(fwcmd::DMA_BUSY,TX_BUSY);}
        for n in 0..self.events{self.inject(0,&c2h_bytes(2,0x12,0,&[n as u8,0xaa]));}
        if self.reply==Reply::Missing{return}
        let mut seq=(w0>>24)as u8;if self.reply==Reply::WrongSequence{seq=seq.wrapping_add(1);}
        let identity=w0&0xffff;let channel=usize::from(self.reply==Reply::WrongChannel);
        if self.reply==Reply::Malformed{self.inject(0,&[0,0]);return}
        if w1&(1<<14)!=0{self.inject(channel,&c2h_bytes(1,0,0,&(identity|u32::from(seq)<<16).to_le_bytes()));}
        if w1&(1<<15)!=0{let status=if self.reply==Reply::Rejected{9}else{0};self.inject(channel,&c2h_bytes(1,0,1,&(identity|status<<16|u32::from(seq)<<24).to_le_bytes()));}
    }
    fn apply(&mut self,at:usize,width:usize,value:u32)->io::Result<()>{
        self.writes.push((at,width,value));let previous=self.get(at&!3);self.regs[at..at+width].copy_from_slice(&value.to_le_bytes()[..width]);
        if at==0x1018{self.set(0x1050,0);self.set(0x1054,0);self.set(at,0);}
        if at==R_AX_WLAN_XTAL_SI_CTRL{let offset=(value&255)as usize;if value>>24&7==XTAL_SI_NORMAL_READ{self.regs[at+1]=self.xtal[offset];}else{let mask=(value>>16)as u8;self.xtal[offset]=(self.xtal[offset]&!mask)|((value>>8)as u8&mask);}self.regs[at+3]&=!0x80;}
        if at==R_AX_H2CREG_CTRL&&value==1{assert_eq!(self.get(R_AX_H2CREG_DATA0),0x205);assert_eq!(self.get(R_AX_H2CREG_DATA0+4),0xffff);self.regs[at]=0;self.regs[R_AX_C2HREG_CTRL]=1;self.set(R_AX_C2HREG_DATA0,0x204);self.set(R_AX_CTN_TXEN,0);}
        if at==R_AX_SS_CTRL&&value&1!=0{self.regs[at+3]|=0x80;}
        if at==R_AX_ADDR_CAM_CTRL{self.regs[at+1]&=!1;}
        if at==R_AX_RESPBA_CAM_CTRL{self.regs[at]&=!3;}
        if at==fwcmd::INDEX{assert_eq!(width,2);self.consume(value);}
        if [0x1050,0x1054].contains(&at)&&width==2&&!self.rx.is_null(){
            let channel=usize::from(at==0x1054);let host=(previous&0xffff)as usize;assert!(host<rx::DESCRIPTORS);
            let offset=rx::BUFFERS_AT+(channel*rx::DESCRIPTORS+host)*rx::BUFFER_STRIDE;
            unsafe{core::ptr::write_bytes(self.rx.add(offset),0xdd,rx::BUFFER_BYTES);}
        }
        Ok(())
    }
}
impl RegisterIo for Device{
    fn read8(&mut self,a:usize)->io::Result<u8>{Ok(self.regs[a])}
    fn read16(&mut self,a:usize)->io::Result<u16>{Ok(u16::from_le_bytes(self.regs[a..a+2].try_into().unwrap()))}
    fn read32(&mut self,a:usize)->io::Result<u32>{if a==fwcmd::INDEX{self.index_reads+=1;if self.late_index==self.index_reads{self.time+=100_000;}if self.reverse_index==self.index_reads{self.time=0;}}Ok(self.get(a))}
    fn write8(&mut self,a:usize,v:u8)->io::Result<()>{self.apply(a,1,v as u32)}
    fn write16(&mut self,a:usize,v:u16)->io::Result<()>{self.apply(a,2,v as u32)}
    fn write32(&mut self,a:usize,v:u32)->io::Result<()>{self.apply(a,4,v)}
    fn now_us(&mut self)->u64{self.time}
    fn delay_us(&mut self,us:u32)->io::Result<()>{if !self.frozen{self.time+=u64::from(us)}Ok(())}
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
