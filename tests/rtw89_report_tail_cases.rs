use super::*;
use crate::report_tail as tail;

fn active(d:&mut Device,q:&mut Queue<'_>,rings:&mut rx::Rings<'_>)->Receiving{
    let receiving=activate_inner(d,q,rings,&board(),runtime_mac::BAR_BYTES).unwrap();
    d.put(0x200,4,1<<17);d.put(0x8040,1,15);d.put(0xce40,4,0x114);
    d.put(0x9c18,4,0x5a5a5a5a);d.put(0xc614,4,0xffff1234);d.now=100;d.writes.clear();receiving
}

#[test]
fn exact_source_reports_and_two_halfword_rts_writes_leave_data_tx_stopped(){
    let mut d=Device::new();let mut c=Region::new(fwcmd::DMA_BYTES);let mut r=Region::new(rx::DMA_BYTES);
    let(mut q,mut rings)=prepared(&mut d,&mut c,&mut r);let receiving=active(&mut d,&mut q,&mut rings);
    let report=tail::configure(&mut d,&mut q,&mut rings,&receiving,None).unwrap();
    assert_eq!(d.writes,[(0xce40,4,0x2b),(0x9c18,4,0x5a5a5a59),(0xc614,2,0x234),(0xc614,2,0x2ff)]);
    assert_eq!(d.get(0xc614,4),0xffff02ff);assert_eq!(report.encoded_rts,0x2ff);
    assert_eq!((report.command_session,report.receive_session),(q.session(),rings.session()));
    assert_eq!(d.get(fwcmd::DMA_STOP,4),RECEIVING);assert_eq!(d.get(fwcmd::INIT_CONFIG,4)&HCI,HCI);
    assert_eq!(d.get(0xc348,4),0);stop(&mut d,&mut q,&mut rings);
}

#[test]
fn source_rts_sentinel_quantization_and_saturation_keep_legal_all_ones(){
    for (request,expected)in [(None,0x2ff),(Some(u32::MAX),0x2ff),(Some(0),0xff00),(Some(15),0xff00),
        (Some(16),0xff01),(Some(4079),0xfffe),(Some(4080),0xffff),(Some(u32::MAX-1),0xffff)]{
        let mut d=Device::new();let mut c=Region::new(fwcmd::DMA_BYTES);let mut r=Region::new(rx::DMA_BYTES);
        let(mut q,mut rings)=prepared(&mut d,&mut c,&mut r);let receiving=active(&mut d,&mut q,&mut rings);
        let report=tail::configure(&mut d,&mut q,&mut rings,&receiving,request).unwrap();
        assert_eq!(report.encoded_rts,expected);assert_eq!(d.get(0xc614,4),0xffff0000|expected as u32);
        stop(&mut d,&mut q,&mut rings);
    }
}

#[test]
fn unknown_phase_or_stale_hardware_refuses_before_report_writes(){
    for (at,width,value)in [(0x8040,1,14),(2,1,0),(0x200,4,0),(0x2f0,4,0),(0x1100,4,0x300000),
        (fwcmd::DMA_STOP,4,0xb0e00),(fwcmd::DMA_BUSY,4,1<<8),(0xc348,4,1),(download::FW_CONTROL,4,0)]{
        let mut d=Device::new();let mut c=Region::new(fwcmd::DMA_BYTES);let mut r=Region::new(rx::DMA_BYTES);
        let(mut q,mut rings)=prepared(&mut d,&mut c,&mut r);let receiving=active(&mut d,&mut q,&mut rings);d.put(at,width,value);
        let error=tail::configure(&mut d,&mut q,&mut rings,&receiving,None).unwrap_err();
        assert_eq!(error.stage,tail::Stage::Guard);assert!(d.writes.is_empty());assert!(error.command_stop.is_none());d.command=0;
    }
}

#[test]
fn old_receiving_token_cannot_configure_new_command_object_at_same_address(){
    let mut d=Device::new();let mut c=Region::new(fwcmd::DMA_BYTES);let mut r=Region::new(rx::DMA_BYTES);
    let(mut q,mut rings)=prepared(&mut d,&mut c,&mut r);let receiving=active(&mut d,&mut q,&mut rings);
    stop(&mut d,&mut q,&mut rings);drop(q);drop(rings);d=Device::new();
    let(mut q,mut rings)=prepared(&mut d,&mut c,&mut r);let _fresh=active(&mut d,&mut q,&mut rings);
    let error=tail::configure(&mut d,&mut q,&mut rings,&receiving,None).unwrap_err();
    assert_eq!(error.stage,tail::Stage::Guard);assert!(d.writes.is_empty());stop(&mut d,&mut q,&mut rings);
}

#[test]
fn every_applied_or_ignored_report_write_fails_and_stops_owned_dma(){
    for ignored in [false,true]{for index in 1..=4{
        let mut d=Device::new();let mut c=Region::new(fwcmd::DMA_BYTES);let mut r=Region::new(rx::DMA_BYTES);
        let(mut q,mut rings)=prepared(&mut d,&mut c,&mut r);let receiving=active(&mut d,&mut q,&mut rings);
        if ignored{d.ignore=Some(index);}else{d.fail=Some(index);}
        let error=tail::configure(&mut d,&mut q,&mut rings,&receiving,None).unwrap_err();
        assert_eq!(error.stage,match index{1=>tail::Stage::Ppdu,2=>tail::Stage::Forward,_=>tail::Stage::Rts});
        assert!(error.report_stop.is_none()&&error.radio_stop.is_none());assert_eq!(d.get(0xce40,4)&1,0);
        assert_eq!(q.state(),fwcmd::State::Faulted);assert_eq!(rings.state(),rx::State::Stopped);d.stopped();d.command=0;
    }}
}

struct Intercept<'a>{d:&'a mut Device,rts_reads:usize,final_time:Option<u64>,fail_report_stop:bool}
impl RegisterIo for Intercept<'_>{
    fn read8(&mut self,at:usize)->io::Result<u8>{self.d.read8(at)}
    fn read16(&mut self,at:usize)->io::Result<u16>{
        let result=self.d.read16(at);if at==0xc614{self.rts_reads+=1;if self.rts_reads==5{if let Some(time)=self.final_time{self.d.now=time;}}}result
    }
    fn read32(&mut self,at:usize)->io::Result<u32>{self.d.read32(at)}
    fn write8(&mut self,at:usize,v:u8)->io::Result<()>{self.d.write8(at,v)}
    fn write16(&mut self,at:usize,v:u16)->io::Result<()>{self.d.write16(at,v)}
    fn write32(&mut self,at:usize,v:u32)->io::Result<()>{
        self.d.write32(at,v)?;if at==0xce40&&v&1==0&&self.fail_report_stop{return Err(Fault::new(FaultKind::Access,at,v));}Ok(())
    }
    fn now_us(&mut self)->u64{self.d.now_us()}
    fn delay_us(&mut self,us:u32)->io::Result<()>{self.d.delay_us(us)}
}
impl Bus for Intercept<'_>{fn pci_command(&mut self)->io::Result<u16>{Ok(self.d.command)}}

#[test]
fn late_final_rts_read_and_clock_regression_never_return_configured(){
    for (time,kind)in [(10_100,FaultKind::Timeout),(99,FaultKind::Clock)]{
        let mut d=Device::new();let mut c=Region::new(fwcmd::DMA_BYTES);let mut r=Region::new(rx::DMA_BYTES);
        let(mut q,mut rings)=prepared(&mut d,&mut c,&mut r);let receiving=active(&mut d,&mut q,&mut rings);
        let mut bus=Intercept{d:&mut d,rts_reads:0,final_time:Some(time),fail_report_stop:false};
        let error=tail::configure(&mut bus,&mut q,&mut rings,&receiving,None).unwrap_err();
        assert_eq!(error.stage,tail::Stage::Verify);assert!(matches!(error.cause,Error::Register(f)if f.kind==kind));
        assert_eq!(d.get(0xce40,4)&1,0);d.stopped();d.command=0;
    }
}

#[test]
fn primary_report_failure_preserves_all_independent_cleanup_errors(){
    let mut d=Device::new();let mut c=Region::new(fwcmd::DMA_BYTES);let mut r=Region::new(rx::DMA_BYTES);
    let(mut q,mut rings)=prepared(&mut d,&mut c,&mut r);let receiving=active(&mut d,&mut q,&mut rings);
    d.fail=Some(1);d.fail_rx=true;d.fail_radio=true;d.fail_cpu=true;
    let mut bus=Intercept{d:&mut d,rts_reads:0,final_time:None,fail_report_stop:true};
    let error=tail::configure(&mut bus,&mut q,&mut rings,&receiving,None).unwrap_err();
    assert_eq!(error.stage,tail::Stage::Ppdu);assert!(matches!(error.cause,Error::Register(f)if f.register==0xce40));
    assert!(error.report_stop.is_some());assert!(error.receive_stop.unwrap().request.is_some());assert!(error.radio_stop.is_some());
    assert!(error.command_stop.unwrap().cpu.is_some());assert_eq!(q.state(),fwcmd::State::Faulted);assert_eq!(rings.state(),rx::State::Faulted);d.command=0;
}
