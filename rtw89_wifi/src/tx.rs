// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! Retained RTL8852B raw-MPDU rings; native activation awaits actual channel and station admission.

use core::{marker::PhantomData,ptr::NonNull,sync::atomic::{fence,Ordering}};
use crate::{fwcmd::{self,Bus},io::{self,Fault,FaultKind,RegisterIo,TimedIo},rx,txdesc::{self,Frame,Status}};

pub(crate) const DESCRIPTORS:usize=32;
pub(crate) const BUFFERS:usize=32;
pub(crate) const BUFFERS_AT:usize=4096;
pub(crate) const STRIDE:usize=4096;
pub(crate) const PAYLOAD_AT:usize=128;
pub(crate) const DMA_BYTES:usize=BUFFERS_AT+BUFFERS*STRIDE;
const CHANNELS:usize=5;
const INDEX:[usize;CHANNELS]=[0x1058,0x105c,0x1060,0x1064,0x1078];
const NUMBER:[usize;CHANNELS]=[0x1024,0x1026,0x1028,0x102a,0x1034];
const BASE:[usize;CHANNELS]=[0x1110,0x1118,0x1120,0x1128,0x1150];
const BDRAM:[usize;CHANNELS]=[0x1200,0x1204,0x1208,0x120c,0x1220];
const RAM:[u32;CHANNELS]=[5<<8|2<<16,5|5<<8|2<<16,10|5<<8|2<<16,15|5<<8|2<<16,20|4<<8|1<<16];
const OWN_STOP:u32=0x0001_0f00;
const WP_STOP:u32=1<<19;
const UNUSED_STOP:u32=1<<17;
const HCI:u32=1<<11|1<<13;

#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) enum Error{Architecture,Region,State,Busy,Input(txdesc::Error),Index,Count,Report,
    Dropped{status:Status,completed_descriptors:u8},Rx(rx::Error),Io(Fault)}
impl From<Fault> for Error{fn from(v:Fault)->Self{Self::Io(v)}}
impl From<rx::Error> for Error{fn from(v:rx::Error)->Self{Self::Rx(v)}}
type Result<T>=core::result::Result<T,Error>;
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) enum State{Allocated,Installed,Running,Faulted,Stopped}
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) enum AttemptCount{Unknown,Known(u8)}
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) struct Completion{pub status:Status,pub acked:bool,pub attempts:AttemptCount,pub completed_descriptors:u8,pub raw_data_tx_count:u16}
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) enum Receive{Empty,Rpq,TxReport{final_report:bool},Other(rx::Outcome)}
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) struct Stop{pub request:Option<Fault>,pub idle:Option<Fault>}
#[derive(Clone,Copy)]
struct Pending{queue:usize,sequence:u16,descriptor:u64,report:Option<Status>,firmware:Option<txdesc::FirmwareReport>,last_counter:u8}
struct Request{meta:Frame,mac_id:u8,limit:u8,completed:u8,raw_count:u16,start:u64,budget:u32,last:u64,polls:u32,frame:[u8;txdesc::MAX_FRAME]}

pub(crate) struct Queue<'a>{
    base:NonNull<u8>,physical:u64,session:u64,state:State,issued:[u64;CHANNELS],completed:[u64;CHANNELS],
    serial:u64,request:Option<Request>,pending:Option<Pending>,last_report:Option<txdesc::Report>,last_firmware:Option<txdesc::FirmwareReport>,
    _exclusive:PhantomData<&'a mut[u8]>,
}
impl<'a> Queue<'a>{
    /// # Safety
    /// Exclusive x86-64 coherent WB bytes with UC MMIO, no aliases, below 4 GiB and disjoint from all other DMA.
    /// No old device transaction may access this region; retain it after every drop/error until PCI quiescence.
    /// Config and BAR access name the same owned RTL8852BE; this object never frees or unmaps the allocation.
    pub(crate) unsafe fn new(bus:&mut impl Bus,base:*mut u8,length:usize,physical:u64)->Result<Self>{
        if !cfg!(target_arch="x86_64"){return Err(Error::Architecture);}
        if bus.pci_command()?&7!=2{return Err(Error::State);}
        let base=NonNull::new(base).ok_or(Error::Region)?;
        if length<DMA_BYTES||length>isize::MAX as usize||base.as_ptr()as usize&4095!=0||physical==0||physical&4095!=0
            ||(base.as_ptr()as usize).checked_add(DMA_BYTES).is_none()
            ||physical.checked_add(DMA_BYTES as u64).is_none_or(|end|end>1u64<<32){return Err(Error::Region);}
        let session=io::dma_session()?;
        for at in 0..DMA_BYTES{base.as_ptr().add(at).write_volatile(0);}
        Ok(Self{base,physical,session,state:State::Allocated,issued:[0;CHANNELS],completed:[0;CHANNELS],serial:0,
            request:None,pending:None,last_report:None,last_firmware:None,_exclusive:PhantomData})
    }
    pub(crate) fn state(&self)->State{self.state}
    pub(crate) fn session(&self)->u64{self.session}
    pub(crate) fn physical(&self)->u64{self.physical}
    pub(crate) fn pending_request(&self)->bool{self.request.is_some()}
    pub(crate) fn poison(&mut self){self.state=State::Faulted;}
    pub(crate) fn installed_for_data(&mut self,bus:&mut impl Bus)->Result<()>{
        if self.state!=State::Installed||self.request.is_some()||self.pending.is_some()||self.serial!=0{return Err(Error::State)}
        if io::read32(bus,fwcmd::DMA_STOP)?&(OWN_STOP|WP_STOP)!=(OWN_STOP|WP_STOP){return Err(Error::State)}
        self.rings(bus)?;self.reclaim(bus)?;
        if self.issued!=[0;CHANNELS]||self.completed!=[0;CHANNELS]{return Err(Error::State)}Ok(())
    }
    pub(crate) fn start_data(&mut self,bus:&mut impl Bus,rings:&rx::Rings<'_>,layout:&crate::btc_session::data_mode::Layout)->Result<()>{
        if !layout.matches(self){return Err(Error::State)}self.start(bus,rings)
    }
    pub(crate) fn validate_data(&mut self,bus:&mut impl Bus)->Result<()>{
        if self.state!=State::Running{return Err(Error::State)}
        let result=(||{self.live(bus)?;self.reclaim(bus)})();self.observe(result)
    }
    fn observe<T>(&mut self,result:Result<T>)->Result<T>{if result.is_err(){self.state=State::Faulted;}result}
    fn physical_ring(&self,q:usize)->u32{(self.physical+(q*DESCRIPTORS*8)as u64)as u32}
    fn rings(&self,bus:&mut impl RegisterIo)->Result<()>{
        for q in 0..CHANNELS{
            if bus.read16(NUMBER[q])?!=DESCRIPTORS as u16||io::read32(bus,BASE[q])?!=self.physical_ring(q)
                ||io::read32(bus,BASE[q]+4)?!=0||io::read32(bus,BDRAM[q])?!=RAM[q]{return Err(Error::State);}
        }Ok(())
    }
    /// Install all five owned queues while both HCI engines and PCI mastering are disabled.
    pub(crate) fn install(&mut self,bus:&mut impl Bus)->Result<()>{
        if self.state!=State::Allocated{return Err(Error::State);}
        let result=(||{
            if bus.pci_command()?&7!=2||io::read32(bus,fwcmd::INIT_CONFIG)?&HCI!=0
                ||io::read32(bus,fwcmd::DMA_STOP)?&(OWN_STOP|WP_STOP)!=(OWN_STOP|WP_STOP){return Err(Error::State);}
            let mut timed=TimedIo::new(bus,10_000);
            io::poll32(&mut timed,fwcmd::DMA_BUSY,fwcmd::BUSY_MASK,0,10,100)?;
            io::modify32(&mut timed,fwcmd::CLEAR_INDEX,0,0x10f)?;
            for q in 0..CHANNELS{
                io::poll32(&mut timed,INDEX[q],0x0fff0fff,0,10,100)?;
                timed.write16(NUMBER[q],DESCRIPTORS as u16)?;timed.write32(BDRAM[q],RAM[q])?;
                timed.write32(BASE[q],self.physical_ring(q))?;timed.write32(BASE[q]+4,0)?;
            }
            self.rings(&mut timed)?;timed.delay_us(0)?;Ok(())
        })();
        self.observe(result)?;self.state=State::Installed;Ok(())
    }
    fn live(&self,bus:&mut impl Bus)->Result<()>{
        let command=bus.pci_command()?;
        if command==u16::MAX||command&6!=6||io::read32(bus,fwcmd::INIT_CONFIG)?&(HCI|1<<18)!=HCI
            ||io::read32(bus,fwcmd::DMA_STOP)?&(OWN_STOP|WP_STOP|3|1<<20|UNUSED_STOP)!=UNUSED_STOP
            ||io::read32(bus,0x1e0)?&0xe0!=0xe0||bus.read8(0xc660)?&0x30!=0x10{return Err(Error::State);}
        self.rings(bus)
    }
    /// This private transition has no native caller; future admission must validate actual channel and CMAC tokens.
    fn start(&mut self,bus:&mut impl Bus,rings:&rx::Rings<'_>)->Result<()>{
        if self.state!=State::Installed{return Err(Error::State);}
        let result=(||{self.live(bus)?;rings.running(bus)?;self.reclaim(bus)?;Ok(())})();
        self.observe(result)?;self.state=State::Running;Ok(())
    }
    fn time(&mut self,bus:&mut impl RegisterIo)->Result<u32>{
        self.time_at(bus.now_us())
    }
    fn time_at(&mut self,now:u64)->Result<u32>{
        let request=self.request.as_mut().ok_or(Error::State)?;
        if now<request.last{return Err(Fault::new(FaultKind::Clock,0,0).into());}request.last=now;
        let elapsed=now.checked_sub(request.start).ok_or(Error::Count)?;
        u64::from(request.budget).checked_sub(elapsed).filter(|v|*v>0).map(|v|v as u32)
            .ok_or(Fault::new(FaultKind::Timeout,0,request.completed as u32).into())
    }
    fn reclaim(&mut self,bus:&mut impl RegisterIo)->Result<()>{
        for q in 0..CHANNELS{
            let index=io::read32(bus,INDEX[q])?;let host=(index&0xfff)as usize;let hardware=((index>>16)&0xfff)as usize;
            if host!=(self.issued[q]%DESCRIPTORS as u64)as usize||hardware>=DESCRIPTORS{return Err(Error::Index);}
            let count=(hardware+DESCRIPTORS-(self.completed[q]%DESCRIPTORS as u64)as usize)%DESCRIPTORS;
            if count as u64>self.issued[q]-self.completed[q]{return Err(Error::Index);}
            self.completed[q]=self.completed[q].checked_add(count as u64).ok_or(Error::Count)?;
        }fence(Ordering::Acquire);Ok(())
    }
    /// Copy one raw MPDU; actual final firmware counts require matching RPQ and DMA completion.
    pub(crate) fn begin(&mut self,bus:&mut impl Bus,frame:&[u8],rate:u8,max_submissions:u8,mac_id:u8,timeout_us:u32)->Result<()>{
        if self.state!=State::Running{return Err(Error::State);}
        if self.request.is_some(){return Err(Error::Busy);}
        let meta=txdesc::frame(frame,rate,max_submissions).map_err(Error::Input)?;
        if mac_id>127||timeout_us==0||timeout_us>1_000_000{return Err(Error::Input(txdesc::Error::Count));}
        let now=bus.now_us();let mut request=Request{meta,mac_id,limit:if meta.group{1}else{max_submissions},completed:0,raw_count:0,start:now,budget:timeout_us,last:now,polls:0,frame:[0;txdesc::MAX_FRAME]};
        request.frame[..frame.len()].copy_from_slice(frame);self.request=Some(request);
        let result=(||{self.live(bus)?;self.reclaim(bus)?;self.submit(bus)})();self.observe(result)
    }
    fn submit(&mut self,bus:&mut impl RegisterIo)->Result<()>{
        self.time(bus)?;
        if self.pending.is_some(){return Err(Error::Busy);}
        let request=self.request.as_ref().ok_or(Error::State)?;let q=request.meta.queue;
        let next=self.serial.checked_add(1).ok_or(Error::Count)?;
        let issued=self.issued[q].checked_add(1).ok_or(Error::Count)?;
        if self.issued[q]-self.completed[q]>=DESCRIPTORS as u64-1{return Err(Error::Busy);}
        let sequence=(self.serial&0x7fff)as u16;
        let buffer=BUFFERS_AT+(self.serial%BUFFERS as u64)as usize*STRIDE;
        let wd=txdesc::descriptor(request.meta,request.mac_id,sequence,self.physical+(buffer+PAYLOAD_AT)as u64).map_err(Error::Input)?;
        let descriptor=(q*DESCRIPTORS+(self.issued[q]%DESCRIPTORS as u64)as usize)*8;
        unsafe{
            for(at,value)in wd.into_iter().enumerate(){self.base.as_ptr().add(buffer+at).write_volatile(value);}
            for at in 0..request.meta.length{
                let value=request.frame[at]|if at==1&&request.completed>0{8}else{0};
                self.base.as_ptr().add(buffer+PAYLOAD_AT+at).write_volatile(value);
            }
            self.base.as_ptr().add(descriptor).cast::<u32>().write_volatile((txdesc::WD_BYTES as u32|1<<30).to_le());
            self.base.as_ptr().add(descriptor+4).cast::<u32>().write_volatile(((self.physical+buffer as u64)as u32).to_le());
        }
        self.time(bus)?;fence(Ordering::Release);
        self.pending=Some(Pending{queue:q,sequence,descriptor:self.issued[q],report:None,firmware:None,last_counter:0});self.serial=next;self.issued[q]=issued;
        bus.write16(INDEX[q],(issued%DESCRIPTORS as u64)as u16)?;self.time(bus)?;Ok(())
    }
    fn reports(&mut self,bytes:&[u8])->Result<()>{
        if bytes.is_empty()||bytes.len()%4!=0||bytes.len()>rx::BUFFER_BYTES{return Err(Error::Report);}
        for raw in bytes.chunks_exact(4){
            let report=txdesc::rpq(raw).map_err(Error::Input)?;
            if self.last_report==Some(report){continue;}
            let pending=self.pending.as_mut().ok_or(Error::Report)?;
            let request=self.request.as_ref().ok_or(Error::Report)?;
            if report.sequence!=pending.sequence||report.qsel!=request.meta.qsel||report.mac_id!=request.mac_id{return Err(Error::Report);}
            if pending.report.is_some_and(|old|old!=report.status){return Err(Error::Report);}
            pending.report=Some(report.status);self.last_report=Some(report);
        }Ok(())
    }
    fn progress(&mut self,bus:&mut impl RegisterIo)->Result<Option<Completion>>{
        self.time(bus)?;self.reclaim(bus)?;self.time(bus)?;
        let pending=self.pending.ok_or(Error::State)?;
        if self.completed[pending.queue]<=pending.descriptor{return Ok(None);}
        let Some(status)=pending.report else{return Ok(None);};
        let request=self.request.as_mut().ok_or(Error::State)?;
        if matches!(status,Status::Lifetime|Status::MacIdDrop){return Err(Error::Dropped{status,completed_descriptors:request.completed});}
        let Some(firmware)=pending.firmware else{return Ok(None);};
        if firmware.status!=status{return Err(Error::Report);}
        if firmware.raw_data_tx_count!=txdesc::SELECTED_LIMIT{return Err(Error::Report);}
        request.raw_count=request.raw_count.checked_add(u16::from(firmware.raw_data_tx_count)).ok_or(Error::Count)?;
        request.completed=request.completed.checked_add(1).ok_or(Error::Count)?;self.pending=None;
        if status==Status::Done||request.completed==request.limit{
            let attempts=u8::try_from(request.raw_count).map_err(|_|Error::Count)?;
            if attempts==0||attempts>request.limit{return Err(Error::Count);}
            let completion=Completion{status,acked:status==Status::Done&&!request.meta.group,attempts:AttemptCount::Known(attempts),completed_descriptors:request.completed,raw_data_tx_count:request.raw_count};
            self.time(bus)?;self.request=None;return Ok(Some(completion));
        }
        self.submit(bus)?;Ok(None)
    }
    fn firmware(&mut self,report:txdesc::FirmwareReport)->Result<bool>{
        if report.raw_data_tx_count>txdesc::SELECTED_LIMIT||report.status==Status::Done&&report.raw_data_tx_count==0{return Err(Error::Report);}
        if self.last_firmware==Some(report){return Ok(true);}
        let pending=self.pending.as_mut().ok_or(Error::Report)?;
        if report.sequence!=(pending.sequence&15)as u8||report.raw_data_tx_count<pending.last_counter{return Err(Error::Report);}
        if let Some(final_report)=pending.firmware{
            if final_report==report{return Ok(true);}return Err(Error::Report);
        }
        pending.last_counter=report.raw_data_tx_count;
        let final_report=report.status==Status::Done||report.raw_data_tx_count==txdesc::SELECTED_LIMIT;
        if final_report{pending.firmware=Some(report);self.last_firmware=Some(report);}
        Ok(final_report)
    }
    fn polling_at(&mut self,now:u64)->Result<()> {
        let request=self.request.as_mut().ok_or(Error::State)?;
        if request.polls>=request.budget{return Err(Fault::new(FaultKind::Timeout,0,request.polls).into());}
        request.polls+=1;self.time_at(now)?;Ok(())
    }
    fn ingest_inner(&mut self,channel:rx::Channel,outcome:rx::Outcome,buffer:&[u8])->Result<Receive> {
        match outcome {
            rx::Outcome::Empty=>Ok(Receive::Empty),
            rx::Outcome::Dropped(_) if channel==rx::Channel::Rpq=>Err(Error::Report),
            rx::Outcome::Packet(packet) if channel==rx::Channel::Rpq=>{
                if packet.crc_error||packet.icv_error||packet.kind==crate::rxdesc::TYPE_C2H{return Err(Error::Report)}
                self.reports(packet.bytes(buffer).ok_or(Error::Report)?)?;Ok(Receive::Rpq)
            },
            rx::Outcome::Packet(packet) if packet.kind==crate::rxdesc::TYPE_C2H=>{
                let bytes=packet.bytes(buffer).ok_or(Error::Report)?;
                match txdesc::firmware_report(bytes).map_err(Error::Input)?{
                    Some(report)=>{
                        if packet.crc_error||packet.icv_error{return Err(Error::Report);}
                        Ok(Receive::TxReport{final_report:self.firmware(report)?})
                    },
                    None=>Ok(Receive::Other(outcome)),
                }
            },
            _=>Ok(Receive::Other(outcome)),
        }
    }
    /// The single RX owner supplies its copied record and a fresh clock; this never polls or releases a receive descriptor.
    /// Retain Other records for their owner. Ingesting a report alone cannot complete or submit an MPDU.
    pub(crate) fn ingest(&mut self,now:u64,channel:rx::Channel,outcome:rx::Outcome,buffer:&[u8])->Result<Receive> {
        if self.state!=State::Running||self.request.is_none(){return Err(Error::State)}
        let result=self.polling_at(now).and_then(|()|self.ingest_inner(channel,outcome,buffer));self.observe(result)
    }
    /// Revalidate actual TX registers and advance DMA/status ownership without touching either receive ring.
    pub(crate) fn advance(&mut self,bus:&mut impl Bus)->Result<Option<Completion>> {
        if self.state!=State::Running||self.request.is_none(){return Err(Error::State)}
        let result=(||{self.polling_at(bus.now_us())?;self.live(bus)?;self.progress(bus)})();self.observe(result)
    }
    // Host-model adapters own the test rings; production uses the Session dispatcher and retained records.
    #[cfg(test)]
    fn poll_receive(&mut self,bus:&mut impl Bus,rings:&mut rx::Rings<'_>,scratch:&mut[u8])->Result<Receive>{
        if self.state!=State::Running||self.request.is_none(){return Err(Error::State);}
        if scratch.len()<rx::BUFFER_BYTES{return Err(Error::Rx(rx::Error::Output));}
        let result=(||{
            self.polling_at(bus.now_us())?;self.live(bus)?;rings.running(bus)?;
            let left=self.time(bus)?;let outcome=rings.receive(bus,rx::Channel::Rxq,scratch,left)?;
            let result=self.ingest_inner(rx::Channel::Rxq,outcome,scratch)?;self.time(bus)?;Ok(result)
        })();self.observe(result)
    }
    #[cfg(test)]
    fn poll(&mut self,bus:&mut impl Bus,rings:&mut rx::Rings<'_>,scratch:&mut[u8])->Result<Option<Completion>>{
        if self.state!=State::Running||self.request.is_none(){return Err(Error::State);}
        if scratch.len()<rx::BUFFER_BYTES{return Err(Error::Rx(rx::Error::Output));}
        let result=(||{
            self.polling_at(bus.now_us())?;self.live(bus)?;rings.running(bus)?;
            let left=self.time(bus)?;let outcome=rings.receive(bus,rx::Channel::Rpq,scratch,left)?;
            self.ingest_inner(rx::Channel::Rpq,outcome,scratch)?;self.progress(bus)
        })();self.observe(result)
    }
    /// Stop only owned ordinary queues and WP DMA; never free memory or claim PCI quiescence from this result.
    pub(crate) fn stop(&mut self,bus:&mut impl RegisterIo)->Stop{
        self.state=State::Faulted;
        let request=(||{
            let mut timed=TimedIo::new(bus,1_000);io::modify32(&mut timed,fwcmd::DMA_STOP,0,OWN_STOP|WP_STOP)?;
            let value=io::read32(&mut timed,fwcmd::DMA_STOP)?;
            if value&(OWN_STOP|WP_STOP)!=(OWN_STOP|WP_STOP){return Err(Fault::new(FaultKind::PowerState,fwcmd::DMA_STOP,value));}
            timed.delay_us(0)
        })().err();
        let idle=(||{let mut timed=TimedIo::new(bus,1_000);io::poll32(&mut timed,fwcmd::DMA_BUSY,OWN_STOP|WP_STOP,0,10,100)?;timed.delay_us(0)})().err();
        if request.is_none()&&idle.is_none(){self.state=State::Stopped;}Stop{request,idle}
    }
}

/// Fixed read-only identity checks for the sealed data-mode owner; current descriptor counters remain with Queue.
pub(crate) fn check_layout(bus:&mut impl RegisterIo,physical:u64)->io::Result<()>{
    for q in 0..CHANNELS{
        let wanted=(physical+(q*DESCRIPTORS*8)as u64)as u32;
        for(at,value)in[(BASE[q],wanted),(BASE[q]+4,0),(BDRAM[q],RAM[q])]{
            let actual=io::read32(bus,at)?;if actual!=value{return Err(Fault::new(FaultKind::PowerState,at,actual))}
        }
        let number=bus.read16(NUMBER[q])?;if number!=DESCRIPTORS as u16{return Err(Fault::new(FaultKind::PowerState,NUMBER[q],number as u32))}
        let index=io::read32(bus,INDEX[q])?;
        if index&0xfff>=DESCRIPTORS as u32||(index>>16)&0xfff>=DESCRIPTORS as u32{return Err(Fault::new(FaultKind::PowerState,INDEX[q],index))}
    }Ok(())
}

#[cfg(test)]
#[path="../../tests/rtw89_tx_cases.rs"]mod tests;
