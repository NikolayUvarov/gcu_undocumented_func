use super::*;
const BT:u32=2|1<<30;
fn event(function:u8,body:&[u8])->Vec<u8>{let length=8+body.len();let mut bytes=vec![0;length];bytes[0]=2|18<<2;bytes[1]=function;bytes[4..8].copy_from_slice(&(length as u32).to_le_bytes());bytes[8..].copy_from_slice(body);bytes}
fn info(profile:u8,ignore:bool)->Vec<u8>{event(1,&[0,6,profile,0,0,u8::from(ignore)*8,0,0])}
fn function(c:&Control,id:u8)->bool{c.coex_packets.iter().any(|p|p[1]==id)}

#[test]
fn runtime_btinfo_burst_during_ack_keeps_every_event_and_packet_completion(){run(0,true,true,|data,c|{
    c.borrow_mut().bt=Some(BT);data.poll_packets(100_000).unwrap();
    data.begin(&frame(),12,1,100_000).unwrap();
    {let mut c=c.borrow_mut();c.burst=true;c.btc_on_command=(0..12).map(|n|info(if n&1==0{0x20}else{0x40},n==0)).collect();}
    command(data);assert_eq!(data.counts().btc_events,12);assert_eq!(data.counts().pressure,8);
    assert!(function(&c.borrow(),12));assert!(function(&c.borrow(),14));
    assert_eq!(data.advance(100_000).unwrap().unwrap().attempts,tx::AttemptCount::Known(1));
});}

#[test]
fn runtime_bluetooth_rfk_running_defers_antenna_then_retries_without_an_event(){run(0,true,true,|data,c|{
    c.borrow_mut().bt=Some(BT|32);data.poll_packets(100_000).unwrap();
    assert_eq!(c.borrow().lte_writes,0);assert!(data.counts().btc_deferred>0);assert_eq!(c.borrow().btg&(1<<19),1<<19);
    c.borrow_mut().bt=Some(BT);data.poll_packets(100_000).unwrap();assert!(c.borrow().lte_writes>0);assert_eq!(c.borrow().grants,0);
    command(data);
});}

#[test]
fn runtime_bt_request_does_not_claim_wlan_rfk_or_drop_the_connection(){run(0,true,true,|data,c|{
    c.borrow_mut().bt=Some(BT|64);data.poll_packets(100_000).unwrap();assert!(c.borrow().lte_writes>0);assert_eq!(c.borrow().grants,0);
    assert_eq!(data.counts().btc_deferred,0);data.begin(&frame(),12,1,100_000).unwrap();assert!(data.advance(100_000).unwrap().unwrap().acked);
});}

#[test]
fn runtime_continuous_valid_wifi_yields_without_an_empty_ring_requirement(){run(0,true,true,|data,c|{
    {let mut c=c.borrow_mut();c.bt=Some(BT);c.continuous_wifi=4096;c.old=1;}
    data.poll_packets(100_000).unwrap();assert!(c.borrow().continuous_wifi>0);assert!(data.pending_packets()>0);assert!(data.counts().pressure>0);
    command(data);assert!(c.borrow().continuous_wifi>0);
});}

#[test]
fn runtime_continuous_valid_bt_changes_defer_common_work_without_overflow(){run(0,true,true,|data,c|{
    {let mut c=c.borrow_mut();c.bt=Some(BT);c.continuous_bt=4096;c.btc_events.push(info(0x20,false));}
    data.poll_packets(100_000).unwrap();assert!(c.borrow().continuous_bt>0);assert!(data.counts().btc_events>4);assert!(data.counts().btc_deferred>0);
    command(data);assert!(c.borrow().continuous_bt>0);
});}

#[test]
fn runtime_scoreboard_enable_disable_applies_common_actions_without_btinfo(){run(0,true,true,|data,c|{
    c.borrow_mut().bt=Some(BT);data.poll_packets(100_000).unwrap();assert_eq!(c.borrow().btg&(1<<19),1<<19);
    c.borrow_mut().bt=Some(0);data.poll_packets(100_000).unwrap();assert_eq!(c.borrow().btg&(1<<19),0);assert_eq!(c.borrow().grants,0x77007700);
    assert!(function(&c.borrow(),0));command(data);
});}

#[test]
fn runtime_malformed_bt_during_ack_retains_the_record_and_exact_error(){run(0,true,true,|data,c|{
    let bad=event(1,&[0,5,0x20,0,0,0,0,0]);c.borrow_mut().btc_on_command.push(bad.clone());
    let failure=data.command(crate::h2c::Command{category:0,class:10,function:2},&[0;20],crate::h2c::Acks{received:true,done:true},100_000).unwrap_err();
    assert!(matches!(failure.cause,Cause::Coexistence(crate::btc_context::Error::Reports(_))));assert!(failure.shutdown.is_some());
    assert_eq!(data.terminal_packet().unwrap().bytes(),bad);
});}

#[test]
fn runtime_missing_common_command_ack_is_terminal_without_retry(){run(0,true,true,|data,c|{
    {let mut c=c.borrow_mut();c.bt=Some(BT);c.missing_coex_ack=true;}
    let failure=data.poll_packets(100_000).unwrap_err();assert!(failure.shutdown.is_some());
    let count=c.borrow().coex_packets.len();assert!(count>0);assert!(data.poll_packets(100_000).is_err());assert_eq!(c.borrow().coex_packets.len(),count);
});}

#[test]
fn runtime_raw_mac_channel_and_filter_are_actual_immutable_observations(){run(0,true,true,|data,_|{
    assert_eq!(data.channel(),6);assert_eq!(data.mac(),[2,3,4,5,6,7]);assert!(data.current_filter().all_beacons());
    data.set_filter(false,[2,9,8,7,6,5],[0;6],100_000).unwrap();assert!(!data.current_filter().all_beacons());
    assert_eq!(data.current_filter().own(),data.mac());
});}

#[test]
fn runtime_reenable_scoreboard_resets_old_info_before_later_info_and_version(){run(0,true,true,|data,c|{
    let mut version=[0;19];version[0]=8;version[1]=16;version[3]=1;version[11..15].copy_from_slice(&0x1234u32.to_le_bytes());
    {let mut c=c.borrow_mut();c.command_bt=Some(BT);c.btc_on_command=vec![event(2,&[]),info(0x20,true),event(0,&version)];}
    command(data);assert_eq!(data.counts().btc_events,3);assert!(function(&c.borrow(),12));assert!(function(&c.borrow(),14));
    assert_eq!(c.borrow().btg&(1<<19),1<<19);
    assert!(!c.borrow().coex_packets.iter().any(|p|p[1]==0&&p[8..]==[5,12,0,0,0,1,0,0,0]),"fresh non-ROM BT version was discarded after re-enable");
});}

#[test]
fn runtime_legitimate_fifteen_millisecond_common_ack_keeps_the_hundred_millisecond_budget(){run(0,true,true,|data,c|{
    {let mut c=c.borrow_mut();c.bt=Some(BT);c.delay_next_command=15_000;}
    data.poll_packets(100_000).unwrap();assert!(c.borrow().delayed_ack);command(data);
});}

#[test]
fn runtime_ppdu_status_burst_does_not_fill_control_inbox_or_steal_tx_ack(){run(0,true,true,|data,c|{
    data.begin(&frame(),12,1,100_000).unwrap();
    {let mut c=c.borrow_mut();c.ppdu_on_command=12;c.burst=true;c.btc_on_command=(0..6).map(|_|info(0,false)).collect();}
    command(data);assert_eq!(data.counts().ppdu_reports,12);assert_eq!(data.counts().btc_events,6);
    assert_eq!(data.pending_packets(),4);assert_eq!(data.counts().pressure,8);
    assert_eq!(data.advance(100_000).unwrap().unwrap().attempts,tx::AttemptCount::Known(1));
});}
