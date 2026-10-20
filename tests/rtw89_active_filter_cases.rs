use super::*;

const OWN:[u8;6]=[2,3,4,5,6,7];
const AP:[u8;6]=[2,9,8,7,6,5];
const OTHER:[u8;6]=[2,8,8,8,8,8];
fn received(ra:[u8;6],bssid:[u8;6],beacon:bool)->Vec<u8>{
    let mut f=frame();f[0]=if beacon{0x80}else{8};f[1]=if beacon{0}else{2};
    f[4..10].copy_from_slice(&ra);f[10..16].copy_from_slice(&bssid);f[16..22].copy_from_slice(&bssid);f
}
fn poll_all(data:&mut DataSession<'_,'_,'_,'_,'_,'_,Device>){
    for _ in 0..32{if data.poll_packets(100_000).unwrap()==[false,false]{return}}
    panic!("finite modeled receive batch did not drain");
}
fn word(bytes:&[u8],at:usize)->u32{u32::from_le_bytes(bytes[at..at+4].try_into().unwrap())}

#[test]
fn active_scan_to_bssid_uses_actual_cam_done_ack_and_preserves_source_filter_masks(){run(0,true,true,|data,c|{
    data.set_filter(false,AP,[0;6],100_000).unwrap();
    let c=c.borrow();assert_eq!(c.cam_packets.len(),1);let p=&c.cam_packets[0];
    assert_eq!((p[0]&3,p[0]>>2,p[1]),(1,6,0));assert_eq!(p.len(),68);
    assert_ne!(word(p,4)&(1<<15),0);
    let expected=[0,0x00400000,0x07010001,0,0x05040302,0x09020706,0x05060708,0,0,0x20000,0,0,0x80000,0x090200fd,0x05060708];
    for(i,wanted)in expected.into_iter().enumerate(){assert_eq!(word(p,8+4*i),wanted,"CAM word {i}");}
    assert_eq!(c.rx_filter,0x03164400);assert_eq!(c.selector,0x18888000);assert_eq!(c.filter_writes,1);
});}

#[test]
fn active_bssid_filter_delivers_selected_group_and_own_unicast_only(){run(0,true,true,|data,c|{
    data.set_filter(false,AP,[0;6],100_000).unwrap();
    c.borrow_mut().incoming=vec![received([255;6],AP,false),received([255;6],OTHER,false),received([255;6],OTHER,true),received(OWN,OTHER,false)];
    poll_all(data);assert_eq!(data.counts().filtered,2);assert_eq!(data.pending_packets(),2);
    let record=data.packet().unwrap();let first=crate::frames::from_packet(record.metadata().packet,record.buffer()).unwrap();
    assert_eq!(&first.mpdu()[4..10],&[255;6]);assert_eq!(&first.mpdu()[10..16],&AP);data.consume_packet();
    assert_eq!(&crate::frames::from_packet(data.packet().unwrap().metadata().packet,data.packet().unwrap().buffer()).unwrap().mpdu()[4..10],&OWN);
    data.consume_packet();command(data);
});}

#[test]
fn active_own_override_changes_cam_receive_and_transmitter_admission(){run(0,true,true,|data,c|{
    let own=[2,0x21,0x22,0x23,0x24,0x25];data.set_filter(false,AP,own,100_000).unwrap();
    assert_eq!(&c.borrow().cam_packets[0][24..30],&own);
    c.borrow_mut().incoming=vec![received(own,OTHER,false),received(OWN,OTHER,false)];poll_all(data);
    assert_eq!(data.pending_packets(),1);assert_eq!(data.counts().filtered,1);
    let failed=data.begin(&frame(),12,1,100_000).unwrap_err();assert!(failed.shutdown.is_none());assert_eq!(c.borrow().published,0);
    data.consume_packet();command(data);
});}

#[test]
fn active_reconfiguration_discards_banked_queued_and_inflight_previous_epoch(){run(0,true,true,|data,c|{
    c.borrow_mut().incoming=vec![received(OWN,OTHER,false),received(OWN,OTHER,false)];poll_all(data);assert_eq!(data.pending_packets(),2);
    {let mut c=c.borrow_mut();c.old=3;c.cam_wifi=2;}
    data.set_filter(false,AP,[0;6],100_000).unwrap();assert_eq!(data.pending_packets(),0);assert_eq!(data.counts().previous_epoch,7);
    c.borrow_mut().incoming=vec![received(OWN,OTHER,false)];poll_all(data);assert_eq!(data.pending_packets(),1);
});}

#[test]
fn active_busy_and_bad_timeout_refuse_before_cam_without_killing_current_tx(){run(0,true,true,|data,c|{
    assert!(data.set_filter(false,AP,[0;6],0).unwrap_err().shutdown.is_none());assert!(c.borrow().cam_packets.is_empty());
    data.begin(&frame(),12,1,100_000).unwrap();let failure=data.set_filter(false,AP,[0;6],100_000).unwrap_err();
    assert!(matches!(failure.cause,Cause::Tx(tx::Error::Busy)));assert!(failure.shutdown.is_none());assert!(c.borrow().cam_packets.is_empty());
    assert!(data.advance(100_000).unwrap().unwrap().acked);data.set_filter(false,AP,[0;6],100_000).unwrap();
});}

#[test]
fn active_cam_ack_without_dma_and_dma_without_ack_never_publish_filter(){for fault in[1,2]{run(0,true,true,|data,c|{
    c.borrow_mut().filter_fault=fault;let failure=data.set_filter(false,AP,[0;6],100_000).unwrap_err();
    assert_eq!(failure.stage,Stage::Filter(station::Stage::Cam));assert!(failure.shutdown.is_some());
    assert_eq!(c.borrow().cam_packets.len(),1);assert_eq!(c.borrow().filter_writes,0);
    assert!(data.poll_packets(100_000).is_err());
});}}

#[test]
fn active_corrupt_applied_failed_and_late_filter_writes_are_terminal(){for fault in[3,4,5]{run(0,true,true,|data,c|{
    c.borrow_mut().filter_fault=fault;let failure=data.set_filter(false,AP,[0;6],100_000).unwrap_err();
    assert_eq!(failure.stage,Stage::Filter(station::Stage::Filter));assert!(failure.shutdown.is_some());
    assert_eq!(c.borrow().cam_packets.len(),1);assert_eq!(c.borrow().filter_writes,1);assert_eq!(c.borrow().published,0);
    assert!(data.set_filter(true,[0;6],[0;6],100_000).is_err());
});}}

#[test]
fn active_selector_restore_failure_is_retained_after_cam(){run(0,true,true,|data,c|{
    c.borrow_mut().filter_fault=6;let failure=data.set_filter(false,AP,[0;6],100_000).unwrap_err();
    assert_eq!(failure.stage,Stage::Filter(station::Stage::Verify));assert!(failure.shutdown.is_some());
    let Cause::Station(check)=failure.cause else{panic!("lost station restore failure")};
    assert_eq!(check.selector_restore.unwrap().register,0xc04);
});}

#[test]
fn active_unknown_control_is_retained_and_blocks_epoch_change_before_cam(){run(0,true,true,|data,c|{
    c.borrow_mut().control_event=true;poll_all(data);assert_eq!(data.pending_packets(),1);
    let failure=data.set_filter(false,AP,[0;6],100_000).unwrap_err();assert!(failure.shutdown.is_some());
    assert!(c.borrow().cam_packets.is_empty());assert_eq!(data.pending_packets(),1);assert_eq!(data.packet().unwrap().bytes()[8],0xa5);
});}

#[test]
fn active_filter_can_return_to_scan_without_recreating_station_or_session(){run(0,true,true,|data,c|{
    data.set_filter(false,AP,[0;6],100_000).unwrap();data.set_filter(true,[0;6],[0;6],100_000).unwrap();
    assert_eq!(c.borrow().cam_packets.len(),2);
    c.borrow_mut().incoming=vec![received([255;6],OTHER,true),received([255;6],OTHER,false)];poll_all(data);
    assert_eq!(data.pending_packets(),1);assert_eq!(data.counts().filtered,1);command(data);
});}
