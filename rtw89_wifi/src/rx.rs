// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022, 2020-2022 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! RTL8852BE RXQ/RPQ ownership from pinned pci.c/pci.h; no packet allocation, interrupts or automatic activation.

use core::{marker::PhantomData,ptr::NonNull,sync::atomic::{fence,Ordering}};
use crate::{fwcmd::{self,Bus},io::{self,Fault,FaultKind,RegisterIo,TimedIo},rxdesc};

pub const DESCRIPTORS:usize=256;
pub const DESCRIPTOR_BYTES:usize=8;
pub const BUFFER_BYTES:usize=rxdesc::BUFFER_BYTES;
pub const BUFFER_STRIDE:usize=(BUFFER_BYTES+63)&!63;
pub const BUFFERS_AT:usize=4096;
pub const DMA_BYTES:usize=BUFFERS_AT+2*DESCRIPTORS*BUFFER_STRIDE;
pub const FUTURE_GRANT_BYTES:usize=8*1024*1024;
const RX_ENABLE:u32=1<<13;
const RX_BUSY:u32=1<<22|3;
const RX_CLEAR:usize=0x1018;
const INDEX:[usize;2]=[0x1050,0x1054];
const NUMBER:[usize;2]=[0x1020,0x1022];
const BASE:[usize;2]=[0x1100,0x1108];

#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub enum Channel{Rxq,Rpq}
impl Channel{fn index(self)->usize{match self{Self::Rxq=>0,Self::Rpq=>1}}}
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub enum State{Allocated,Installed,Running,Faulted,Stopped}
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub enum Error{Architecture,Region,State,Index,Output,Budget,Io(Fault)}
impl From<Fault> for Error{fn from(value:Fault)->Self{Self::Io(value)}}
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub enum Outcome{Empty,Packet(rxdesc::Packet),Dropped(rxdesc::Error)}
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub struct Stop{pub request:Option<Fault>,pub hci:Option<Fault>,pub idle:Option<Fault>}

pub struct Rings<'a>{base:NonNull<u8>,physical:u64,host:[u16;2],state:State,_exclusive:PhantomData<&'a mut[u8]>}
impl<'a> Rings<'a>{
    /// # Safety
    /// The complete used region is exclusive coherent WB DMA on x86-64, mapped at `physical`, with UC BAR access.
    /// It is disjoint from the command queue and all CPU aliases; no earlier device transaction may access it.
    /// Keep allocation/mapping alive after every error or drop until external PCI quiescence is verified.
    /// The PCI configuration and MMIO adapter must name the same exclusively owned RTL8852BE function.
    pub unsafe fn new(bus:&mut impl Bus,base:*mut u8,length:usize,physical:u64)->Result<Self,Error>{
        if !cfg!(target_arch="x86_64"){return Err(Error::Architecture);}
        if bus.pci_command()?&7!=2{return Err(Error::State);}
        let base=NonNull::new(base).ok_or(Error::Region)?;
        if length<DMA_BYTES||length>isize::MAX as usize||base.as_ptr()as usize&4095!=0||physical==0||physical&4095!=0
            ||(base.as_ptr()as usize).checked_add(DMA_BYTES).is_none()||physical.checked_add(DMA_BYTES as u64).is_none_or(|end|end>1u64<<32){return Err(Error::Region);}
        for at in 0..DMA_BYTES{base.as_ptr().add(at).write_volatile(0);}
        for channel in 0..2{for index in 0..DESCRIPTORS{
            let descriptor=(channel*DESCRIPTORS+index)*DESCRIPTOR_BYTES;
            let buffer=BUFFERS_AT+(channel*DESCRIPTORS+index)*BUFFER_STRIDE;
            base.as_ptr().add(descriptor).cast::<u32>().write_volatile((BUFFER_BYTES as u32).to_le());
            base.as_ptr().add(descriptor+4).cast::<u32>().write_volatile(((physical+buffer as u64)as u32).to_le());
        }}
        Ok(Self{base,physical,host:[0;2],state:State::Allocated,_exclusive:PhantomData})
    }
    pub fn state(&self)->State{self.state}
    pub fn physical(&self)->u64{self.physical}
    fn poison<T>(&mut self,result:Result<T,Error>)->Result<T,Error>{if result.is_err(){self.state=State::Faulted;}result}
    fn check_rings(&self,bus:&mut impl RegisterIo,zero:bool)->Result<(),Error>{
        for channel in 0..2{
            let physical=self.physical+(channel*DESCRIPTORS*DESCRIPTOR_BYTES)as u64;
            if bus.read16(NUMBER[channel])?!=DESCRIPTORS as u16||io::read32(bus,BASE[channel])?!=physical as u32||io::read32(bus,BASE[channel]+4)?!=0{return Err(Error::State);}
            let index=io::read32(bus,INDEX[channel])?;
            let host=index&0xfff;let hardware=(index>>16)&0xfff;
            if host!=self.host[channel]as u32||hardware>=DESCRIPTORS as u32||zero&&hardware!=0{return Err(Error::Index);}
        }
        Ok(())
    }
    /// Install both owned rings before mastering or RXHCI activation; no other channel's registers are changed.
    pub fn install(&mut self,bus:&mut impl Bus)->Result<(),Error>{
        if self.state!=State::Allocated{return Err(Error::State);}
        let result=(||{
            if bus.pci_command()?&7!=2||io::read32(bus,fwcmd::INIT_CONFIG)?&RX_ENABLE!=0||io::read32(bus,fwcmd::DMA_STOP)?&3!=3{return Err(Error::State);}
            let mut timed=TimedIo::new(bus,10_000);
            io::poll32(&mut timed,fwcmd::DMA_BUSY,RX_BUSY,0,10,100)?;
            io::modify32(&mut timed,RX_CLEAR,0,3)?;
            for channel in 0..2{
                io::poll32(&mut timed,INDEX[channel],0x0fff_0fff,0,10,100)?;
                let physical=self.physical+(channel*DESCRIPTORS*DESCRIPTOR_BYTES)as u64;
                timed.write16(NUMBER[channel],DESCRIPTORS as u16)?;
                timed.write32(BASE[channel],physical as u32)?;timed.write32(BASE[channel]+4,0)?;
            }
            self.check_rings(&mut timed,true)?;timed.delay_us(0)?;Ok(())
        })();
        self.poison(result)?;self.state=State::Installed;Ok(())
    }
    /// # Safety
    /// Caller completed supported MAC/PCIe setup and enabled only channels whose rings were installed first.
    /// RXQ/RPQ use these unchanged descriptors; PHY/regulatory readiness and ordinary TX remain separate.
    /// RX payloads are untrusted. Start never enables bus mastering, RXHCI or any DMA channel itself.
    pub unsafe fn start(&mut self,bus:&mut impl Bus)->Result<(),Error>{
        if self.state!=State::Installed{return Err(Error::State);}
        let result=(||{
            let command=bus.pci_command()?;
            if command==u16::MAX||command&6!=6||io::read32(bus,fwcmd::INIT_CONFIG)?&(RX_ENABLE|1<<18)!=RX_ENABLE
                ||io::read32(bus,fwcmd::DMA_STOP)?&(3|1<<20)!=0{return Err(Error::State);}
            self.check_rings(bus,false)?;Ok(())
        })();
        self.poison(result)?;self.state=State::Running;Ok(())
    }
    /// Process at most one descriptor under the caller's budget; returned packet offsets refer to `output` only.
    /// Malformed or fragmented frames are explicitly dropped. No reference into DMA survives the host-index release.
    pub fn receive(&mut self,bus:&mut impl RegisterIo,channel:Channel,output:&mut[u8],budget_us:u32)->Result<Outcome,Error>{
        if self.state!=State::Running{return Err(Error::State);}
        if budget_us==0||budget_us>1_000_000{return Err(Error::Budget);}
        if output.len()<BUFFER_BYTES{return Err(Error::Output);}
        let channel=channel.index();
        let result=(||{
            let mut timed=TimedIo::new(bus,u64::from(budget_us));
            let index=io::read32(&mut timed,INDEX[channel])?;
            let hardware=((index>>16)&0xfff)as usize;
            if index&0xfff!=u32::from(self.host[channel])||hardware>=DESCRIPTORS{return Err(Error::Index);}
            if hardware==self.host[channel]as usize{timed.delay_us(0)?;return Ok(Outcome::Empty);}
            fence(Ordering::Acquire);
            let at=BUFFERS_AT+(channel*DESCRIPTORS+self.host[channel]as usize)*BUFFER_STRIDE;
            let info=unsafe{u32::from_le(self.base.as_ptr().add(at).cast::<u32>().read_volatile())};
            let length=(info&0x3fff)as usize;
            let outcome=if !(20..=BUFFER_BYTES).contains(&length){Outcome::Dropped(rxdesc::Error::Length)}else{
                for start in (0..length).step_by(64){
                    timed.delay_us(0)?;
                    for i in start..length.min(start+64){output[i]=unsafe{self.base.as_ptr().add(at+i).read_volatile()};}
                }
                match rxdesc::parse(&output[..length]){Ok(packet)=>Outcome::Packet(packet),Err(error)=>Outcome::Dropped(error)}
            };
            timed.delay_us(0)?;
            unsafe{self.base.as_ptr().add(at).cast::<u32>().write_volatile(0);}
            fence(Ordering::Release);
            self.host[channel]=(self.host[channel]+1)%DESCRIPTORS as u16;
            timed.write16(INDEX[channel],self.host[channel])?;
            timed.delay_us(0)?;Ok(outcome)
        })();
        self.poison(result)
    }
    /// A bounded local stop preserves TX command operation; external PCI quiescence still controls DMA release.
    pub fn stop(&mut self,bus:&mut impl RegisterIo)->Stop{
        self.state=State::Faulted;
        let mut timed=TimedIo::new(bus,1000);
        let request=(||{
            io::modify32(&mut timed,fwcmd::DMA_STOP,0,3)?;
            let value=io::read32(&mut timed,fwcmd::DMA_STOP)?;
            if value&3!=3{return Err(Fault::new(FaultKind::PowerState,fwcmd::DMA_STOP,value));}
            timed.delay_us(0)
        })().err();
        let hci=(||{
            io::modify32(&mut timed,fwcmd::INIT_CONFIG,RX_ENABLE,0)?;
            let value=io::read32(&mut timed,fwcmd::INIT_CONFIG)?;
            if value&RX_ENABLE!=0{return Err(Fault::new(FaultKind::PowerState,fwcmd::INIT_CONFIG,value));}
            timed.delay_us(0)
        })().err();
        let idle=(||{io::poll32(&mut timed,fwcmd::DMA_BUSY,RX_BUSY,0,10,100)?;timed.delay_us(0)})().err();
        if request.is_none()&&hci.is_none()&&idle.is_none(){self.state=State::Stopped;}
        Stop{request,hci,idle}
    }
}
