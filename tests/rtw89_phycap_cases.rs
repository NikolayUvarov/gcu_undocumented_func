use super::*;
use crate::{fwcmd,mac,registers::*,runtime_mac_regs::*};

struct Device{
    regs:Vec<u8>,command:u16,now:u64,writes:Vec<(usize,usize,u32)>,response:[u32;4],
    no_reply:bool,frozen:bool,fail_write:Option<usize>,late_counter:Option<u64>,late_guard:Option<u64>,
    fail_cleanup:bool,reads:usize,
}
impl Device{
    fn new(q:&Queue<'_>)->Self{
        let mut d=Self{regs:vec![0;runtime_mac::BAR_BYTES],command:6,now:100,writes:Vec::new(),
            response:[0x02020403,0x12345602,0x0a,0x03020200],no_reply:false,frozen:false,fail_write:None,
            late_counter:None,late_guard:None,fail_cleanup:false,reads:0};
        for(at,v)in[(SYS_CFG1,1<<12),(R_AX_IC_PWR_STATE,1<<8),(R_AX_PLATFORM_ENABLE,3),
            (R_AX_CMAC_FUNC_EN,1<<30),(download::FW_CONTROL,0xe0),(mac::PLE_CONFIG,0x00400801),
            (fwcmd::INIT_CONFIG,1<<11),(fwcmd::DMA_STOP,0xb0f03),(fwcmd::BASE_LOW,q.physical()as u32),
            (fwcmd::BDRAM,28|4<<8|1<<16),(fwcmd::NUMBER,32)]{d.put(at,4,v);}d
    }
    fn put(&mut self,at:usize,n:usize,v:u32){self.regs[at..at+n].copy_from_slice(&v.to_le_bytes()[..n]);}
    fn get(&mut self,at:usize,n:usize)->u32{
        self.reads+=1;let mut v=[0;4];v[..n].copy_from_slice(&self.regs[at..at+n]);
        if at==R_AX_WLAN_XTAL_SI_CTRL{if let Some(now)=self.late_guard.take(){self.now=now;}}
        u32::from_le_bytes(v)
    }
    fn write(&mut self,at:usize,n:usize,v:u32)->io::Result<()>{
        self.writes.push((at,n,v));self.put(at,n,v);
        if at==R_AX_H2CREG_CTRL&&v==1{
            assert_eq!((0..4).map(|i|self.get(R_AX_H2CREG_DATA0+i*4,4)).collect::<Vec<_>>(),[0x103,0,0,0]);
            if !self.no_reply{self.put(at,1,0);self.put(R_AX_C2HREG_CTRL,1,1);
                for(i,value)in self.response.into_iter().enumerate(){self.put(RESPONSE+i*4,4,value);}}
        }
        if at==R_AX_UDM1+1&&self.writes.iter().any(|w|w.0==R_AX_C2HREG_CTRL){
            if let Some(now)=self.late_counter.take(){self.now=now;}
        }
        if self.fail_write==Some(self.writes.len())||self.fail_cleanup&&(at==fwcmd::DMA_STOP||at==R_AX_PLATFORM_ENABLE){
            return Err(Fault::new(FaultKind::Access,at,v));}Ok(())
    }
}
impl RegisterIo for Device{
    fn read8(&mut self,a:usize)->io::Result<u8>{Ok(self.get(a,1)as u8)}
    fn read16(&mut self,a:usize)->io::Result<u16>{Ok(self.get(a,2)as u16)}
    fn read32(&mut self,a:usize)->io::Result<u32>{Ok(self.get(a,4))}
    fn write8(&mut self,a:usize,v:u8)->io::Result<()>{self.write(a,1,v as u32)}
    fn write16(&mut self,a:usize,v:u16)->io::Result<()>{self.write(a,2,v as u32)}
    fn write32(&mut self,a:usize,v:u32)->io::Result<()>{self.write(a,4,v)}
    fn now_us(&mut self)->u64{self.now}
    fn delay_us(&mut self,us:u32)->io::Result<()>{if !self.frozen{self.now+=u64::from(us);}Ok(())}
}
impl Bus for Device{fn pci_command(&mut self)->io::Result<u16>{Ok(self.command)}}
fn board()->Board{Board{identity:crate::board::Identity{cut:1,rfe:1,mac:[2;6],xtal:1,country:*b"US",channel_plan:1},
    maps:crate::efuse::Maps{logical:[0xff;2048],dav_logical:[0xff;16],phycap:[0xff;128]}}}
fn query_default(d:&mut Device,q:&mut Queue<'_>,ready:&download::Ready)->Result<Capabilities,Failure>{
    query(d,q,&board(),runtime_mac::BAR_BYTES,ready,MAX_TIMEOUT_US)
}

#[test]
fn actual_exchange_returns_raw_session_bound_capabilities_without_enabling_radio(){
    download::tests::with_downloaded_queue(|ready,q|{
        let mut d=Device::new(q);let cap=query_default(&mut d,q,&ready).unwrap();
        assert_eq!(cap.raw(),d.response);assert_eq!(cap.session(),ready.session());
        assert!(cap.belongs_to(&ready));
        assert_eq!((cap.tx_nss(),cap.rx_nss(),cap.antenna_tx(),cap.antenna_rx(),cap.effective_rx_path()),(2,2,0,0,3));
        assert!(!cap.tx_nss_defaulted()&&!cap.rx_nss_defaulted()&&!cap.tx_path_diversity());
        assert_eq!(d.writes,[(R_AX_H2CREG_DATA0,4,0x103),(R_AX_H2CREG_DATA0+4,4,0),(R_AX_H2CREG_DATA0+8,4,0),
            (R_AX_H2CREG_DATA0+12,4,0),(R_AX_UDM1+1,1,1),(R_AX_H2CREG_CTRL,1,1),(R_AX_C2HREG_CTRL,1,0),(R_AX_UDM1+1,1,17)]);
        assert_eq!(q.state(),fwcmd::State::Running);
    });
}
#[test]
fn source_resolution_uses_only_successful_reply_and_marks_zero_nss_fallback(){
    for(tx,rx,ta,ra,tn,rn,at,ar,div)in[(0,0,0,0,2,2,0,0,false),(1,1,1,1,1,1,2,2,false),
        (1,2,2,2,1,2,2,0,true),(2,1,2,1,2,1,0,2,false),(255,3,0,0,2,2,0,0,false)]{
        download::tests::with_downloaded_queue(|ready,q|{
            let mut d=Device::new(q);d.response=[0x403|(rx as u32)<<16,tx as u32,0,(ta as u32)<<8|(ra as u32)<<16];
            let cap=query_default(&mut d,q,&ready).unwrap();
            assert_eq!((cap.raw_tx_nss(),cap.raw_rx_nss(),cap.raw_tx_antennas(),cap.raw_rx_antennas()),(tx,rx,ta,ra));
            assert_eq!((cap.tx_nss(),cap.rx_nss(),cap.antenna_tx(),cap.antenna_rx(),cap.tx_path_diversity()),(tn,rn,at,ar,div));
            assert_eq!((cap.tx_nss_defaulted(),cap.rx_nss_defaulted()),(tx==0,rx==0));
        });
    }
}
#[test]
fn shared_mailbox_counter_nibbles_roll_over_without_losing_each_other(){
    for initial in [0x0f,0xf0,0xff,0xa5]{download::tests::with_downloaded_queue(|ready,q|{
        let mut d=Device::new(q);d.put(R_AX_UDM1+1,1,initial);query_default(&mut d,q,&ready).unwrap();
        assert_eq!(d.get(R_AX_UDM1+1,1),((initial+1)&15)|((((initial>>4)+1)&15)<<4));
    });}
}
#[test]
fn foreign_ready_at_same_dma_address_is_rejected_before_device_access(){
    let mut old=None;download::tests::with_downloaded_queue(|ready,_|old=Some(ready));
    download::tests::with_downloaded_queue(|_,q|{
        let mut d=Device::new(q);let error=query_default(&mut d,q,old.as_ref().unwrap()).unwrap_err();
        assert_eq!(error.stage,Stage::Guard);assert!(error.command_stop.is_none());assert_eq!(d.reads,0);assert!(d.writes.is_empty());
    });
}
#[test]
fn live_guard_rejects_wrong_firmware_power_queue_or_budget_without_writes(){
    for case in 0..9{download::tests::with_downloaded_queue(|ready,q|{
        let mut d=Device::new(q);let mut b=board();let mut size=runtime_mac::BAR_BYTES;let mut budget=MAX_TIMEOUT_US;
        match case{0=>d.command=0xffff,1=>d.put(download::FW_CONTROL,4,0),2=>d.put(R_AX_SYS_FUNC_EN,1,3),
            3=>b.identity.rfe=2,4=>size-=1,5=>budget=0,6=>budget+=1,7=>d.late_guard=Some(100+u64::from(budget)),_=>d.late_guard=Some(99)}
        let error=query(&mut d,q,&b,size,&ready,budget).unwrap_err();assert_eq!(error.stage,Stage::Guard);
        assert!(error.command_stop.is_none());assert!(d.writes.is_empty());
    });}
}
#[test]
fn occupied_mailbox_is_not_overwritten_and_uncertain_exchange_is_not_retried(){
    for at in [R_AX_H2CREG_CTRL,R_AX_C2HREG_CTRL]{download::tests::with_downloaded_queue(|ready,q|{
        let mut d=Device::new(q);d.put(at,1,1);let error=query_default(&mut d,q,&ready).unwrap_err();
        assert_eq!(error.stage,Stage::Mailbox);assert_eq!(q.state(),fwcmd::State::Faulted);
        assert!(!d.writes.iter().any(|w|(R_AX_H2CREG_DATA0..=R_AX_C2HREG_CTRL).contains(&w.0)));
        let count=d.writes.len();assert_eq!(query_default(&mut d,q,&ready).unwrap_err().stage,Stage::Guard);assert_eq!(d.writes.len(),count);
    });}
}
#[test]
fn malformed_function_length_or_all_ones_never_returns_capabilities(){
    for raw in [0x404,0x003,0x103,0x203,0x303,0x503,u32::MAX]{download::tests::with_downloaded_queue(|ready,q|{
        let mut d=Device::new(q);d.response[0]=raw;let error=query_default(&mut d,q,&ready).unwrap_err();
        assert!(matches!(error.stage,Stage::Mailbox|Stage::Decode));assert!(error.command_stop.is_some());assert_eq!(q.state(),fwcmd::State::Faulted);
    });}
}
#[test]
fn missing_response_and_frozen_clock_have_finite_bounds_and_one_trigger(){
    for frozen in [false,true]{download::tests::with_downloaded_queue(|ready,q|{
        let mut d=Device::new(q);d.no_reply=true;d.frozen=frozen;let error=query_default(&mut d,q,&ready).unwrap_err();
        assert_eq!(error.cause.kind,FaultKind::Timeout);assert_eq!(q.state(),fwcmd::State::Faulted);
        assert_eq!(d.writes.iter().filter(|w|w.0==R_AX_H2CREG_CTRL).count(),1);assert!(d.reads<1_000_100);
    });}
}
#[test]
fn final_counter_clock_overrun_or_regression_prevents_success(){
    for now in [100+u64::from(MAX_TIMEOUT_US),99]{download::tests::with_downloaded_queue(|ready,q|{
        let mut d=Device::new(q);d.late_counter=Some(now);let error=query_default(&mut d,q,&ready).unwrap_err();
        assert_eq!(error.cause.kind,if now==99{FaultKind::Clock}else{FaultKind::Timeout});assert!(error.command_stop.is_some());
    });}
}
#[test]
fn every_applied_exchange_write_error_retains_primary_and_independent_shutdown_errors(){
    for at in 1..=8{download::tests::with_downloaded_queue(|ready,q|{
        let mut d=Device::new(q);d.fail_write=Some(at);d.fail_cleanup=true;let error=query_default(&mut d,q,&ready).unwrap_err();
        assert_eq!(error.cause.kind,FaultKind::Access);assert_eq!(error.stage,Stage::Mailbox);
        let stop=error.command_stop.unwrap();assert!(stop.dma.is_some()&&stop.cpu.is_some());assert_eq!(q.state(),fwcmd::State::Faulted);
        assert_eq!(d.writes.iter().filter(|w|w.0==R_AX_H2CREG_CTRL).count(),usize::from(at>=6));
    });}
}
