use super::*;
use phy::dm;

fn prepared(p:&mut Phy<Device>){
    p.baseband().unwrap();p.radio().unwrap();let d=p.bus_between_stages();d.events.clear();
    d.rf[0][0x42]=0x12345;d.rf[1][0x42]=0x23456;
    for at in [0x10d7c,0x12d7c,0x10c00,0x10c28,0x10c2c,0x10c30,0x10c34,0x10c38,
        0x10738,0x14860,0x14b74,0x14b64,0x146e8,0x146ec,0x147a8,0x147ac,0x14494,0x14490,0xd248,0x10c70,0x14498]{d.put(at,4,0xa5a5a5a5);}
    d.put(0x149b0,4,0x123456e1);d.put(0x14a00,4,0x6543217f);
    for at in (0x1073c..=0x10774).step_by(4){d.put(at,4,0x000500f0);}
}
fn off(d:&Device){d.reset();assert_eq!(d.get(0x1010,4),0xb0f03);assert_eq!(d.get(0xc348,4),0);}

#[test]
fn dm_rejects_wrong_state_unknown_xtal_and_live_tx_without_writes(){
    let mut d=Device::new();let mut p=Phy::new(&mut d,identity());
    assert_eq!(dm::run(&mut p,dack::Band::Ghz2).unwrap_err().stage,dm::Stage::Guard);assert!(p.bus_between_stages().events.is_empty());
    let mut id=identity();id.xtal=255;let mut d=Device::new();let mut p=Phy::new(&mut d,id);prepared(&mut p);
    assert_eq!(dm::run(&mut p,dack::Band::Ghz2).unwrap_err().cause.kind,FaultKind::Xtal);assert!(p.bus_between_stages().events.is_empty());
    for (at,value) in [(0xc348,1),(0x1010,0xb0f00),(0x8040,15),(0x1000,0x2800)]{
        let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);p.bus_between_stages().put(at,4,value);
        assert_eq!(dm::run(&mut p,dack::Band::Ghz2).unwrap_err().stage,dm::Stage::Guard);
        assert!(p.bus_between_stages().events.is_empty());assert_eq!(p.state(),State::Failed);
    }
}
#[test]
fn dm_orders_the_contiguous_prefix_and_clears_all_128_macid_limits(){
    let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);let start=p.bus_between_stages().now;
    let result=dm::run(&mut p,dack::Band::Ghz2).unwrap();let d=p.bus_between_stages();
    let writes:Vec<_>=d.events.iter().map(|e|match e{Event::Write(a,w,v)=>(*a,*w,*v),_=>panic!("DM cannot send H2C or invent BTC readiness")}).collect();
    let radio:Vec<_>=writes.iter().filter(|(at,_,_)|*at==0x10370).map(|(_,_,v)|*v).collect();
    assert_eq!(radio,[0x84280000,0x84200000,0x84280000,0x94280000,0x94200000,0x94280000]);
    let limits:Vec<_>=writes.iter().filter(|(at,_,_)|(0xd36c..=0xd568).contains(at)).copied().collect();
    assert_eq!(limits.len(),128);for (i,&(at,width,value)) in limits.iter().enumerate(){assert_eq!((at,width,value),(0xd36c+4*i,4,0));}
    let order=[0x10370,0x10d7c,0xd36c,0x10c00,0x10738,0x14860,0x270,0x14494,0xd248,0x10c70];
    let positions:Vec<_>=order.iter().map(|at|writes.iter().position(|(a,_,_)|a==at).unwrap()).collect();
    assert!(positions.windows(2).all(|p|p[0]<p[1]));assert!(d.now-start>=400);
    assert_eq!((result.thermal,result.thermal_ewma_fixed4,result.last_thermal_max),([34,43],[544,688],43));
    assert_eq!((result.gain_offset,result.gain_rssi),(-31,127));
    assert_eq!(d.get(0x10d7c,4),0xa5a5a5a5&!2);assert_eq!(d.get(0x12d7c,4),0xa5a5a5a5&!2);
    assert_eq!(d.get(0x1010,4),0xb0f03);assert_eq!(d.get(0xc348,4),0);assert!(d.btc.is_empty());
}
#[test]
fn dm_ccx_bins_use_32us_units_and_preserve_enable_order(){
    let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);
    let r=dm::run(&mut p,dack::Band::Ghz2).unwrap();let d=p.bus_between_stages();
    assert_eq!((r.ccx_unit,r.ifs_low,r.ifs_high),(3,[0,3,9,33],[2,8,32,128]));
    assert_eq!(d.get(0x10c00,4),(0xa5a5a5a5&!0x77)|7);
    assert_eq!(d.get(0x10c28,4),0xa5a5a5a5|1<<12);
    for (i,value) in [0x00028000,0x00088003,0x00208009,0x00808021].iter().enumerate(){assert_eq!(d.get(0x10c2c+4*i,4),*value);}
}
#[test]
fn dm_physts_compresses_reserved_page_and_excludes_eht(){
    let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);dm::run(&mut p,dack::Band::Ghz2).unwrap();let d=p.bus_between_stages();
    for (at,expected) in [(0x10750,0x500f0),(0x10754,0x520f0),(0x10758,0x520f0),(0x1075c,0x500f0),
        (0x10760,0x520f2),(0x10764,0x50002),(0x10768,0x50000),(0x1076c,0x150000),(0x10770,0x150000),(0x10774,0x150000)]{assert_eq!(d.get(at,4),expected);}
    assert_eq!(d.events.iter().filter(|e|matches!(e,Event::Write(0x1073c..=0x10774,4,_))).count(),15);
    assert!(!d.events.iter().any(|e|matches!(e,Event::Write(0x10788,_,_))));
}
#[test]
fn dm_initial_dig_is_unlinked_and_does_not_program_unsupported_igi(){
    for (band,force) in [(dack::Band::Ghz2,false),(dack::Band::Ghz5,true)]{
        let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);let r=dm::run(&mut p,band).unwrap();let d=p.bus_between_stages();
        assert_eq!(r.dig.force_gaincode_index,force);assert_eq!(r.dig.gaincode,[6,1,31]);assert_eq!(r.dig.force_gaincode,[6,1,31]);
        assert_eq!(r.dig.false_alarm_thresholds,[196,352,440,528]);assert_eq!(r.dig.rssi_thresholds,[68,84,90,98,104]);
        assert_eq!(d.get(0x14860,4),0xa5a5a5a5&!(0x7c0|1<<30));assert_eq!(d.get(0x14b64,4),0x80a5a5a5);assert_eq!(d.get(0x14b74,4),0xa5a5a5a5&!(1<<30));
        for at in [0x146e8,0x146ec,0x147a8,0x147ac]{assert_eq!(d.get(at,4),0xa5a5a5a5&!(1<<5));}
        assert!(!d.events.iter().any(|e|matches!(e,Event::Write(0x146ac|0x14760,_,_))));
    }
}
#[test]
fn dm_crystal_uses_board_cap_and_exact_dcfo_edcca_masks(){
    for (raw,bounds) in [(0,[1,64]),(32,[1,96]),(126,[62,127]),(254,[62,127])]{
        let mut id=identity();id.xtal=raw;let mut d=Device::new();let mut p=Phy::new(&mut d,id);prepared(&mut p);let r=dm::run(&mut p,dack::Band::Ghz2).unwrap();let d=p.bus_between_stages();
        assert_eq!(r.cfo.bounds,bounds);assert_eq!(r.cfo.crystal,raw&127);assert_eq!(d.xtal[4],raw&127);assert_eq!(d.xtal[5],raw&127);
        assert_eq!(r.cfo.timer_ms,2000);assert!(!r.cfo.timer_enabled);assert_eq!(r.edcca_backup,[0;4]);assert_eq!(r.bandedge,0);assert!(r.dynamic_tb);
        assert_eq!(d.get(0x14494,4),0xa5a5a5a5|1<<29);assert_eq!(d.get(0x14490,4),0xa8a5a5a5);
        assert_eq!(d.get(0xd248,4),(0xa5a5a5a5&!7)|6);assert_eq!(d.get(0x10c70,4),(0xa5a5a5a5&!0x03f00000)|0x02900000);
    }
}
#[test]
fn dm_zero_thermal_is_not_replaced_by_a_fabricated_measurement(){
    let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);let d=p.bus_between_stages();d.rf[0][0x42]=0;d.rf[1][0x42]=0;
    let r=dm::run(&mut p,dack::Band::Ghz2).unwrap();assert_eq!(r.thermal,[0,0]);assert_eq!(r.thermal_ewma_fixed4,[0,0]);assert_eq!(r.last_thermal_max,0);
}
#[test]
fn dm_rejects_xtal_readback_mismatch_and_keeps_cleanup_failure_separate(){
    let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);let d=p.bus_between_stages();d.xtal_bad_read=true;d.fail_cleanup=Some(0x2f0);
    let e=dm::run(&mut p,dack::Band::Ghz2).unwrap_err();assert_eq!(e.stage,dm::Stage::Crystal);assert_eq!(e.cause.kind,FaultKind::Xtal);assert!(e.radio_stop.is_some());assert_eq!(p.state(),State::Failed);
}
#[test]
fn dm_never_replaces_an_outstanding_xtal_command_even_with_a_frozen_clock(){
    let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);let d=p.bus_between_stages();d.put(0x270,4,1<<31);d.freeze_read=Some(0x270);
    let e=dm::run(&mut p,dack::Band::Ghz2).unwrap_err();assert_eq!(e.stage,dm::Stage::Crystal);assert_eq!(e.cause.kind,FaultKind::Timeout);assert!(e.radio_stop.is_some());
    let d=p.bus_between_stages();assert!(!d.events.iter().any(|e|matches!(e,Event::Write(0x270,_,_))));assert!(d.reads<20_000);assert_eq!(d.command,6);
}
#[test]
fn every_applied_dm_write_error_poisoned_and_retains_dma(){
    let count={let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);dm::run(&mut p,dack::Band::Ghz2).unwrap();p.bus_between_stages().events.len()};
    for index in 1..=count{
        let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);let d=p.bus_between_stages();d.fail_write=Some(d.writes+index);
        let e=dm::run(&mut p,dack::Band::Ghz2).unwrap_err();assert_eq!(e.cause.kind,FaultKind::Access,"write{index}");assert_eq!(p.state(),State::Failed);
        let d=p.bus_between_stages();assert_eq!(d.command,6);assert_eq!(d.get(0x1010,4),0xb0f03);assert_eq!(d.get(2,1)&3,0);
        let n=d.events.len();assert!(dm::run(&mut p,dack::Band::Ghz2).is_err());assert_eq!(p.bus_between_stages().events.len(),n);
    }
}
#[test]
fn dm_whole_budget_includes_xtal_waits_and_late_final_read(){
    for last in [false,true]{
        let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);let d=p.bus_between_stages();
        if last{d.late_read=Some(0x14498);d.late_read_us=300_000;}else{d.deadline_write=Some(0x10d7c);d.deadline_us=300_000;}
        let e=dm::run(&mut p,dack::Band::Ghz2).unwrap_err();assert_eq!(e.cause.kind,FaultKind::Timeout);assert_eq!(e.stage,if last{dm::Stage::UlTb}else{dm::Stage::Baseband});off(p.bus_between_stages());
    }
}
#[test]
fn dm_xtal_source_ceiling_prevents_late_success(){
    let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);p.bus_between_stages().xtal_delay_us=50_000;
    let e=dm::run(&mut p,dack::Band::Ghz2).unwrap_err();assert_eq!(e.stage,dm::Stage::Crystal);assert_eq!(e.cause.kind,FaultKind::Timeout);assert_eq!(p.state(),State::Failed);
    assert_eq!(p.bus_between_stages().command,6);
}
