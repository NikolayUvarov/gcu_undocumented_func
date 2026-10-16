#![allow(dead_code)]
#[path="../rtw89_wifi/src/io.rs"]mod io;
#[path="../rtw89_wifi/src/fwcmd.rs"]mod fwcmd;
#[path="../rtw89_wifi/src/rx.rs"]mod rx;
#[path="../rtw89_wifi/src/rxdesc.rs"]mod rxdesc;
#[path="../rtw89_wifi/src/c2h.rs"]mod c2h;
use io::{Fault,FaultKind,RegisterIo};
use rx::{Rings,Channel,State,Error,Outcome};
use std::alloc::{alloc_zeroed,dealloc,Layout};

struct Region{base:*mut u8,layout:Layout}
impl Region{
    fn new()->Self{let layout=Layout::from_size_align(rx::DMA_BYTES,4096).unwrap();let base=unsafe{alloc_zeroed(layout)};assert!(!base.is_null());Self{base,layout}}
    fn rings<'a>(&'a mut self,d:&mut Device)->Rings<'a>{d.base=self.base;unsafe{Rings::new(d,self.base,self.layout.size(),0x200000).unwrap()}}
}
impl Drop for Region{fn drop(&mut self){unsafe{dealloc(self.base,self.layout)}}}
struct Device{regs:Vec<u8>,command:u16,now:u64,writes:Vec<(usize,usize,u32)>,base:*mut u8,
    replace_on_release:bool,fail_write:Option<usize>,ignored_write:Option<usize>,jump_release:bool,jump_read:bool,time_step:u64}
impl Device{
    fn new()->Self{let mut d=Self{regs:vec![0;0x2000],command:2,now:0,writes:Vec::new(),base:std::ptr::null_mut(),
        replace_on_release:false,fail_write:None,ignored_write:None,jump_release:false,jump_read:false,time_step:0};d.set(fwcmd::DMA_STOP,0x1f0f03);d}
    fn get(&self,at:usize)->u32{u32::from_le_bytes(self.regs[at..at+4].try_into().unwrap())}
    fn set(&mut self,at:usize,value:u32){self.regs[at..at+4].copy_from_slice(&value.to_le_bytes());}
    fn apply(&mut self,at:usize,width:usize,value:u32)->io::Result<()>{
        io::check_offset(at,width,self.regs.len())?;self.writes.push((at,width,value));
        if self.ignored_write!=Some(self.writes.len()){
            self.regs[at..at+width].copy_from_slice(&value.to_le_bytes()[..width]);
            if at==0x1018{self.set(0x1050,0);self.set(0x1054,0);self.set(at,0);}
            if at==0x1050||at==0x1054{
                assert_eq!(width,2);
                if self.replace_on_release{let ch=(at-0x1050)/4;let previous=(value as usize+255)%256;
                    let start=rx::BUFFERS_AT+(ch*256+previous)*rx::BUFFER_STRIDE;
                    unsafe{for at in start..start+rx::BUFFER_BYTES{self.base.add(at).write_volatile(0xa5);}}
                }
                if self.jump_release{self.now+=1_000_000;}
            }
        }
        if self.fail_write==Some(self.writes.len()){return Err(Fault::new(FaultKind::Access,at,value));}Ok(())
    }
    fn enable(&mut self){self.command=6;self.set(fwcmd::INIT_CONFIG,(1<<11)|(1<<13));self.set(fwcmd::DMA_STOP,0xb0f00);}
    fn put_packet(&mut self,ch:usize,index:usize,data:&[u8]){
        assert!(data.len()<=rx::BUFFER_BYTES);let at=rx::BUFFERS_AT+(ch*256+index)*rx::BUFFER_STRIDE;
        unsafe{for(i,v)in data.iter().copied().enumerate(){self.base.add(at+i).write_volatile(v);}}
        let host=self.get(0x1050+ch*4)&0xfff;self.set(0x1050+ch*4,host|(((index+1)%256)as u32)<<16);
    }
}
impl RegisterIo for Device{
    fn read8(&mut self,at:usize)->io::Result<u8>{Ok(self.regs[at])}
    fn read16(&mut self,at:usize)->io::Result<u16>{Ok(u16::from_le_bytes(self.regs[at..at+2].try_into().unwrap()))}
    fn read32(&mut self,at:usize)->io::Result<u32>{if self.jump_read&&(at==0x1050||at==0x1054){self.now+=1_000_000;}Ok(self.get(at))}
    fn write8(&mut self,at:usize,v:u8)->io::Result<()>{self.apply(at,1,v as u32)}
    fn write16(&mut self,at:usize,v:u16)->io::Result<()>{self.apply(at,2,v as u32)}
    fn write32(&mut self,at:usize,v:u32)->io::Result<()>{self.apply(at,4,v)}
    fn now_us(&mut self)->u64{self.now+=self.time_step;self.now}
    fn delay_us(&mut self,us:u32)->io::Result<()>{self.now+=u64::from(us);Ok(())}
}
impl fwcmd::Bus for Device{fn pci_command(&mut self)->io::Result<u16>{Ok(self.command)}}
fn active<'a>(r:&'a mut Region,d:&mut Device)->Rings<'a>{let mut q=r.rings(d);q.install(d).unwrap();d.enable();unsafe{q.start(d).unwrap()};d.writes.clear();q}
fn packet(payload:&[u8],long:bool,kind:u8)->Vec<u8>{
    let offset=if long{36}else{20};let mut data=vec![0;offset+payload.len()];let length=data.len()as u32;
    data[0..4].copy_from_slice(&(length|0xc000|0x1fff0000).to_le_bytes());
    data[4..8].copy_from_slice(&((payload.len()as u32)|((kind as u32)<<24)|if long{1<<31}else{0}).to_le_bytes());
    data[offset..].copy_from_slice(payload);data
}
fn ack(done:bool,id:c2h::Identity,status:u8)->Vec<u8>{let mut b=vec![0;12];b[0..4].copy_from_slice(&(1u32|if done{1<<8}else{0}).to_le_bytes());b[4..8].copy_from_slice(&12u32.to_le_bytes());
    let seq=if done{(id.sequence as u32)<<24|(status as u32)<<16}else{(id.sequence as u32)<<16};
    b[8..12].copy_from_slice(&((id.category as u32)|((id.class as u32)<<2)|((id.function as u32)<<8)|seq).to_le_bytes());b}
fn stop(q:&mut Rings<'_>,d:&mut Device){let s=q.stop(d);assert_eq!(s,rx::Stop{request:None,hci:None,idle:None});}

#[test]
fn region_bounds_are_checked_before_volatile_initialization(){
    let mut d=Device::new();let r=Region::new();
    for (base,len,physical)in[(std::ptr::null_mut(),rx::DMA_BYTES,0x200000),(r.base,rx::DMA_BYTES-1,0x200000),(r.base,rx::DMA_BYTES,0xfffff000),(r.base,rx::DMA_BYTES,0),(unsafe{r.base.add(1)},rx::DMA_BYTES,0x200000)]{
        assert!(unsafe{Rings::new(&mut d,base,len,physical)}.is_err());assert!(d.writes.is_empty());
    }
    d.command=6;assert!(unsafe{Rings::new(&mut d,r.base,rx::DMA_BYTES,0x200000)}.is_err());
}
#[test]
fn exact_upstream_descriptors_fit_disjoint_future_pool(){
    assert!(fwcmd::DMA_BYTES+rx::DMA_BYTES<=rx::FUTURE_GRANT_BYTES);
    let mut d=Device::new();let mut r=Region::new();let q=r.rings(&mut d);
    for index in 0..512{let at=index*8;let size=unsafe{u32::from_le(d.base.add(at).cast::<u32>().read_volatile())};let addr=unsafe{u32::from_le(d.base.add(at+4).cast::<u32>().read_volatile())};
        assert_eq!(size,11498);assert_eq!(addr,0x200000+(4096+index*11520)as u32);}
    assert_eq!(q.state(),State::Allocated);assert!(d.writes.is_empty());
}
#[test]
fn installation_changes_only_owned_ring_registers_without_activation(){
    let mut d=Device::new();let mut r=Region::new();let mut q=r.rings(&mut d);q.install(&mut d).unwrap();
    assert_eq!(q.state(),State::Installed);assert_eq!(d.command,2);assert_eq!(d.get(fwcmd::DMA_STOP),0x1f0f03);
    assert_eq!(d.read16(0x1020).unwrap(),256);assert_eq!(d.read16(0x1022).unwrap(),256);
    assert_eq!(d.get(0x1100),0x200000);assert_eq!(d.get(0x1108),0x200800);
    assert!(d.writes.iter().all(|(at,_,_)|[0x1018,0x1020,0x1022,0x1100,0x1104,0x1108,0x110c].contains(at)));
}
#[test]
fn install_refuses_active_or_busy_receive_before_programming_rings(){
    for variant in 0..4{let mut d=Device::new();let mut r=Region::new();let mut q=r.rings(&mut d);
        match variant{0=>d.command=6,1=>d.set(fwcmd::INIT_CONFIG,1<<13),2=>d.set(fwcmd::DMA_STOP,0),_=>d.set(fwcmd::DMA_BUSY,1<<22)}
        assert!(q.install(&mut d).is_err());assert!(d.writes.is_empty());assert_eq!(q.state(),State::Faulted);
    }
}
#[test]
fn ignored_or_failed_installation_never_admits_start(){
    for failure in [false,true]{let mut d=Device::new();let mut r=Region::new();let mut q=r.rings(&mut d);
        if failure{d.fail_write=Some(3);}else{d.ignored_write=Some(3);}assert!(q.install(&mut d).is_err());d.enable();
        assert_eq!(unsafe{q.start(&mut d)},Err(Error::State));assert_eq!(q.state(),State::Faulted);
    }
}
#[test]
fn start_requires_installed_unchanged_packet_mode_rings_and_never_writes(){
    for variant in 0..5{let mut d=Device::new();let mut r=Region::new();let mut q=r.rings(&mut d);q.install(&mut d).unwrap();d.enable();d.writes.clear();
        match variant{0=>d.command=0xffff,1=>d.set(0x1100,0),2=>d.set(fwcmd::INIT_CONFIG,1<<13|1<<18),3=>d.set(0x1050,256<<16),_=>d.set(fwcmd::DMA_STOP,1)}
        assert!(unsafe{q.start(&mut d)}.is_err());assert!(d.writes.is_empty());
    }
}
#[test]
fn copied_c2h_survives_immediate_hardware_reuse_after_release(){
    let id=c2h::Identity{category:1,class:9,function:5,sequence:254};let body=ack(true,id,0);
    let mut d=Device::new();let mut r=Region::new();let mut q=active(&mut r,&mut d);d.replace_on_release=true;d.put_packet(0,0,&packet(&body,false,10));
    let mut out=vec![0;rx::BUFFER_BYTES];let Outcome::Packet(p)=q.receive(&mut d,Channel::Rxq,&mut out,1000).unwrap()else{panic!()};
    assert_eq!(p.bytes(&out).unwrap(),body);let a=c2h::parse(p.bytes(&out).unwrap()).unwrap().ack().unwrap().unwrap();assert!(a.matches(id));
    assert_eq!(a,c2h::Ack::Done{command:id,status:0});assert_eq!(d.writes,[(0x1050,2,1)]);stop(&mut q,&mut d);
}
#[test]
fn ring_wrap_and_rpq_are_independent_without_tag_checks(){
    let mut d=Device::new();let mut r=Region::new();let mut q=active(&mut r,&mut d);let mut out=vec![0;rx::BUFFER_BYTES];
    for n in 0..260{let data=packet(&[n as u8],false,0);d.put_packet(0,n%256,&data);assert!(matches!(q.receive(&mut d,Channel::Rxq,&mut out,1000),Ok(Outcome::Packet(_))));}
    assert_eq!(d.get(0x1050)&0xfff,4);assert_eq!(d.get(0x1054),0);
    let report=0x81234567u32.to_le_bytes();d.put_packet(1,0,&packet(&report,true,0));let Outcome::Packet(p)=q.receive(&mut d,Channel::Rpq,&mut out,1000).unwrap()else{panic!()};
    assert_eq!(rxdesc::report(p.bytes(&out).unwrap()).unwrap().sequence,0x123);assert_eq!(d.get(0x1054)&0xfff,1);stop(&mut q,&mut d);
}
#[test]
fn malformed_length_and_fragments_drop_once_then_next_valid_packet_recovers(){
    let mut d=Device::new();let mut r=Region::new();let mut q=active(&mut r,&mut d);let mut out=vec![0;rx::BUFFER_BYTES];
    let mut bad=packet(&[1,2,3],false,0);bad[0..4].copy_from_slice(&0x3fffu32.to_le_bytes());d.put_packet(0,0,&bad);
    assert_eq!(q.receive(&mut d,Channel::Rxq,&mut out,1000),Ok(Outcome::Dropped(rxdesc::Error::Length)));
    bad=packet(&[1,2,3],false,0);bad[1]&=!0x80;d.put_packet(0,1,&bad);assert_eq!(q.receive(&mut d,Channel::Rxq,&mut out,1000),Ok(Outcome::Dropped(rxdesc::Error::Fragment)));
    d.put_packet(0,2,&packet(&[9],false,0));assert!(matches!(q.receive(&mut d,Channel::Rxq,&mut out,1000),Ok(Outcome::Packet(_))));stop(&mut q,&mut d);
}
#[test]
fn bad_indices_poison_without_dereferencing_or_releasing_dma(){
    for value in [1,256<<16,0xfff<<16,u32::MAX]{let mut d=Device::new();let mut r=Region::new();let mut q=active(&mut r,&mut d);d.set(0x1050,value);
        let mut out=vec![0;rx::BUFFER_BYTES];assert!(q.receive(&mut d,Channel::Rxq,&mut out,1000).is_err());assert!(d.writes.is_empty());assert_eq!(q.state(),State::Faulted);stop(&mut q,&mut d);}
}
#[test]
fn insufficient_output_and_invalid_budget_leave_descriptor_owned(){
    let mut d=Device::new();let mut r=Region::new();let mut q=active(&mut r,&mut d);d.put_packet(0,0,&packet(&[1],false,0));
    assert_eq!(q.receive(&mut d,Channel::Rxq,&mut[0;10],1000),Err(Error::Output));let mut out=vec![0;rx::BUFFER_BYTES];
    assert_eq!(q.receive(&mut d,Channel::Rxq,&mut out,0),Err(Error::Budget));assert!(d.writes.is_empty());assert_eq!(q.state(),State::Running);stop(&mut q,&mut d);
}
#[test]
fn applied_doorbell_error_poison_prevents_second_release(){
    let mut d=Device::new();let mut r=Region::new();let mut q=active(&mut r,&mut d);d.put_packet(0,0,&packet(&[1],false,0));d.fail_write=Some(1);let mut out=vec![0;rx::BUFFER_BYTES];
    assert!(q.receive(&mut d,Channel::Rxq,&mut out,1000).is_err());assert_eq!(d.get(0x1050)&0xfff,1);assert_eq!(q.state(),State::Faulted);
    assert_eq!(q.receive(&mut d,Channel::Rxq,&mut out,1000),Err(Error::State));assert_eq!(d.writes.len(),1);stop(&mut q,&mut d);
}
#[test]
fn timeout_during_copy_does_not_release_descriptor(){
    let mut d=Device::new();let mut r=Region::new();let mut q=active(&mut r,&mut d);d.put_packet(0,0,&packet(&vec![7;1000],false,0));d.time_step=1;let mut out=vec![0;rx::BUFFER_BYTES];
    assert!(matches!(q.receive(&mut d,Channel::Rxq,&mut out,10),Err(Error::Io(Fault{kind:FaultKind::Timeout,..}))));assert!(d.writes.is_empty());d.time_step=0;stop(&mut q,&mut d);
}
#[test]
fn late_empty_read_and_last_doorbell_cannot_report_success(){
    for empty in [false,true]{let mut d=Device::new();let mut r=Region::new();let mut q=active(&mut r,&mut d);if empty{d.jump_read=true;}else{d.put_packet(0,0,&packet(&[1],false,0));d.jump_release=true;}
        let mut out=vec![0;rx::BUFFER_BYTES];assert!(matches!(q.receive(&mut d,Channel::Rxq,&mut out,1000),Err(Error::Io(Fault{kind:FaultKind::Timeout,..}))));assert_eq!(q.state(),State::Faulted);stop(&mut q,&mut d);}
}
#[test]
fn stop_attempts_hci_even_when_stop_write_errors_and_preserves_command_tx(){
    let mut d=Device::new();let mut r=Region::new();let mut q=active(&mut r,&mut d);d.fail_write=Some(1);let result=q.stop(&mut d);
    assert!(result.request.is_some());assert!(result.hci.is_none()&&result.idle.is_none());assert_eq!(d.get(fwcmd::INIT_CONFIG),1<<11);assert_eq!(d.get(fwcmd::DMA_STOP),0xb0f03);assert_eq!(q.state(),State::Faulted);
}
#[test]
fn descriptor_shift_long_header_and_driver_info_are_bounds_checked(){
    let mut p=packet(&[0;80],true,10);let w0=5u32|3<<14|2<<28|10<<24|1<<31;p[4..8].copy_from_slice(&w0.to_le_bytes());
    let decoded=rxdesc::parse(&p).unwrap();assert_eq!(decoded.offset,58);assert_eq!(decoded.length,5);
    p[4..8].copy_from_slice(&(w0|0x3fff).to_le_bytes());assert_eq!(rxdesc::parse(&p),Err(rxdesc::Error::Descriptor));
    for end in 0..20{assert!(rxdesc::parse(&p[..end]).is_err());}
}
#[test]
fn complete_c2h_length_and_ack_identity_are_not_inferred_from_dma_completion(){
    let id=c2h::Identity{category:2,class:8,function:1,sequence:255};
    for done in [false,true]{let b=ack(done,id,7);let event=c2h::parse(&b).unwrap();let value=event.ack().unwrap().unwrap();assert!(value.matches(id));
        for wrong in [c2h::Identity{sequence:0,..id},c2h::Identity{category:1,..id},c2h::Identity{class:9,..id},c2h::Identity{function:2,..id}]{assert!(!value.matches(wrong));}
        if done{assert_eq!(value,c2h::Ack::Done{command:id,status:7});}else{assert_eq!(value,c2h::Ack::Received(id));}
        for end in 0..12{assert!(c2h::parse(&b[..end]).is_err());}
        let mut extra=b.clone();extra.push(0);assert_eq!(c2h::parse(&extra),Err(c2h::Error::Length));
    }
    let mut b=ack(false,id,0);b[0]=2;assert!(c2h::parse(&b).unwrap().ack().unwrap().is_none());
}
#[test]
fn one_call_never_drains_more_than_one_completed_descriptor(){
    let mut d=Device::new();let mut r=Region::new();let mut q=active(&mut r,&mut d);let mut out=vec![0;rx::BUFFER_BYTES];
    d.put_packet(0,0,&packet(&[11],false,0));d.put_packet(0,1,&packet(&[22],false,0));
    let Outcome::Packet(first)=q.receive(&mut d,Channel::Rxq,&mut out,1000).unwrap()else{panic!()};
    assert_eq!(first.bytes(&out).unwrap(),[11]);assert_eq!(d.writes,[(0x1050,2,1)]);
    let Outcome::Packet(second)=q.receive(&mut d,Channel::Rxq,&mut out,1000).unwrap()else{panic!()};
    assert_eq!(second.bytes(&out).unwrap(),[22]);assert_eq!(d.writes.len(),2);
    assert_eq!(q.receive(&mut d,Channel::Rxq,&mut out,1000),Ok(Outcome::Empty));assert_eq!(d.writes.len(),2);stop(&mut q,&mut d);
}

#[test]
fn validated_copy_survives_release_failure_but_never_claims_successful_ownership(){
    let mut d=Device::new();let mut r=Region::new();let mut q=active(&mut r,&mut d);
    d.put_packet(0,0,&packet(&[0x63;64],false,0));d.jump_release=true;d.replace_on_release=true;
    let mut out=vec![0;rx::BUFFER_BYTES];assert!(q.receive(&mut d,Channel::Rxq,&mut out,1000).is_err());
    assert_eq!(q.state(),State::Faulted);assert_eq!(q.copied_packet().unwrap().bytes(&out).unwrap(),[0x63;64]);
    assert_eq!(q.receive(&mut d,Channel::Rxq,&mut out,1000),Err(Error::State));assert!(q.copied_packet().is_none());stop(&mut q,&mut d);
}
#[test]
fn partial_copy_or_empty_receive_cannot_reuse_previous_validated_metadata(){
    let mut d=Device::new();let mut r=Region::new();let mut q=active(&mut r,&mut d);let mut out=vec![0;rx::BUFFER_BYTES];
    d.put_packet(0,0,&packet(&[0x42;80],false,0));q.receive(&mut d,Channel::Rxq,&mut out,1000).unwrap();assert!(q.copied_packet().is_some());
    assert_eq!(q.receive(&mut d,Channel::Rxq,&mut out,1000).unwrap(),Outcome::Empty);assert!(q.copied_packet().is_none());
    d.put_packet(0,1,&packet(&[0x53;80],false,0));d.time_step=2;
    assert!(q.receive(&mut d,Channel::Rxq,&mut out,5).is_err());assert!(q.copied_packet().is_none());d.time_step=0;stop(&mut q,&mut d);
}
