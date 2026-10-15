use super::*;
use phy::{dm,tail::{self,Subband}};
use phycap::Capabilities;

fn board()->board::Board{board::Board{identity:identity(),maps:efuse::Maps{logical:[255;2048],dav_logical:[255;16],phycap:[255;128]}}}
fn with_caps(rx:u8,ant:u8,run:impl FnOnce(Capabilities)){
    phycap::tests::with_capabilities([0x403|(rx as u32)<<16,2,0,(ant as u32)<<16],run);
}
pub(super) fn prepared(p:&mut Phy<Device>,cap:&Capabilities,band:dack::Band)->dm::Prefix{
    p.baseband().unwrap();p.radio().unwrap();
    let d=p.bus_between_stages();d.session=cap.session();d.put(0x149b0,4,0xe1);d.put(0x14a00,4,0x7f);
    let prefix=dm::run(p,band).unwrap();p.nctl().unwrap();p.rck().unwrap();dack_cases::prepare_results(p);
    p.dack(band).unwrap();p.rxdck(band).unwrap();
    let d=p.bus_between_stages();d.events.clear();d.rf[0][0x43]=0xabcde;d.rf[1][0x43]=0x12345;
    d.rf[0][0x60]=0x23456;d.rf[1][0x60]=0x34567;
    for at in [0xd200,0x15804,0x17804,0x15808,0x17808,0x149c4,0x149c0,0x10d18,0x10d80,
        0x14acc,0x14ad8,0x14694,0x14778,0x149b0,0x14a00,0x123ac,0x10980,0x14420,0x10c6c,
        0x112ac,0x132ac,0x112b0,0x132b0,0x109a4]{d.put(at,4,0xa5a5a5a5);}
    prefix
}
fn off(d:&Device){
    assert_eq!(d.get(2,1)&3,0);assert_eq!(d.get(0x2f0,4)&(1<<17),0);
    assert_eq!(d.xtal[0x80]&7,0);assert_eq!(d.xtal[0x81]&7,0);
    assert_eq!(d.get(0x1010,4),0xb0f03);assert_eq!(d.get(0xc348,4),0);assert_eq!(d.command,6);
}

#[test]
fn tail_orders_source_power_trim_and_receive_configuration_without_packet_tx(){
    with_caps(2,2,|cap|{
        let mut d=Device::new();let mut p=Phy::new(&mut d,identity());let prefix=prepared(&mut p,&cap,dack::Band::Ghz2);
        let mut b=board();b.maps.phycap[0x5df-0x580]=0x12;b.maps.phycap[0x5de-0x580]=0x34;
        let done=tail::run(&mut p,&b,&prefix,&cap,Subband::Ghz2).unwrap();assert_eq!(p.state(),State::PathsConfigured);
        assert_eq!((done.attempt,done.session,done.rx_path,done.rx_nss),(p.attempt(),cap.session(),3,2));
        let d=p.bus_between_stages();let writes:Vec<_>=d.events.iter().filter_map(|e|if let Event::Write(a,w,v)=e{Some((*a,*w,*v))}else{None}).collect();
        assert_eq!(writes[0],(0xd200,4,0xa00001a5));
        assert_eq!(&writes[1..5],&[(0x15804,4,0xa2b27000),(0x17804,4,0xa2b27000),(0x15808,4,0xa2b27000),(0x17808,4,0xa2b27000)]);
        let order=[0xd200,0x15804,0x17804,0x15808,0x17808,0x10370,0x149c4,0x14738,0x178dc,0x112ac,0x109a4];
        let pos:Vec<_>=order.iter().map(|at|writes.iter().position(|w|w.0==*at).unwrap()).collect();assert!(pos.windows(2).all(|v|v[0]<v[1]));
        assert_eq!(d.rf[0][0x43],0x9bcde);assert_eq!(d.rf[1][0x43],0xf2345);
        assert_eq!(d.rf[0][0x60],0x34456);assert_eq!(d.rf[1][0x60],0xff567);
        assert_eq!(d.get(0x1010,4),0xb0f03);assert_eq!(d.get(0xc348,4),0);assert_eq!(d.get(0x109a4,4)&0x1c,0);
        assert!(d.events.iter().all(|e|matches!(e,Event::Write(_,_,_))));
        let count=d.events.len();assert!(tail::run(&mut p,&b,&prefix,&cap,Subband::Ghz2).is_err());assert_eq!(p.bus_between_stages().events.len(),count);
    });
}

#[test]
fn erased_trims_leave_radio_calibration_registers_untouched(){
    with_caps(2,2,|cap|{
        let mut d=Device::new();let mut p=Phy::new(&mut d,identity());let prefix=prepared(&mut p,&cap,dack::Band::Ghz2);
        tail::run(&mut p,&board(),&prefix,&cap,Subband::Ghz2).unwrap();let d=p.bus_between_stages();
        assert_eq!((d.rf[0][0x43],d.rf[1][0x43],d.rf[0][0x60],d.rf[1][0x60]),(0xabcde,0x12345,0x23456,0x34567));
        assert!(!d.events.iter().any(|e|matches!(e,Event::Write(0x10370,_,_))));
        for at in [0x14acc,0x14ad8,0x14694,0x149b0,0x14a00,0x123ac]{assert_eq!(d.get(at,4),0xa5a5a5a5);}
    });
}

#[test]
fn actual_reply_controls_rx_paths_stream_limits_and_explicit_zero_fallback(){
    for(rx,ant,path,nss)in[(0,0,3,2),(1,2,3,1),(2,2,3,2),(2,1,2,1)]{with_caps(rx,ant,|cap|{
        let mut d=Device::new();let mut p=Phy::new(&mut d,identity());let prefix=prepared(&mut p,&cap,dack::Band::Ghz2);
        let done=tail::run(&mut p,&board(),&prefix,&cap,Subband::Ghz2).unwrap();let d=p.bus_between_stages();
        assert_eq!((done.rx_path,done.rx_nss),(path,nss));assert_eq!(d.get(0x149c4,4)&15,path as u32);
        assert_eq!((d.get(0x149c0,4)>>14)&15,path as u32);assert_eq!((d.get(0x149c0,4)>>18)&15,path as u32);
        assert_eq!((d.get(0x10d18,4)>>8)&3,(nss-1)as u32);assert_eq!((d.get(0x10d18,4)>>21)&3,(nss-1)as u32);
        assert_eq!((d.get(0x10d80,4)>>6)&255,4);assert_eq!((d.get(0x10d80,4)>>14)&7,(nss-1)as u32);assert_eq!((d.get(0x10d80,4)>>23)&7,(nss-1)as u32);
        assert_eq!(d.get(0x112ac,4),if path==2{0x11111115}else{0x12333125});assert_eq!(d.get(0x132ac,4),0x12333125);
        assert_eq!(d.get(0x112b0,4)&0xfff,if path==2{0x111}else{0x333});assert_eq!(d.get(0x132b0,4)&0xfff,0x333);
        assert_eq!(d.get(0x178dc,4)>>30,3);assert_eq!(d.get(0x149c4,4)&(1<<14),1<<14);
    });}
}

#[test]
fn gain_offsets_keep_signed_baselines_global_trim_and_path_b_selection(){
    for(antenna,cck,rpl0,rpl1)in[(2,48,0x61,0x7f),(1,0xc0,0x31,0x7f)]{with_caps(2,antenna,|cap|{
        let mut d=Device::new();let mut p=Phy::new(&mut d,identity());let prefix=prepared(&mut p,&cap,dack::Band::Ghz2);
        let mut b=board();b.maps.logical[0x2d4]=0x8b;b.maps.logical[0x2d6]=0x87;b.maps.phycap[0x5bb-0x580]=0x8;b.maps.phycap[0x590-0x580]=7;
        tail::run(&mut p,&b,&prefix,&cap,Subband::Ghz2).unwrap();let d=p.bus_between_stages();
        assert_eq!(d.get(0x14acc,4),0xa5a5a5e0);assert_eq!(d.get(0x14ad8,4),0xa5a5a51c);
        assert_eq!((d.get(0x14694,4)>>16)&255,0xe8);assert_eq!((d.get(0x14778,4)>>16)&255,0xf4);
        assert_eq!(d.get(0x149b0,4)&255,rpl0);assert_eq!(d.get(0x14a00,4)&255,rpl1);assert_eq!(d.get(0x123ac,4)&127,cck&127);
    });}
}

#[test]
fn each_five_ghz_subband_selects_correct_gain_and_disables_btg(){
    for(subband,gain,comp)in[(Subband::Ghz5Low,0x2d8,0x5ba),(Subband::Ghz5Middle,0x2da,0x5b9),(Subband::Ghz5High,0x2dc,0x5b8)]{with_caps(2,2,|cap|{
        let mut d=Device::new();let mut p=Phy::new(&mut d,identity());let prefix=prepared(&mut p,&cap,dack::Band::Ghz5);
        let mut b=board();b.maps.logical[gain]=0x31;b.maps.phycap[comp-0x580]=5;
        tail::run(&mut p,&b,&prefix,&cap,subband).unwrap();let d=p.bus_between_stages();
        assert_eq!(d.get(0x14acc,4)&255,20);assert_eq!(d.get(0x149b0,4)&255,0xb1);assert_eq!(d.get(0x14a00,4)&255,79);
        assert_eq!(d.get(0x123ac,4),0xa5a5a5a5);assert_eq!(d.get(0x149c4,4)&(1<<14),0);
        assert_eq!((d.get(0x10980,4)>>17)&15,12);assert_eq!(d.get(0x10c6c,4)&(1<<21),0);
    });}
}

#[test]
fn foreign_capability_session_board_band_and_prefix_refuse_before_any_write(){
    for case in 0..5{with_caps(2,2,|cap|{
        let mut d=Device::new();let mut p=Phy::new(&mut d,identity());let prefix=prepared(&mut p,&cap,dack::Band::Ghz2);let mut b=board();
        let band=if case==3{Subband::Ghz5Low}else{Subband::Ghz2};
        match case{0=>p.bus_between_stages().session=0,1=>p.bus_between_stages().session=cap.session()+1,2=>b.identity.mac[5]^=1,4=>p.mark_failed(),_=>{}}
        let e=tail::run(&mut p,&b,&prefix,&cap,band).unwrap_err();assert_eq!(e.stage,tail::Stage::Guard);assert!(p.bus_between_stages().events.is_empty());
    });}
    with_caps(2,2,|cap|{
        let mut old=Device::new();let old_prefix={let mut p=Phy::new(&mut old,identity());prepared(&mut p,&cap,dack::Band::Ghz2)};
        let mut d=Device::new();let mut p=Phy::new(&mut d,identity());let _prefix=prepared(&mut p,&cap,dack::Band::Ghz2);
        assert_eq!(tail::run(&mut p,&board(),&old_prefix,&cap,Subband::Ghz2).unwrap_err().stage,tail::Stage::Guard);
        assert!(p.bus_between_stages().events.is_empty());
    });
}

#[test]
fn changed_post_dack_phase_and_live_tx_cannot_start_tail(){
    for(at,value)in[(0x8040,14),(0xc348,1),(0xc6ec,1<<7),(0x1010,0),(0x101c,1<<8)]{with_caps(2,2,|cap|{
        let mut d=Device::new();let mut p=Phy::new(&mut d,identity());let prefix=prepared(&mut p,&cap,dack::Band::Ghz2);p.bus_between_stages().put(at,4,value);
        assert_eq!(tail::run(&mut p,&board(),&prefix,&cap,Subband::Ghz2).unwrap_err().stage,tail::Stage::Guard);
        assert!(p.bus_between_stages().events.is_empty());assert_eq!(p.state(),State::Failed);
    });}
}

#[test]
fn every_applied_tail_write_error_stops_radio_and_preserves_dma(){
    with_caps(2,2,|cap|{
        let mut b=board();b.maps.phycap[0x5df-0x580]=0x12;b.maps.phycap[0x5de-0x580]=0x34;b.maps.logical[0x2d4]=0x21;
        let count={let mut d=Device::new();let mut p=Phy::new(&mut d,identity());let prefix=prepared(&mut p,&cap,dack::Band::Ghz2);
            tail::run(&mut p,&b,&prefix,&cap,Subband::Ghz2).unwrap();p.bus_between_stages().events.len()};
        for index in 1..=count{
            let mut d=Device::new();let mut p=Phy::new(&mut d,identity());let prefix=prepared(&mut p,&cap,dack::Band::Ghz2);
            let d=p.bus_between_stages();d.fail_write=Some(d.writes+index);
            let e=tail::run(&mut p,&b,&prefix,&cap,Subband::Ghz2).unwrap_err();assert_eq!(e.cause.kind,FaultKind::Access,"write{index}");
            assert_eq!(p.state(),State::Failed);off(p.bus_between_stages());
        }
    });
}

#[test]
fn ignored_reference_write_is_detected_and_cleanup_error_stays_separate(){
    with_caps(2,2,|cap|{
        let mut d=Device::new();let mut p=Phy::new(&mut d,identity());let prefix=prepared(&mut p,&cap,dack::Band::Ghz2);
        let d=p.bus_between_stages();d.ignore_write=Some(d.writes+2);d.fail_cleanup=Some(0x2f0);
        let e=tail::run(&mut p,&board(),&prefix,&cap,Subband::Ghz2).unwrap_err();assert_eq!(e.stage,tail::Stage::PowerReference);
        assert_eq!(e.cause.register,0x15804);assert!(e.radio_stop.is_some());assert_eq!(p.state(),State::Failed);
    });
}

#[test]
fn last_successful_readback_cannot_overrun_whole_stage_deadline(){
    with_caps(2,2,|cap|{
        let mut d=Device::new();let mut p=Phy::new(&mut d,identity());let prefix=prepared(&mut p,&cap,dack::Band::Ghz2);
        let d=p.bus_between_stages();d.late_read=Some(0x109a4);d.late_read_skip=1;d.late_read_us=100_000;
        let e=tail::run(&mut p,&board(),&prefix,&cap,Subband::Ghz2).unwrap_err();assert_eq!(e.cause.kind,FaultKind::Timeout);
        assert_eq!(p.state(),State::Failed);off(p.bus_between_stages());
    });
}
