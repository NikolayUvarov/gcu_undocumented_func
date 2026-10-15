use super::*;
use std::alloc::{alloc_zeroed,dealloc,Layout};

struct Region{base:*mut u8,len:usize}
impl Region{
    fn new(len:usize)->Self{let base=unsafe{alloc_zeroed(Layout::from_size_align(len,4096).unwrap())};assert!(!base.is_null());Self{base,len}}
    fn bytes(&self,at:usize,len:usize)->&[u8]{assert!(at.checked_add(len).is_some_and(|end|end<=self.len));unsafe{std::slice::from_raw_parts(self.base.add(at),len)}}
}
impl Drop for Region{fn drop(&mut self){unsafe{dealloc(self.base,Layout::from_size_align(self.len,4096).unwrap())}}}
#[derive(Clone)]
struct Published{q:usize,seq:u16,mac:u8,qsel:u8,wd:Vec<u8>,frame:Vec<u8>}
struct Device{
    mmio:Vec<u8>,command:u16,now:u64,frozen:bool,tx:*mut u8,rx:*mut u8,rx_written:u16,
    published:Vec<Published>,statuses:Vec<Status>,auto_dma:bool,auto_rpq:bool,
    fail_write:Option<usize>,ignore_stop:bool,late_read:Option<usize>,writes:Vec<(usize,u32)>,
}
impl Device{
    fn new(tx:&Region,rx:&Region)->Self{
        let mut d=Self{mmio:vec![0;0x20000],command:2,now:1,frozen:false,tx:tx.base,rx:rx.base,rx_written:0,
            published:Vec::new(),statuses:Vec::new(),auto_dma:true,auto_rpq:true,fail_write:None,ignore_stop:false,late_read:None,writes:Vec::new()};
        d.put(fwcmd::DMA_STOP,4,0x001f0f03);d.put(0x1e0,4,0xe0);d
    }
    fn get(&self,at:usize,width:usize)->u32{let mut raw=[0;4];raw[..width].copy_from_slice(&self.mmio[at..at+width]);u32::from_le_bytes(raw)}
    fn put(&mut self,at:usize,width:usize,value:u32){self.mmio[at..at+width].copy_from_slice(&value.to_le_bytes()[..width]);}
    fn tick(&mut self){if !self.frozen{self.now+=1;}}
    fn dma(&self,physical:u32,len:usize)->&[u8]{
        let at=(physical as usize).checked_sub(0x200000).unwrap();assert!(at.checked_add(len).is_some_and(|end|end<=DMA_BYTES));
        unsafe{std::slice::from_raw_parts(self.tx.add(at),len)}
    }
    fn word(bytes:&[u8],at:usize)->u32{u32::from_le_bytes(bytes[at..at+4].try_into().unwrap())}
    fn inject(&mut self,report:u32){
        let host=self.get(0x1054,4)&0xfff;let next=(self.rx_written+1)%256;assert_ne!(u32::from(next),host);
        let descriptor=(256+usize::from(self.rx_written))*8;
        let raw=unsafe{std::slice::from_raw_parts(self.rx.add(descriptor),8)};
        assert_eq!(Self::word(raw,0),rx::BUFFER_BYTES as u32);
        let at=(Self::word(raw,4)as usize).checked_sub(0x400000).unwrap();assert!(at.checked_add(24).is_some_and(|end|end<=rx::DMA_BYTES));
        let bytes=unsafe{std::slice::from_raw_parts_mut(self.rx.add(at),24)};bytes.fill(0);
        bytes[..4].copy_from_slice(&(24u32|0xc000).to_le_bytes());bytes[4..8].copy_from_slice(&(4u32|7<<24).to_le_bytes());
        bytes[20..24].copy_from_slice(&report.to_le_bytes());self.rx_written=next;self.put(0x1054,4,host|u32::from(next)<<16);
    }
    fn report(p:&Published,status:Status)->u32{(u32::from(p.seq)<<16)|(match status{Status::Done=>0,Status::RetryLimit=>1,Status::Lifetime=>2,Status::MacIdDrop=>3}<<13)|u32::from(p.qsel)<<8|u32::from(p.mac)}
    fn inject_last(&mut self,status:Status){let value=Self::report(self.published.last().unwrap(),status);self.inject(value);}
    fn complete_dma(&mut self,q:usize){let host=self.get(INDEX[q],4)&0xfff;self.put(INDEX[q],4,host|host<<16);}
    fn read(&mut self,at:usize,width:usize)->io::Result<u32>{
        io::check_offset(at,width,self.mmio.len())?;self.tick();
        if self.late_read==Some(at){self.now+=1_000_000;self.late_read=None;}Ok(self.get(at,width))
    }
    fn write(&mut self,at:usize,width:usize,value:u32)->io::Result<()>{
        io::check_offset(at,width,self.mmio.len())?;self.tick();self.writes.push((at,value));let previous=self.get(at,width);
        if !(self.ignore_stop&&at==fwcmd::DMA_STOP){self.put(at,width,value);}
        if at==0x1054&&width==2{
            let descriptor=(256+previous as usize)*8;assert!(descriptor+8<=4096);
            let raw=unsafe{std::slice::from_raw_parts(self.rx.add(descriptor),8)};
            let offset=(Self::word(raw,4)as usize).checked_sub(0x400000).unwrap();assert!(offset+24<=rx::DMA_BYTES);
            unsafe{std::slice::from_raw_parts_mut(self.rx.add(offset),24)}.fill(0xa5);
        }
        if width==2{if let Some(q)=INDEX.iter().position(|v|*v==at){
            let slot=(value as usize+DESCRIPTORS-1)%DESCRIPTORS;let physical=self.get(BASE[q],4)+slot as u32*8;
            let bd=self.dma(physical,8);assert_eq!(Self::word(bd,0),64|1<<30);
            let wd=self.dma(Self::word(bd,4),64).to_vec();let length=u16::from_le_bytes(wd[56..58].try_into().unwrap())as usize;
            assert!((24..=2346).contains(&length));let frame=self.dma(Self::word(&wd,60),length).to_vec();
            let p=Published{q,seq:u16::from_le_bytes(wd[48..50].try_into().unwrap())&0x7fff,mac:(Self::word(&wd,8)>>24)as u8,
                qsel:((Self::word(&wd,8)>>17)&31)as u8,wd,frame};self.published.push(p);
            if self.auto_dma{self.complete_dma(q);}
            if self.auto_rpq{let status=self.statuses.get(self.published.len()-1).copied().unwrap_or(Status::Done);self.inject_last(status);}
        }}
        if self.fail_write==Some(at){self.fail_write=None;return Err(Fault::new(FaultKind::Access,at,value));}Ok(())
    }
}
impl RegisterIo for Device{
    fn read8(&mut self,at:usize)->io::Result<u8>{self.read(at,1).map(|v|v as u8)}
    fn read16(&mut self,at:usize)->io::Result<u16>{self.read(at,2).map(|v|v as u16)}
    fn read32(&mut self,at:usize)->io::Result<u32>{self.read(at,4)}
    fn write8(&mut self,at:usize,v:u8)->io::Result<()>{self.write(at,1,u32::from(v))}
    fn write16(&mut self,at:usize,v:u16)->io::Result<()>{self.write(at,2,u32::from(v))}
    fn write32(&mut self,at:usize,v:u32)->io::Result<()>{self.write(at,4,v)}
    fn now_us(&mut self)->u64{self.now}
    fn delay_us(&mut self,us:u32)->io::Result<()>{if !self.frozen{self.now+=u64::from(us);}Ok(())}
}
impl Bus for Device{fn pci_command(&mut self)->io::Result<u16>{Ok(self.command)}}
fn frame(kind:u8,tid:Option<u8>,group:bool)->Vec<u8>{
    let mut f=vec![0;64];f[0]=kind<<2;if let Some(tid)=tid{f[0]|=0x80;f[24]=tid;}
    f[4]=if group{1}else{2};f[22]=0x50;f[23]=0x12;f
}
fn run(check:impl FnOnce(&mut Device,&mut Queue<'_>,&mut rx::Rings<'_>,&mut[u8],&Region)){
    let tx=Region::new(DMA_BYTES);let rx_mem=Region::new(rx::DMA_BYTES);let mut d=Device::new(&tx,&rx_mem);
    let mut q=unsafe{Queue::new(&mut d,tx.base,tx.len,0x200000).unwrap()};
    let mut rings=unsafe{rx::Rings::new(&mut d,rx_mem.base,rx_mem.len,0x400000).unwrap()};
    q.install(&mut d).unwrap();rings.install(&mut d).unwrap();d.command=6;d.put(fwcmd::INIT_CONFIG,4,HCI);d.put(fwcmd::DMA_STOP,4,UNUSED_STOP);
    unsafe{rings.start(&mut d).unwrap()};q.start(&mut d,&rings).unwrap();let mut scratch=vec![0;rx::BUFFER_BYTES];
    check(&mut d,&mut q,&mut rings,&mut scratch,&tx);
    q.stop(&mut d);rings.stop(&mut d);d.command=0;
}
#[test]
fn descriptor_preserves_raw_sequence_and_selects_exact_legacy_rate_and_selected_limit_one(){
    for(rate_index,rate)in txdesc::LEGACY_RATES.into_iter().enumerate(){
        let f=frame(0,None,false);let meta=txdesc::frame(&f,rate,15).unwrap();let wd=txdesc::descriptor(meta,7,0x1234,0x300000).unwrap();
        assert_eq!(Device::word(&wd,0),1<<22|8<<16|12<<11|1<<7);assert_eq!(Device::word(&wd,8),7<<24|0x12<<17|64);
        assert_eq!(Device::word(&wd,12),0x125);assert_eq!(Device::word(&wd,24),1<<30|(rate_index as u32)<<16|1<<10);
        assert_eq!(Device::word(&wd,28),1<<31|1<<25|(rate_index as u32)<<16);assert_eq!(&wd[32..48],&[0;16]);
        assert_eq!(&wd[48..56],&[0x34,0x92,0,0,0,0,0,0]);assert_eq!(&wd[56..64],&[64,0,1,128,0,0,48,0]);
    }
}
#[test]
fn frame_metadata_checks_lengths_qos_and_actual_tid_mapping(){
    for(tid,q,indicate)in [(0,0,false),(1,1,false),(2,1,true),(3,0,true),(4,2,false),(5,2,true),(6,3,false),(7,3,true)]{
        let mut f=frame(2,Some(tid),false);let meta=txdesc::frame(&f,12,3).unwrap();assert_eq!((meta.header,meta.queue,meta.tid_indicate),(26,q,indicate));
        f[1]=0x83;f[30]=tid;let meta=txdesc::frame(&f,12,3).unwrap();assert_eq!(meta.header,36);
    }
    let f=frame(0,None,false);for rate in [0,1,3,10,255]{assert_eq!(txdesc::frame(&f,rate,1),Err(txdesc::Error::Rate));}
    for attempts in [0,16,255]{assert_eq!(txdesc::frame(&f,2,attempts),Err(txdesc::Error::Count));}
    for len in [0,10,23,2347]{assert!(txdesc::frame(&vec![0;len],2,1).is_err());}
    for bad in [0x20,0x40,0x80,8]{let mut f=frame(2,Some(0),false);f[24]=bad;assert!(txdesc::frame(&f,2,1).is_err());}
    assert!(txdesc::frame(&frame(1,None,false),2,1).is_err());
}
#[test]
fn actual_rpq_and_dma_are_both_required_and_group_done_is_not_acknowledged(){
    for early_report in [false,true]{run(|d,q,r,s,_|{
        d.auto_dma=early_report==false;d.auto_rpq=early_report;q.begin(d,&frame(0,None,false),2,3,4,100_000).unwrap();
        assert_eq!(q.poll(d,r,s).unwrap(),None);
        if early_report{d.complete_dma(4);}else{d.inject_last(Status::Done);}
        assert_eq!(q.poll(d,r,s).unwrap(),Some(Completion{status:Status::Done,acked:true,attempts:AttemptCount::Unknown,completed_descriptors:1}));
    });}
    run(|d,q,r,s,_|{q.begin(d,&frame(2,None,true),108,15,0,100_000).unwrap();
        assert_eq!(q.poll(d,r,s).unwrap(),Some(Completion{status:Status::Done,acked:false,attempts:AttemptCount::Unknown,completed_descriptors:1}));assert_eq!(d.published.len(),1);});
    run(|d,q,r,s,_|{let mut f=frame(2,None,false);f.resize(2346,0x6a);q.begin(d,&f,2,1,0,100_000).unwrap();
        assert_eq!(q.poll(d,r,s).unwrap(),Some(Completion{status:Status::Done,acked:true,attempts:AttemptCount::Unknown,completed_descriptors:1}));assert_eq!(d.published[0].frame,f);});
}
#[test]
fn software_retries_only_after_confirmed_limit_keep_sequence_and_set_retry_bit(){
    run(|d,q,r,s,_|{d.statuses=vec![Status::RetryLimit,Status::RetryLimit,Status::Done];let f=frame(2,Some(7),false);
        q.begin(d,&f,48,5,3,100_000).unwrap();assert_eq!(q.poll(d,r,s).unwrap(),None);assert_eq!(q.poll(d,r,s).unwrap(),None);
        assert_eq!(q.poll(d,r,s).unwrap(),Some(Completion{status:Status::Done,acked:true,attempts:AttemptCount::Unknown,completed_descriptors:3}));assert_eq!(d.published.len(),3);
        for(i,p)in d.published.iter().enumerate(){assert_eq!((p.q,p.qsel,p.seq),(3,3,i as u16));let mut expected=f.clone();if i>0{expected[1]|=8;}assert_eq!(p.frame,expected);}
    });
    run(|d,q,r,s,_|{d.statuses=vec![Status::RetryLimit;15];q.begin(d,&frame(0,None,false),22,15,0,100_000).unwrap();
        for _ in 0..14{assert_eq!(q.poll(d,r,s).unwrap(),None);}assert_eq!(q.poll(d,r,s).unwrap(),Some(Completion{status:Status::RetryLimit,acked:false,attempts:AttemptCount::Unknown,completed_descriptors:15}));assert_eq!(d.published.len(),15);});
}
#[test]
fn lifetime_and_macid_drops_cannot_invent_actual_attempts_or_retry(){
    for status in [Status::Lifetime,Status::MacIdDrop]{run(|d,q,r,s,_|{
        d.statuses=vec![Status::RetryLimit,status];q.begin(d,&frame(0,None,false),2,15,0,100_000).unwrap();q.poll(d,r,s).unwrap();
        assert_eq!(q.poll(d,r,s),Err(Error::Dropped{status,completed_descriptors:1}));assert_eq!(q.state(),State::Faulted);assert_eq!(d.published.len(),2);
    });}
}
#[test]
fn normal_ring_rotation_continues_beyond_sixteen_packets_and_reclaims_every_ownership(){
    run(|d,q,r,s,_|{for i in 0..320{
        let f=frame(2,Some((i%8)as u8),false);q.begin(d,&f,12,1,0,100_000).unwrap();
        assert_eq!(q.poll(d,r,s).unwrap(),Some(Completion{status:Status::Done,acked:true,attempts:AttemptCount::Unknown,completed_descriptors:1}));
    }assert_eq!(q.serial,320);assert_eq!(q.issued,q.completed);assert!(q.pending.is_none());assert_eq!(d.rx_written,64);
    assert!(d.published.windows(2).all(|p|p[1].seq==p[0].seq+1));});
}
#[test]
fn wp_tag_wrap_occurs_only_after_prior_status_and_dma_and_host_counter_cannot_overflow(){
    run(|d,q,r,s,_|{for _ in 0..32770{q.begin(d,&frame(2,None,false),12,1,0,100_000).unwrap();
        assert_eq!(q.poll(d,r,s).unwrap(),Some(Completion{status:Status::Done,acked:true,attempts:AttemptCount::Unknown,completed_descriptors:1}));}
        assert_eq!(d.published[32767].seq,32767);assert_eq!(d.published[32768].seq,0);assert_eq!(q.serial,32770);
        assert_eq!(q.issued,q.completed);assert!(q.pending.is_none());
    });
    run(|d,q,_,_,_|{q.serial=u64::MAX;assert_eq!(q.begin(d,&frame(0,None,false),2,1,0,1000),Err(Error::Count));
        assert_eq!(q.state(),State::Faulted);assert!(d.published.is_empty());});
}
#[test]
fn foreign_polluted_malformed_or_conflicting_reports_fail_closed(){
    for change in [1u32<<31,1<<16,1<<8,1,4<<13]{run(|d,q,r,s,_|{d.auto_rpq=false;q.begin(d,&frame(0,None,false),2,1,0,100_000).unwrap();
        let report=Device::report(d.published.last().unwrap(),Status::Done)^change;d.inject(report);
        assert!(q.poll(d,r,s).is_err());assert_eq!(q.state(),State::Faulted);assert_eq!(d.published.len(),1);
    });}
    run(|d,q,_,_,_|{d.auto_dma=false;d.auto_rpq=false;q.begin(d,&frame(0,None,false),2,1,0,100_000).unwrap();
        let report=Device::report(d.published.last().unwrap(),Status::Done);q.reports(&report.to_le_bytes()).unwrap();q.reports(&report.to_le_bytes()).unwrap();
        assert_eq!(q.reports(&(report|1<<13).to_le_bytes()),Err(Error::Report));assert!(q.reports(&[0;3]).is_err());});
}
#[test]
fn timeout_or_applied_doorbell_failure_never_reuses_held_buffers(){
    run(|d,q,r,s,mem|{d.auto_dma=false;d.auto_rpq=false;q.begin(d,&frame(0,None,false),2,2,0,1000).unwrap();
        let before=mem.bytes(BUFFERS_AT,STRIDE).to_vec();d.now+=1000;assert!(matches!(q.poll(d,r,s),Err(Error::Io(Fault{kind:FaultKind::Timeout,..}))));
        assert_eq!(q.state(),State::Faulted);assert_eq!(q.begin(d,&frame(0,None,false),2,1,0,1000),Err(Error::State));assert_eq!(mem.bytes(BUFFERS_AT,STRIDE),before);
    });
    run(|d,q,_,_,mem|{d.fail_write=Some(INDEX[4]);assert!(matches!(q.begin(d,&frame(0,None,false),2,1,0,1000),Err(Error::Io(_))));
        assert_eq!(q.state(),State::Faulted);assert!(q.pending.is_some());assert_eq!(d.published.len(),1);assert_ne!(mem.bytes(BUFFERS_AT,64),&[0;64]);});
}
#[test]
fn bounded_poll_returns_with_frozen_clock_and_rejects_regression_and_late_final_read(){
    run(|d,q,r,s,_|{d.frozen=true;d.auto_dma=false;d.auto_rpq=false;q.begin(d,&frame(0,None,false),2,1,0,1000).unwrap();
        let before=d.writes.len();assert_eq!(q.poll(d,r,s).unwrap(),None);assert_eq!(d.writes.len(),before);
        d.now-=1;assert!(matches!(q.poll(d,r,s),Err(Error::Io(Fault{kind:FaultKind::Clock,..}))));});
    run(|d,q,r,s,_|{d.frozen=true;d.auto_dma=false;d.auto_rpq=false;q.begin(d,&frame(0,None,false),2,1,0,8).unwrap();
        for _ in 0..8{assert_eq!(q.poll(d,r,s).unwrap(),None);}
        assert!(matches!(q.poll(d,r,s),Err(Error::Io(Fault{kind:FaultKind::Timeout,..}))));assert_eq!(d.published.len(),1);});
    run(|d,q,r,s,_|{q.begin(d,&frame(0,None,false),2,1,0,1000).unwrap();d.late_read=Some(INDEX[4]);
        assert!(matches!(q.poll(d,r,s),Err(Error::Io(Fault{kind:FaultKind::Timeout,..}))));assert_eq!(q.state(),State::Faulted);});
}
#[test]
fn dma_region_and_partial_install_failures_are_rejected_before_activation(){
    let tx=Region::new(DMA_BYTES);let rx_mem=Region::new(rx::DMA_BYTES);let mut d=Device::new(&tx,&rx_mem);
    for (ptr,len,physical) in [(std::ptr::null_mut(),DMA_BYTES,0x200000),(tx.base,DMA_BYTES-1,0x200000),
        (tx.base,DMA_BYTES,0x200001),(unsafe{tx.base.add(1)},DMA_BYTES,0x200000),(tx.base,DMA_BYTES,0xfffff000)]{
        assert!(matches!(unsafe{Queue::new(&mut d,ptr,len,physical)},Err(Error::Region)));}
    let mut q=unsafe{Queue::new(&mut d,tx.base,tx.len,0x200000).unwrap()};d.fail_write=Some(BASE[2]);
    assert!(matches!(q.install(&mut d),Err(Error::Io(_))));assert_eq!(q.state(),State::Faulted);
    assert_eq!(d.command,2);assert_eq!(d.get(fwcmd::INIT_CONFIG,4)&HCI,0);assert_eq!(d.get(fwcmd::DMA_STOP,4)&OWN_STOP,OWN_STOP);
}
#[test]
fn invalid_input_busy_and_short_scratch_do_not_poison_a_valid_pending_frame(){
    run(|d,q,r,s,_|{assert!(q.begin(d,&[0;23],2,1,0,1000).is_err());assert_eq!(q.state(),State::Running);
        q.begin(d,&frame(0,None,false),2,1,0,100_000).unwrap();assert_eq!(q.begin(d,&frame(0,None,false),2,1,0,1000),Err(Error::Busy));
        assert_eq!(q.poll(d,r,&mut[0;32]),Err(Error::Rx(rx::Error::Output)));assert_eq!(q.state(),State::Running);
        assert_eq!(q.poll(d,r,s).unwrap(),Some(Completion{status:Status::Done,acked:true,attempts:AttemptCount::Unknown,completed_descriptors:1}));});
}
#[test]
fn altered_dma_mapping_count_index_or_inaccessible_pci_is_not_accepted(){
    for mode in 0..4{run(|d,q,r,s,_|{d.auto_rpq=false;q.begin(d,&frame(0,None,false),2,1,0,100_000).unwrap();
        match mode{0=>d.put(BASE[4],4,0x300000),1=>d.put(NUMBER[4],2,31),2=>d.put(INDEX[4],4,1|32<<16),_=>d.command=0xffff}
        assert!(q.poll(d,r,s).is_err());assert_eq!(q.state(),State::Faulted);});}
}
#[test]
fn independent_stop_retains_failure_and_never_disables_command_or_receive_dma(){
    run(|d,q,_,_,_|{d.ignore_stop=true;let stop=q.stop(d);assert!(stop.request.is_some());assert_eq!(stop.idle,None);assert_eq!(q.state(),State::Faulted);
        assert_eq!(d.get(fwcmd::INIT_CONFIG,4),HCI);assert_eq!(d.get(fwcmd::DMA_STOP,4)&(3|1<<18),0);
        d.ignore_stop=false;d.put(fwcmd::DMA_BUSY,4,OWN_STOP);let stop=q.stop(d);assert_eq!(stop.request,None);assert!(stop.idle.is_some());assert_eq!(q.state(),State::Faulted);});
}
#[test]
fn firmware_counter_codec_keeps_its_distinct_class_and_six_bit_count(){
    let mut p=[0;32];p[..4].copy_from_slice(&(1u32|9<<2|1<<8).to_le_bytes());p[4]=32;
    p[8..12].copy_from_slice(&(13u32<<8|1<<6).to_le_bytes());p[20..24].copy_from_slice(&(15u32<<8).to_le_bytes());
    assert_eq!(txdesc::firmware_report(&p).unwrap(),Some(txdesc::FirmwareReport{sequence:13,status:Status::RetryLimit,raw_data_tx_count:15}));
    p[0]=1;assert_eq!(txdesc::firmware_report(&p).unwrap(),None);p[0]=1|9<<2;p[4]=31;assert!(txdesc::firmware_report(&p).is_err());
}
