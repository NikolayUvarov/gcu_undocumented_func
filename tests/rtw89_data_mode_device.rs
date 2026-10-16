use crate::{btc_session,fwcmd,io::{self,RegisterIo},tx};
use super::{initial,station_device};
use std::{cell::RefCell,rc::Rc};

#[derive(Default)]
pub(super) struct Control{pub armed:bool,pub fault:u8,pub burst:bool,pub old:usize,pub corrupt:bool,pub published:usize,pub unknown:bool,pub advance_us:u64,
    pub incoming:Vec<Vec<u8>>,pub filter_fault:u8,pub cam_packets:Vec<Vec<u8>>,pub filter_writes:usize,
    pub selector:u32,pub rx_filter:u32,pub cam_wifi:usize,pub control_event:bool}
pub(super) struct Region{pub base:*mut u8}
impl Region{pub fn new()->Self{let base=unsafe{std::alloc::alloc_zeroed(std::alloc::Layout::from_size_align(tx::DMA_BYTES,4096).unwrap())};assert!(!base.is_null());Self{base}}}
impl Drop for Region{fn drop(&mut self){unsafe{std::alloc::dealloc(self.base,std::alloc::Layout::from_size_align(tx::DMA_BYTES,4096).unwrap())}}}
pub(super) struct Device{
    pub station:station_device::Device,pub control:Rc<RefCell<Control>>,tx:*mut u8,pub active:bool,
    pub published:Vec<Vec<u8>>,pub scheduler:usize,pub shutdowns:usize,
}
impl Device{
    pub fn new(base:initial::Device,region:&Region,control:Rc<RefCell<Control>>)->Self{
        Self{station:station_device::Device::new(base),control,tx:region.base,active:false,published:vec![],scheduler:0,shutdowns:0}
    }
    fn raw(&mut self)->&mut crate::model::Device{&mut self.station.base.inner}
    fn inject_old(&mut self){let count=std::mem::take(&mut self.control.borrow_mut().old);
        for _ in 0..count{self.wifi(true,false);}
        let incoming=std::mem::take(&mut self.control.borrow_mut().incoming);
        for mut frame in incoming{frame.extend_from_slice(&[0;4]);self.raw().inject_packet(0,0,&frame);}
        if std::mem::take(&mut self.control.borrow_mut().control_event){
            let mut event=[0;12];event[0]=3;event[1]=0x77;event[4]=12;event[8]=0xa5;self.raw().inject_c2h(&event);
        }}
    fn wifi(&mut self,own:bool,bad:bool){let mut f=super::frame();f.extend_from_slice(&[0;4]);
        f[4..10].copy_from_slice(if own{&[2,3,4,5,6,7]}else{&[2,8,7,6,5,4]});if bad{f[0]=255;}
        self.raw().inject_packet(0,0,&f);
    }
    fn dma(&self,physical:u32,length:usize)->&[u8]{
        let at=(physical as usize).checked_sub(0x1000000).unwrap();assert!(at.checked_add(length).is_some_and(|end|end<=tx::DMA_BYTES));
        unsafe{std::slice::from_raw_parts(self.tx.add(at),length)}
    }
    fn word(bytes:&[u8],at:usize)->u32{u32::from_le_bytes(bytes[at..at+4].try_into().unwrap())}
    fn transmit(&mut self,q:usize,value:u32){
        const BASE:[usize;5]=[0x1110,0x1118,0x1120,0x1128,0x1150];const INDEX:[usize;5]=[0x1058,0x105c,0x1060,0x1064,0x1078];
        assert!(self.active);assert!(value<32);let slot=(value as usize+31)%32;let physical=self.raw().get(BASE[q],4)+slot as u32*8;
        let bd=self.dma(physical,8);assert_eq!(Self::word(bd,0),64|1<<30);let wd=self.dma(Self::word(bd,4),64).to_vec();
        let length=u16::from_le_bytes(wd[56..58].try_into().unwrap())as usize;assert!((24..=2346).contains(&length));
        let f=self.dma(Self::word(&wd,60),length).to_vec();self.published.push(f);self.control.borrow_mut().published+=1;
        self.raw().put(INDEX[q],4,value|value<<16);
        let seq=u16::from_le_bytes(wd[48..50].try_into().unwrap())&0x7fff;
        let ctl=Self::word(&wd,8);let rpq=u32::from(if self.control.borrow().fault==14{seq+1}else{seq})<<16|((ctl>>17)&31)<<8|(ctl>>24);
        self.raw().inject_packet(1,7,&rpq.to_le_bytes());
        if self.control.borrow().fault!=7{
            let mut c2h=[0;32];c2h[..4].copy_from_slice(&(1u32|9<<2|1<<8).to_le_bytes());c2h[4]=32;
            c2h[8..12].copy_from_slice(&(u32::from(seq&15)<<8).to_le_bytes());c2h[20..24].copy_from_slice(&256u32.to_le_bytes());
            self.raw().inject_c2h(&c2h);
        }
    }
    fn write(&mut self,at:usize,width:usize,value:u32)->io::Result<()>{
        let(armed,fault)={let c=self.control.borrow();(c.armed,c.fault)};
        let filter_fault=self.control.borrow().filter_fault;
        if armed&&self.active&&at==fwcmd::INDEX&&width==2{
            if filter_fault==1{self.raw().hold_header=Some((1,6,0));}
            if filter_fault==2{self.raw().normal_ack=crate::model::NormalAck::Missing;}
            let count=std::mem::take(&mut self.control.borrow_mut().cam_wifi);
            for _ in 0..count{self.wifi(true,false);}
        }
        if armed&&self.active&&filter_fault==6&&!self.control.borrow().cam_packets.is_empty()&&at==0xc04&&value==0x18888000{
            return Err(io::Fault::new(io::FaultKind::Access,at,value));
        }
        let enable=armed&&at==fwcmd::DMA_STOP&&value&0x001f0f03==0x20000;
        if enable&&fault==1{return Ok(())}
        let scheduler=armed&&at==0x8160&&width==1&&value==1&&self.raw().get(0x8140,4)==0x010f0205;
        if self.active&&at==fwcmd::INDEX&&width==2&&std::mem::take(&mut self.control.borrow_mut().burst){
            for _ in 0..12{self.wifi(true,false);}for _ in 0..3{self.wifi(false,false);}self.wifi(true,true);
        }
        if self.active&&at==fwcmd::INDEX&&width==2&&std::mem::take(&mut self.control.borrow_mut().unknown){
            for n in 0..5{let mut event=[0;12];event[0]=3;event[1]=0x77;event[4]=12;event[8]=n;self.raw().inject_c2h(&event);}
        }
        let before=self.station.station_packets.len();
        match width{1=>self.station.write8(at,value as u8)?,2=>self.station.write16(at,value as u16)?,_=>self.station.write32(at,value)?}
        if armed&&self.active{
            let packets=self.station.station_packets[before..].iter().filter(|p|(p[0]&3,p[0]>>2,p[1])==(1,6,0)).cloned().collect::<Vec<_>>();
            self.control.borrow_mut().cam_packets.extend(packets);
            if at==0xce20{
                self.control.borrow_mut().filter_writes+=1;
                if filter_fault==3{self.raw().put(at,4,value^0x4000);}
                if filter_fault==5{self.raw().now+=100_000;}
                if filter_fault==4{return Err(io::Fault::new(io::FaultKind::Access,at,value));}
            }
            let selector=self.station.selector;let filter=self.raw().get(0xce20,4);
            let mut c=self.control.borrow_mut();c.selector=selector;c.rx_filter=filter;
        }
        if enable{self.active=true;if fault==2{return Err(io::Fault::new(io::FaultKind::Access,at,value))}if fault==5{self.raw().now+=1_000_000;}}
        if scheduler{self.scheduler+=1;if fault==3{self.raw().put(0x8150,4,0x203);}if fault==4{self.raw().put(0x8164,1,0);}if fault==6{self.raw().put(0xc348,4,0xffff);}}
        if self.active&&width==2{if let Some(q)=[0x1058,0x105c,0x1060,0x1064,0x1078].iter().position(|v|*v==at){self.transmit(q,value);}}
        if armed&&at==2&&width==1&&value&3==0{self.shutdowns+=1;}
        Ok(())
    }
}
impl RegisterIo for Device{
    fn read8(&mut self,at:usize)->io::Result<u8>{self.inject_old();let c=self.control.borrow();
        if c.armed&&((c.fault==9&&at==2)||(c.fault==12&&at==0x8040)){return Ok(0)}drop(c);self.station.read8(at)}
    fn read16(&mut self,at:usize)->io::Result<u16>{self.inject_old();self.station.read16(at)}
    fn read32(&mut self,at:usize)->io::Result<u32>{self.inject_old();
        if self.control.borrow().armed&&matches!((self.control.borrow().fault,at),(10,0x200)|(11,0x2f0)|(13,0x10734)){return Ok(0)}
        if self.control.borrow().corrupt&&at==0x1110{return Ok(self.raw().get(at,4)^4096)}self.station.read32(at)}
    fn write8(&mut self,at:usize,v:u8)->io::Result<()>{self.write(at,1,u32::from(v))}
    fn write16(&mut self,at:usize,v:u16)->io::Result<()>{self.write(at,2,u32::from(v))}
    fn write32(&mut self,at:usize,v:u32)->io::Result<()>{self.write(at,4,v)}
    fn now_us(&mut self)->u64{let delta=std::mem::take(&mut self.control.borrow_mut().advance_us);self.raw().now+=delta;self.station.now_us()}
    fn delay_us(&mut self,us:u32)->io::Result<()>{self.station.delay_us(us)}
}
impl fwcmd::Bus for Device{fn pci_command(&mut self)->io::Result<u16>{self.station.pci_command()}}
impl btc_session::DeviceIo for Device{fn pci_id(&mut self)->io::Result<u32>{Ok(0xb85210ec)}fn bar_len(&self)->usize{self.station.bar}}
