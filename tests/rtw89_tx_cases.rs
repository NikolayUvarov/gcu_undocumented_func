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
    mmio:Vec<u8>,command:u16,now:u64,frozen:bool,tx:*mut u8,rx:*mut u8,rx_written:[u16;2],
    published:Vec<Published>,statuses:Vec<Status>,auto_dma:bool,auto_rpq:bool,auto_firmware:bool,
    fail_write:Option<usize>,ignore_stop:bool,late_read:Option<usize>,writes:Vec<(usize,u32)>,
}
impl Device{
    fn new(tx:&Region,rx:&Region)->Self{
        let mut d=Self{mmio:vec![0;0x20000],command:2,now:1,frozen:false,tx:tx.base,rx:rx.base,rx_written:[0;2],
            published:Vec::new(),statuses:Vec::new(),auto_dma:true,auto_rpq:true,auto_firmware:true,fail_write:None,ignore_stop:false,late_read:None,writes:Vec::new()};
        d.put(fwcmd::DMA_STOP,4,0x001f0f03);d.put(0x1e0,4,0xe0);d.put(0xc660,1,0x10);d
    }
    fn get(&self,at:usize,width:usize)->u32{let mut raw=[0;4];raw[..width].copy_from_slice(&self.mmio[at..at+width]);u32::from_le_bytes(raw)}
    fn put(&mut self,at:usize,width:usize,value:u32){self.mmio[at..at+width].copy_from_slice(&value.to_le_bytes()[..width]);}
    fn tick(&mut self){if !self.frozen{self.now+=1;}}
    fn dma(&self,physical:u32,len:usize)->&[u8]{
        let at=(physical as usize).checked_sub(0x200000).unwrap();assert!(at.checked_add(len).is_some_and(|end|end<=DMA_BYTES));
        unsafe{std::slice::from_raw_parts(self.tx.add(at),len)}
    }
    fn word(bytes:&[u8],at:usize)->u32{u32::from_le_bytes(bytes[at..at+4].try_into().unwrap())}
    fn packet(&mut self,channel:usize,kind:u8,payload:&[u8]){
        let index=0x1050+4*channel;let host=self.get(index,4)&0xfff;
        let next=(self.rx_written[channel]+1)%256;assert_ne!(u32::from(next),host);
        let descriptor=(256*channel+usize::from(self.rx_written[channel]))*8;
        assert!(descriptor+8<=4096);
        let raw=unsafe{std::slice::from_raw_parts(self.rx.add(descriptor),8)};
        assert_eq!(Self::word(raw,0),rx::BUFFER_BYTES as u32);
        let at=(Self::word(raw,4)as usize).checked_sub(0x400000).unwrap();
        let length=20usize.checked_add(payload.len()).unwrap();assert!(length<=rx::BUFFER_BYTES);
        assert!(at.checked_add(length).is_some_and(|end|end<=rx::DMA_BYTES));
        let bytes=unsafe{std::slice::from_raw_parts_mut(self.rx.add(at),length)};bytes.fill(0);
        bytes[..4].copy_from_slice(&(length as u32|0xc000).to_le_bytes());
        bytes[4..8].copy_from_slice(&(payload.len()as u32|u32::from(kind)<<24).to_le_bytes());
        bytes[20..].copy_from_slice(payload);self.rx_written[channel]=next;self.put(index,4,host|u32::from(next)<<16);
    }
    fn inject(&mut self,report:u32){self.packet(1,7,&report.to_le_bytes());}
    fn firmware_bytes(sequence:u8,status:Status,count:u8)->[u8;32]{
        let mut p=[0;32];p[..4].copy_from_slice(&(1u32|9<<2|1<<8).to_le_bytes());p[4]=32;
        let status=match status{Status::Done=>0,Status::RetryLimit=>1,Status::Lifetime=>2,Status::MacIdDrop=>3};
        p[8..12].copy_from_slice(&(u32::from(sequence)<<8|status<<6).to_le_bytes());
        p[20..24].copy_from_slice(&(u32::from(count)<<8).to_le_bytes());p
    }
    fn firmware_last(&mut self,status:Status,count:u8){
        let p=self.published.last().unwrap();let wd=Device::word(&p.wd,40);
        assert_ne!(Device::word(&p.wd,36)&(1<<10),0);self.packet(0,10,&Self::firmware_bytes((wd&15)as u8,status,count));
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
        if [0x1050,0x1054].contains(&at)&&width==2{
            let descriptor=(256*((at-0x1050)/4)+previous as usize)*8;assert!(descriptor+8<=4096);
            let raw=unsafe{std::slice::from_raw_parts(self.rx.add(descriptor),8)};
            let offset=(Self::word(raw,4)as usize).checked_sub(0x400000).unwrap();assert!(offset+rx::BUFFER_BYTES<=rx::DMA_BYTES);
            unsafe{std::slice::from_raw_parts_mut(self.rx.add(offset),rx::BUFFER_BYTES)}.fill(0xa5);
        }
        if width==2{if let Some(q)=INDEX.iter().position(|v|*v==at){
            let slot=(value as usize+DESCRIPTORS-1)%DESCRIPTORS;let physical=self.get(BASE[q],4)+slot as u32*8;
            let bd=self.dma(physical,8);assert_eq!(Self::word(bd,0),64|1<<30);
            let wd=self.dma(Self::word(bd,4),64).to_vec();let length=u16::from_le_bytes(wd[56..58].try_into().unwrap())as usize;
            assert!((24..=2346).contains(&length));let frame=self.dma(Self::word(&wd,60),length).to_vec();
            let p=Published{q,seq:u16::from_le_bytes(wd[48..50].try_into().unwrap())&0x7fff,mac:(Self::word(&wd,8)>>24)as u8,
                qsel:((Self::word(&wd,8)>>17)&31)as u8,wd,frame};self.published.push(p);
            if self.auto_dma{self.complete_dma(q);}
            let status=self.statuses.get(self.published.len()-1).copied().unwrap_or(Status::Done);
            if self.auto_rpq{self.inject_last(status);}
            if self.auto_firmware{self.firmware_last(status,1);}
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
fn cycle(q:&mut Queue<'_>,d:&mut Device,r:&mut rx::Rings<'_>,s:&mut[u8])->Result<Option<Completion>>{
    let received=q.poll_receive(d,r,s)?;assert!(matches!(received,Receive::Empty|Receive::TxReport{..}));
    q.poll(d,r,s)
}
#[test]
fn descriptor_preserves_raw_sequence_and_selects_exact_legacy_rate_and_selected_limit_one(){
    for(rate_index,rate)in txdesc::LEGACY_RATES.into_iter().enumerate(){
        let f=frame(0,None,false);let meta=txdesc::frame(&f,rate,15).unwrap();let wd=txdesc::descriptor(meta,7,0x1234,0x300000).unwrap();
        assert_eq!(Device::word(&wd,0),1<<22|8<<16|12<<11|1<<7);assert_eq!(Device::word(&wd,8),7<<24|0x12<<17|64);
        assert_eq!(Device::word(&wd,12),0x125);assert_eq!(Device::word(&wd,24),1<<30|(rate_index as u32)<<16|1<<10);
        assert_eq!(Device::word(&wd,28),1<<31|1<<25|(rate_index as u32)<<16);assert_eq!(Device::word(&wd,32),0);assert_eq!(Device::word(&wd,36),1<<10);assert_eq!(Device::word(&wd,40),4);assert_eq!(Device::word(&wd,44),0);
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
fn management_order_bit_requires_complete_ht_control_and_encodes_its_length(){
    let mut f=frame(0,None,false);f[1]=0x80;f.truncate(28);
    let meta=txdesc::frame(&f,12,1).unwrap();assert_eq!(meta.header,28);
    let wd=txdesc::descriptor(meta,0,1,0x300000).unwrap();assert_eq!((Device::word(&wd,0)>>11)&31,14);
    for len in 24..28{assert_eq!(txdesc::frame(&f[..len],12,1),Err(txdesc::Error::Length));}
    f[1]=0;f.truncate(24);assert_eq!(txdesc::frame(&f,12,1).unwrap().header,24);
}
#[test]
fn actual_rpq_and_dma_are_both_required_and_group_done_is_not_acknowledged(){
    for early_report in [false,true]{run(|d,q,r,s,_|{
        d.auto_dma=early_report==false;d.auto_rpq=early_report;q.begin(d,&frame(0,None,false),2,3,4,100_000).unwrap();
        assert_eq!(cycle(q,d,r,s).unwrap(),None);
        if early_report{d.complete_dma(4);}else{d.inject_last(Status::Done);}
        assert_eq!(cycle(q,d,r,s).unwrap(),Some(Completion{status:Status::Done,acked:true,attempts:AttemptCount::Known(1),completed_descriptors:1,raw_data_tx_count:1}));
    });}
    run(|d,q,r,s,_|{q.begin(d,&frame(2,None,true),108,15,0,100_000).unwrap();
        assert_eq!(cycle(q,d,r,s).unwrap(),Some(Completion{status:Status::Done,acked:false,attempts:AttemptCount::Known(1),completed_descriptors:1,raw_data_tx_count:1}));assert_eq!(d.published.len(),1);});
    run(|d,q,r,s,_|{let mut f=frame(2,None,false);f.resize(2346,0x6a);q.begin(d,&f,2,1,0,100_000).unwrap();
        assert_eq!(cycle(q,d,r,s).unwrap(),Some(Completion{status:Status::Done,acked:true,attempts:AttemptCount::Known(1),completed_descriptors:1,raw_data_tx_count:1}));assert_eq!(d.published[0].frame,f);});
}
#[test]
fn software_retries_only_after_confirmed_limit_keep_sequence_and_set_retry_bit(){
    run(|d,q,r,s,_|{d.statuses=vec![Status::RetryLimit,Status::RetryLimit,Status::Done];let f=frame(2,Some(7),false);
        q.begin(d,&f,48,5,3,100_000).unwrap();assert_eq!(cycle(q,d,r,s).unwrap(),None);assert_eq!(cycle(q,d,r,s).unwrap(),None);
        assert_eq!(cycle(q,d,r,s).unwrap(),Some(Completion{status:Status::Done,acked:true,attempts:AttemptCount::Known(3),completed_descriptors:3,raw_data_tx_count:3}));assert_eq!(d.published.len(),3);
        for(i,p)in d.published.iter().enumerate(){assert_eq!((p.q,p.qsel,p.seq),(3,3,i as u16));let mut expected=f.clone();if i>0{expected[1]|=8;}assert_eq!(p.frame,expected);}
    });
    run(|d,q,r,s,_|{d.statuses=vec![Status::RetryLimit;15];q.begin(d,&frame(0,None,false),22,15,0,100_000).unwrap();
        for _ in 0..14{assert_eq!(cycle(q,d,r,s).unwrap(),None);}assert_eq!(cycle(q,d,r,s).unwrap(),Some(Completion{status:Status::RetryLimit,acked:false,attempts:AttemptCount::Known(15),completed_descriptors:15,raw_data_tx_count:15}));assert_eq!(d.published.len(),15);});
}
#[test]
fn lifetime_and_macid_drops_cannot_invent_actual_attempts_or_retry(){
    for status in [Status::Lifetime,Status::MacIdDrop]{run(|d,q,r,s,_|{
        d.statuses=vec![Status::RetryLimit,status];q.begin(d,&frame(0,None,false),2,15,0,100_000).unwrap();cycle(q,d,r,s).unwrap();
        assert_eq!(cycle(q,d,r,s),Err(Error::Dropped{status,completed_descriptors:1}));assert_eq!(q.state(),State::Faulted);assert_eq!(d.published.len(),2);
    });}
}
#[test]
fn normal_ring_rotation_continues_beyond_sixteen_packets_and_reclaims_every_ownership(){
    run(|d,q,r,s,_|{for i in 0..320{
        let f=frame(2,Some((i%8)as u8),false);q.begin(d,&f,12,1,0,100_000).unwrap();
        assert_eq!(cycle(q,d,r,s).unwrap(),Some(Completion{status:Status::Done,acked:true,attempts:AttemptCount::Known(1),completed_descriptors:1,raw_data_tx_count:1}));
    }assert_eq!(q.serial,320);assert_eq!(q.issued,q.completed);assert!(q.pending.is_none());assert_eq!(d.rx_written,[64;2]);
    assert!(d.published.windows(2).all(|p|p[1].seq==p[0].seq+1));});
}
#[test]
fn wp_tag_wrap_occurs_only_after_prior_status_and_dma_and_host_counter_cannot_overflow(){
    run(|d,q,r,s,_|{for _ in 0..32770{q.begin(d,&frame(2,None,false),12,1,0,100_000).unwrap();
        assert_eq!(cycle(q,d,r,s).unwrap(),Some(Completion{status:Status::Done,acked:true,attempts:AttemptCount::Known(1),completed_descriptors:1,raw_data_tx_count:1}));}
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
        assert!(cycle(q,d,r,s).is_err());assert_eq!(q.state(),State::Faulted);assert_eq!(d.published.len(),1);
    });}
    run(|d,q,_,_,_|{d.auto_dma=false;d.auto_rpq=false;q.begin(d,&frame(0,None,false),2,1,0,100_000).unwrap();
        let report=Device::report(d.published.last().unwrap(),Status::Done);q.reports(&report.to_le_bytes()).unwrap();q.reports(&report.to_le_bytes()).unwrap();
        assert_eq!(q.reports(&(report|1<<13).to_le_bytes()),Err(Error::Report));assert!(q.reports(&[0;3]).is_err());});
}
#[test]
fn timeout_or_applied_doorbell_failure_never_reuses_held_buffers(){
    run(|d,q,r,s,mem|{d.auto_dma=false;d.auto_rpq=false;q.begin(d,&frame(0,None,false),2,2,0,1000).unwrap();
        let before=mem.bytes(BUFFERS_AT,STRIDE).to_vec();d.now+=1000;assert!(matches!(cycle(q,d,r,s),Err(Error::Io(Fault{kind:FaultKind::Timeout,..}))));
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
        assert_eq!(cycle(q,d,r,s).unwrap(),Some(Completion{status:Status::Done,acked:true,attempts:AttemptCount::Known(1),completed_descriptors:1,raw_data_tx_count:1}));});
}
#[test]
fn altered_dma_mapping_count_index_or_inaccessible_pci_is_not_accepted(){
    for mode in 0..4{run(|d,q,r,s,_|{d.auto_rpq=false;q.begin(d,&frame(0,None,false),2,1,0,100_000).unwrap();
        match mode{0=>d.put(BASE[4],4,0x300000),1=>d.put(NUMBER[4],2,31),2=>d.put(INDEX[4],4,1|32<<16),_=>d.command=0xffff}
        assert!(cycle(q,d,r,s).is_err());assert_eq!(q.state(),State::Faulted);});}
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

#[test]
fn firmware_rpq_and_dma_all_finish_before_release_in_every_completion_order(){
    for order in [[0,1,2],[0,2,1],[1,0,2],[1,2,0],[2,0,1],[2,1,0]]{run(|d,q,r,s,mem|{
        d.auto_dma=false;d.auto_rpq=false;d.auto_firmware=false;
        q.begin(d,&frame(0,None,false),2,1,0,100_000).unwrap();let held=mem.bytes(BUFFERS_AT,STRIDE).to_vec();
        for(i,event)in order.into_iter().enumerate(){
            match event{0=>{d.firmware_last(Status::Done,1);assert_eq!(q.poll_receive(d,r,s).unwrap(),Receive::TxReport{final_report:true});},
                1=>d.inject_last(Status::Done),_=>d.complete_dma(4)}
            let completion=q.poll(d,r,s).unwrap();
            if i<2{assert_eq!(completion,None);assert!(q.pending.is_some());assert_eq!(mem.bytes(BUFFERS_AT,STRIDE),held);}
            else{assert_eq!(completion,Some(Completion{status:Status::Done,acked:true,attempts:AttemptCount::Known(1),completed_descriptors:1,raw_data_tx_count:1}));}
        }
    });}
}
#[test]
fn interim_report_never_counts_and_only_final_firmware_attempts_accumulate(){
    run(|d,q,r,s,_|{d.auto_firmware=false;d.statuses=vec![Status::RetryLimit,Status::Done];
        q.begin(d,&frame(0,None,false),2,2,0,100_000).unwrap();
        d.firmware_last(Status::RetryLimit,0);assert_eq!(q.poll_receive(d,r,s).unwrap(),Receive::TxReport{final_report:false});
        assert_eq!(q.poll(d,r,s).unwrap(),None);assert_eq!(d.published.len(),1);
        d.firmware_last(Status::RetryLimit,1);assert_eq!(q.poll_receive(d,r,s).unwrap(),Receive::TxReport{final_report:true});
        assert_eq!(q.poll(d,r,s).unwrap(),None);assert_eq!(d.published.len(),2);
        d.firmware_last(Status::Done,1);assert_eq!(q.poll_receive(d,r,s).unwrap(),Receive::TxReport{final_report:true});
        assert_eq!(q.poll(d,r,s).unwrap(),Some(Completion{status:Status::Done,acked:true,attempts:AttemptCount::Known(2),completed_descriptors:2,raw_data_tx_count:2}));
    });
}
#[test]
fn impossible_firmware_attempts_poison_without_releasing_or_retrying(){
    for (status,count) in [(Status::Done,0),(Status::Done,2),(Status::Done,63),(Status::RetryLimit,2)]{
        run(|d,q,r,s,mem|{d.auto_firmware=false;
            q.begin(d,&frame(0,None,false),2,15,0,100_000).unwrap();
            let held=mem.bytes(BUFFERS_AT,STRIDE).to_vec();
            assert_eq!(q.poll(d,r,s).unwrap(),None);assert_eq!(q.issued,q.completed);
            d.firmware_last(status,count);assert_eq!(q.poll_receive(d,r,s),Err(Error::Report));
            assert_eq!(q.state(),State::Faulted);assert!(q.pending.is_some());
            assert_eq!(d.published.len(),1);assert_eq!(mem.bytes(BUFFERS_AT,STRIDE),held);
            assert_eq!(q.begin(d,&frame(0,None,false),2,1,0,1000),Err(Error::State));
        });
    }
}
#[test]
fn firmware_count_alone_never_claims_an_attempt_or_releases_payload(){
    run(|d,q,r,s,mem|{d.auto_rpq=false;d.auto_dma=false;
        q.begin(d,&frame(0,None,false),2,15,0,100_000).unwrap();
        let held=mem.bytes(BUFFERS_AT,STRIDE).to_vec();
        assert_eq!(q.poll_receive(d,r,s).unwrap(),Receive::TxReport{final_report:true});
        assert_eq!(q.poll(d,r,s).unwrap(),None);assert_eq!(q.request.as_ref().unwrap().raw_count,0);
        assert_eq!(q.request.as_ref().unwrap().completed,0);assert_eq!(d.published.len(),1);
        assert_eq!(mem.bytes(BUFFERS_AT,STRIDE),held);
        d.inject_last(Status::Lifetime);d.complete_dma(4);
        assert_eq!(q.poll(d,r,s),Err(Error::Dropped{status:Status::Lifetime,completed_descriptors:0}));
        assert!(q.pending.is_some());assert_eq!(q.request.as_ref().unwrap().raw_count,0);
    });
}
#[test]
fn missing_final_firmware_report_retains_dma_and_timeout_never_reopens_tag(){
    run(|d,q,r,s,mem|{d.auto_firmware=false;q.begin(d,&frame(0,None,false),2,1,0,100_000).unwrap();
        assert_eq!(q.poll(d,r,s).unwrap(),None);assert_eq!(q.issued,q.completed);assert!(q.pending.is_some());
        let held=mem.bytes(BUFFERS_AT,STRIDE).to_vec();d.now+=100_000;
        assert!(matches!(q.poll_receive(d,r,s),Err(Error::Io(Fault{kind:FaultKind::Timeout,..}))));
        d.firmware_last(Status::Done,1);assert_eq!(q.poll(d,r,s),Err(Error::State));
        assert_eq!(q.begin(d,&frame(0,None,false),2,1,0,1000),Err(Error::State));assert_eq!(mem.bytes(BUFFERS_AT,STRIDE),held);
    });
}
#[test]
fn firmware_wrong_tag_counter_regression_conflict_or_rpq_disagreement_are_terminal(){
    for mode in 0..5{run(|d,q,r,s,_|{d.auto_firmware=false;q.begin(d,&frame(0,None,false),2,1,0,100_000).unwrap();
        match mode{
            0=>d.packet(0,10,&Device::firmware_bytes(1,Status::Done,1)),
            1=>{d.firmware_last(Status::RetryLimit,1);q.poll_receive(d,r,s).unwrap();d.firmware_last(Status::RetryLimit,0);},
            2=>{d.firmware_last(Status::Done,1);q.poll_receive(d,r,s).unwrap();d.firmware_last(Status::RetryLimit,1);},
            3=>d.firmware_last(Status::RetryLimit,1),
            _=>{let mut p=Device::firmware_bytes(0,Status::Done,1);p[4]=31;d.packet(0,10,&p);},
        }
        if mode==3{q.poll_receive(d,r,s).unwrap();assert_eq!(q.poll(d,r,s),Err(Error::Report));}
        else{assert!(q.poll_receive(d,r,s).is_err());}
        assert_eq!(q.state(),State::Faulted);assert_eq!(d.published.len(),1);
    });}
}
#[test]
fn exact_final_duplicate_does_not_release_or_add_a_second_counter(){
    run(|d,q,r,s,_|{d.auto_dma=false;q.begin(d,&frame(0,None,false),2,1,0,100_000).unwrap();
        assert_eq!(q.poll_receive(d,r,s).unwrap(),Receive::TxReport{final_report:true});d.firmware_last(Status::Done,1);
        assert_eq!(q.poll_receive(d,r,s).unwrap(),Receive::TxReport{final_report:true});assert_eq!(q.poll(d,r,s).unwrap(),None);
        d.complete_dma(4);assert_eq!(q.poll(d,r,s).unwrap().unwrap().raw_data_tx_count,1);
    });
}
#[test]
fn non_tx_c2h_and_wifi_bytes_are_returned_before_scratch_reuse(){
    run(|d,q,r,s,_|{d.auto_firmware=false;q.begin(d,&frame(0,None,false),2,1,0,100_000).unwrap();
        let mut event=[0;12];event[..4].copy_from_slice(&(1u32|0x12<<2|1<<8).to_le_bytes());event[4]=12;event[8..].copy_from_slice(&[1,2,3,4]);
        for (kind,bytes)in [(10,event.as_slice()),(0,frame(2,None,false).as_slice())]{
            d.packet(0,kind,bytes);let Receive::Other(rx::Outcome::Packet(packet))=q.poll_receive(d,r,s).unwrap()else{panic!("packet lost")};
            assert_eq!(packet.kind,kind);assert_eq!(packet.bytes(s).unwrap(),bytes);
        }
        assert_eq!(q.poll(d,r,s).unwrap(),None);assert!(q.pending.unwrap().firmware.is_none());
        d.firmware_last(Status::Done,1);assert!(cycle(q,d,r,s).unwrap().is_some());
    });
}
#[test]
fn firmware_poll_uses_shared_finite_budget_and_checks_final_clock(){
    run(|d,q,r,s,_|{d.frozen=true;d.auto_firmware=false;d.auto_rpq=false;d.auto_dma=false;
        q.begin(d,&frame(0,None,false),2,1,0,8).unwrap();
        for _ in 0..4{assert_eq!(q.poll_receive(d,r,s).unwrap(),Receive::Empty);assert_eq!(q.poll(d,r,s).unwrap(),None);}
        assert!(matches!(q.poll_receive(d,r,s),Err(Error::Io(Fault{kind:FaultKind::Timeout,..}))));
    });
    run(|d,q,r,s,_|{q.begin(d,&frame(0,None,false),2,1,0,100_000).unwrap();d.late_read=Some(0x1050);
        assert!(matches!(q.poll_receive(d,r,s),Err(Error::Io(Fault{kind:FaultKind::Timeout,..}))));assert!(q.pending.is_some());
    });
}

#[test]
fn missing_firmware_report_forwarding_refuses_publication(){
    for value in [0,0x20,0x30]{run(|d,q,_,_,_|{d.put(0xc660,1,value);
        assert_eq!(q.begin(d,&frame(0,None,false),2,1,0,100_000),Err(Error::State));
        assert_eq!(q.state(),State::Faulted);assert!(d.published.is_empty());
    });}
}

#[test]
fn retained_dispatch_records_complete_tx_without_any_tx_receive_ring_poll(){
    for reverse in [false,true]{run(|d,q,r,s,_|{
        q.begin(d,&frame(0,None,false),12,3,0,100_000).unwrap();
        let mut records=Vec::new();
        for channel in if reverse{[rx::Channel::Rpq,rx::Channel::Rxq]}else{[rx::Channel::Rxq,rx::Channel::Rpq]}{
            let outcome=r.receive(d,channel,s,1000).unwrap();let rx::Outcome::Packet(packet)=outcome else{panic!("missing owned record")};
            records.push((channel,outcome,s[..packet.offset+packet.length].to_vec()));s.fill(0x99);
        }
        let indices=[d.get(0x1050,4),d.get(0x1054,4)];let writes=d.writes.len();
        for(channel,outcome,bytes)in records{
            let observed=q.ingest(d.now_us(),channel,outcome,&bytes).unwrap();
            assert_eq!(observed,if channel==rx::Channel::Rpq{Receive::Rpq}else{Receive::TxReport{final_report:true}});
        }
        assert_eq!(d.writes.len(),writes);
        let completed=q.advance(d).unwrap().unwrap();assert!(completed.acked);assert_eq!(completed.completed_descriptors,1);
        assert_eq!([d.get(0x1050,4),d.get(0x1054,4)],indices);assert_eq!(d.writes.len(),writes);
    });}
}
#[test]
fn retained_tx_ingest_preserves_other_packet_and_rejects_corrupt_rpq(){run(|d,q,_,_,_|{
    d.auto_rpq=false;d.auto_firmware=false;q.begin(d,&frame(0,None,false),2,1,0,100_000).unwrap();
    let mut packet=crate::rxdesc::Packet{offset:0,length:4,kind:0,long:false,crc_error:false,icv_error:false,hardware_decrypted:false};
    let outcome=rx::Outcome::Packet(packet);assert_eq!(q.ingest(d.now_us(),rx::Channel::Rxq,outcome,&[1,2,3,4]).unwrap(),Receive::Other(outcome));
    assert!(q.advance(d).unwrap().is_none());
    packet.crc_error=true;packet.kind=7;assert_eq!(q.ingest(d.now_us(),rx::Channel::Rpq,rx::Outcome::Packet(packet),&[0;4]),Err(Error::Report));
    assert_eq!(q.state(),State::Faulted);
});}
#[test]
fn retained_ingest_and_dma_progress_share_finite_deadline_and_poll_budget(){run(|d,q,_,_,_|{
    d.frozen=true;d.auto_rpq=false;d.auto_firmware=false;d.auto_dma=false;q.begin(d,&frame(0,None,false),2,1,0,8).unwrap();
    for _ in 0..4{assert_eq!(q.ingest(d.now_us(),rx::Channel::Rxq,rx::Outcome::Empty,&[]).unwrap(),Receive::Empty);assert_eq!(q.advance(d).unwrap(),None);}
    assert!(matches!(q.advance(d),Err(Error::Io(Fault{kind:FaultKind::Timeout,..}))));assert_eq!(d.published.len(),1);
});}
