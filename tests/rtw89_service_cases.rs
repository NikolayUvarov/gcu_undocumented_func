use super::*;
use crate::{abi,codec::{self,Reader,Wire,Writer},service::{Service,Fatal},service_ipc::ipc,wlanhw,wire};

fn body(frame:&[u8],rate:u8,attempts:u8)->Vec<u8>{let mut out=vec![0;wlanhw::REQUEST_MAX];let mut w=Writer::new(&mut out);
    codec::encode_bytes::<2346>(frame,&mut w).unwrap();rate.encode(&mut w).unwrap();attempts.encode(&mut w).unwrap();let length=w.len();out.truncate(length);out}
fn filter(all:bool)->Vec<u8>{let mut out=vec![0;17];let mut w=Writer::new(&mut out);
    wlanhw::Filter{all_beacons:all,bssid:0x020908070605,own:0}.encode(&mut w).unwrap();out}
fn words(id:usize)->[usize;2]{ipc::response(id).unwrap().0}
fn error(id:usize,wanted:wlanhw::Error){assert_eq!(words(id),[wire::STATUS_ERROR,wanted as usize]);}
fn clean(){assert_eq!(ipc::outstanding(),0);assert_eq!(ipc::duplicates(),0);}
fn packet()->Vec<u8>{let mut frame=frame();frame[4..10].copy_from_slice(&[2,3,4,5,6,7]);frame}

#[test]
fn service_info_advertises_only_the_actual_admitted_2g20_channel(){run(0,true,true,|data,_|{
    ipc::reset();let mut service=Service::new(data).unwrap();let id=ipc::buffer(1,&[]);assert!(service.step());
    let(words,bytes)=ipc::response(id).unwrap();assert_eq!(words[0]&255,0);let mut r=Reader::new(&bytes[..words[0]>>16]);
    let info=wlanhw::Info::decode(&mut r).unwrap();assert!(r.done());assert_eq!(info.mac,0x020304050607);
    // bands 7 since 213-DRV-0087: 2.4 GHz, its OFDM, and 5 GHz; the radio's own channels include the 5 GHz ones.
    assert_eq!(info.bands,7);assert_eq!(info.supported.len(),41);assert_eq!(info.channels.as_slice(),&[6]);assert_eq!(info.channel,6);assert_eq!(info.frame_max,2346);
    assert_eq!(ipc::dropped(id),1);let stopped=service.stop();assert!(stopped.failure.is_none());clean();
});}

#[test]
fn service_same_channel_is_idempotent_and_other_channels_are_invalid(){run(0,true,true,|data,c|{
    ipc::reset();let mut service=Service::new(data).unwrap();
    for channel in[0,1,6,13,14,255]{let id=ipc::words(2,channel);assert!(service.step());
        if channel==6{assert_eq!(words(id),[0,0]);}else{error(id,wlanhw::Error::Invalid);}}
    assert_eq!(c.borrow().published,0);assert!(c.borrow().cam_packets.is_empty());drop(service);clean();
});}

#[test]
fn service_pending_send_keeps_its_buffer_cap_while_other_ipc_requests_progress(){run(0,true,true,|data,_|{
    ipc::reset();let mut service=Service::new(data).unwrap();let sent=ipc::buffer(4,&body(&frame(),12,3));
    assert!(service.step());assert!(ipc::response(sent).is_none());assert_eq!(ipc::dropped(sent),0);
    let info=ipc::buffer(1,&[]);assert!(service.step());let(w,b)=ipc::response(sent).unwrap();assert_eq!(w,[2<<16,0]);
    let mut r=Reader::new(&b[..2]);assert_eq!(wlanhw::Sent::decode(&mut r).unwrap(),wlanhw::Sent{acked:true,attempts:1});
    assert!(ipc::response(info).is_some());assert_eq!(ipc::dropped(sent),1);assert_eq!(ipc::dropped(info),1);
    assert_eq!(ipc::receives(),vec![(9,2),(10,2)]);drop(service);clean();
});}

#[test]
fn service_back_to_back_send_calls_rotate_actual_retained_receive_slots(){run(0,true,true,|data,c|{
    ipc::reset();let mut service=Service::new(data).unwrap();let first=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());
    let second=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());assert!(ipc::response(first).is_some());assert!(ipc::response(second).is_none());
    let info=ipc::buffer(1,&[]);assert!(service.step());assert!(ipc::response(second).is_some());assert!(ipc::response(info).is_some());
    assert_eq!(ipc::receives(),vec![(9,2),(10,2),(9,2)]);assert_eq!(c.borrow().published,2);drop(service);clean();
});}

#[test]
fn service_bad_rate_attempts_and_transmitter_return_invalid_without_publication(){run(0,true,true,|data,c|{
    ipc::reset();let mut service=Service::new(data).unwrap();let mut wrong=frame();wrong[10]^=2;
    for(f,r,a)in[(frame(),3,1),(frame(),12,0),(frame(),12,16),(wrong,12,1)]{
        let id=ipc::buffer(4,&body(&f,r,a));assert!(service.step());error(id,wlanhw::Error::Invalid);assert_eq!(ipc::dropped(id),1);
    }assert_eq!(c.borrow().published,0);drop(service);clean();
});}

#[test]
fn service_actual_frame_serialization_removes_fcs_and_uses_stable_channel_flags(){run(0,true,true,|data,c|{
    ipc::reset();let mut service=Service::new(data).unwrap();c.borrow_mut().incoming.push(packet());let id=ipc::buffer(5,&[]);assert!(service.step());
    let(w,b)=ipc::response(id).unwrap();let mut r=Reader::new(&b[..w[0]>>16]);let received=codec::decode_bytes::<2354>(&mut r).unwrap();assert!(r.done());
    assert_eq!(&received[..8],&[64,0,0,6,2,1,0,0]);assert_eq!(&received[8..],&packet());assert_eq!(ipc::dropped(id),1);
    let empty=ipc::buffer(5,&[]);assert!(service.step());error(empty,wlanhw::Error::Empty);drop(service);clean();
});}

#[test]
fn service_active_filter_changes_real_cam_and_clears_previous_serialized_epoch(){run(0,true,true,|data,c|{
    ipc::reset();let mut service=Service::new(data).unwrap();c.borrow_mut().incoming.push(packet());assert!(service.step());
    let change=ipc::buffer(3,&filter(false));assert!(service.step());assert_eq!(words(change),[0,0]);assert_eq!(c.borrow().cam_packets.len(),1);
    let empty=ipc::buffer(5,&[]);assert!(service.step());error(empty,wlanhw::Error::Empty);
    c.borrow_mut().incoming.push(packet());let next=ipc::buffer(5,&[]);assert!(service.step());let(w,b)=ipc::response(next).unwrap();
    let mut r=Reader::new(&b[..w[0]>>16]);let received=codec::decode_bytes::<2354>(&mut r).unwrap();assert_eq!(received[5],0);drop(service);clean();
});}

#[test]
fn service_waiters_are_bounded_expire_and_release_cancelled_saved_replies(){run(0,true,true,|data,c|{
    ipc::reset();let mut service=Service::new(data).unwrap();let mut waits=vec![];
    for _ in 0..5{let id=ipc::words(6,0);waits.push(id);assert!(service.step());}
    for id in &waits[..4]{assert!(ipc::response(*id).is_none());}assert_eq!(words(waits[4]),[0,0]);assert_eq!(ipc::outstanding(),4);
    ipc::cancel(waits[0]);c.borrow_mut().incoming.push(packet());assert!(service.step());assert!(ipc::response(waits[0]).is_none());
    for id in &waits[1..4]{assert_eq!(words(*id),[1<<16,0]);}assert_eq!(ipc::outstanding(),0);
    let take=ipc::buffer(5,&[]);assert!(service.step());assert_eq!(words(take)[0]&255,0);
    let expiring=ipc::words(6,0);assert!(service.step());ipc::advance(1001);assert!(service.step());assert_eq!(words(expiring),[0,0]);drop(service);clean();
});}

#[test]
fn service_missing_save_reply_capacity_does_not_start_tx_or_leak_calls(){run(0,true,true,|data,c|{
    ipc::reset();let mut service=Service::new(data).unwrap();ipc::save_failure(true);
    let sent=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());error(sent,wlanhw::Error::Busy);
    let wait=ipc::words(6,0);assert!(service.step());assert_eq!(words(wait),[0,0]);assert_eq!(c.borrow().published,0);drop(service);clean();
});}

#[test]
fn service_unsupported_ring_and_malformed_requests_drop_caps_exactly_once(){run(0,true,true,|data,_|{
    ipc::reset();let mut service=Service::new(data).unwrap();let ring=ipc::enqueue([7|ipc::MAJOR<<8,0],abi::CAP_KIND_MEMORY,&[],true);
    assert!(service.step());error(ring,wlanhw::Error::Invalid);assert_eq!(ipc::dropped(ring),1);
    let kick=ipc::words(8,0);assert!(service.step());error(kick,wlanhw::Error::Invalid);
    for words in[[1|(ipc::MAJOR+1)<<8,0],[1|(ipc::MAJOR-1)<<8,0],[99|ipc::MAJOR<<8,0],[2|ipc::MAJOR<<8|6<<16,0]]{
        let id=ipc::enqueue(words,abi::CAP_KIND_MEMORY,&[],true);assert!(service.step());assert!(self::words(id)[0]>=wire::STATUS_INVALID);assert_eq!(ipc::dropped(id),1);
    }
    let noncall=ipc::enqueue([7|ipc::MAJOR<<8,0],abi::CAP_KIND_MEMORY,&[],false);assert!(service.step());assert!(ipc::response(noncall).is_none());assert_eq!(ipc::dropped(noncall),1);
    drop(service);clean();
});}

#[test]
fn service_continuous_ipc_and_rx_each_progress_with_fixed_storage(){run(0,true,true,|data,c|{
    ipc::reset();let mut service=Service::new(data).unwrap();c.borrow_mut().incoming=vec![packet();20];
    for _ in 0..20{let id=ipc::buffer(1,&[]);assert!(service.step());assert!(ipc::response(id).is_some());}
    // Twenty frames against a buffer of sixteen: the last four wait in the ring, and none is dropped for want of room.
    for _ in 0..20{let id=ipc::buffer(5,&[]);assert!(service.step());assert_eq!(words(id)[0]&255,0);}
    assert_eq!(ipc::receives().len(),40);assert!(ipc::receives().iter().all(|(_,ms)|*ms==2));
    let empty=ipc::buffer(5,&[]);assert!(service.step());assert_ne!(words(empty)[0]&255,0,"a twenty-first frame");
    let stopped=service.stop();assert!(stopped.failure.is_none());assert_eq!(stopped.owner.counts().pressure,0);clean();
});}

#[test]
fn service_no_final_tx_report_times_out_answers_every_owner_and_retains_data(){run(7,true,true,|data,c|{
    ipc::reset();let mut service=Service::new(data).unwrap().keep_late(false);let wait=ipc::words(6,0);assert!(service.step());
    let send=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());let busy=ipc::buffer(4,&body(&frame(),12,1));
    assert!(service.step());error(busy,wlanhw::Error::Busy);ipc::advance(101);assert!(!service.step());
    error(send,wlanhw::Error::Failed);assert_eq!(words(wait),[0,0]);assert_eq!(c.borrow().published,1);
    // What the send saw: fetched and released, never reported by the firmware.
    let stopped=service.stop();let Some(Fatal::TransmitTimeout(seen))=stopped.failure else{panic!("no timeout")};
    assert_eq!((seen.finished,seen.fetched,seen.released,seen.reported,seen.tries),(0,true,true,false,0));
    assert!(stopped.owner.poll_packets(10_000).is_err());
    // The registers as the timeout found them, before the shutdown: the scheduler still the data mode's.
    let found=stopped.owner.snapshot().unwrap();assert_eq!(found.0[9]&0xffff,0x010f);clean();
});}

#[test]
fn service_unhandled_control_is_not_delivered_or_discarded_as_wifi(){run(0,true,true,|data,c|{
    ipc::reset();let mut service=Service::new(data).unwrap();c.borrow_mut().control_event=true;
    let info=ipc::buffer(1,&[]);assert!(!service.step());error(info,wlanhw::Error::NoDevice);
    let stopped=service.stop();assert!(matches!(stopped.failure,Some(Fatal::RetainedControl)));
    assert_eq!(stopped.owner.packet().unwrap().bytes()[8],0xa5);clean();
});}

#[test]
fn service_drop_releases_buffer_and_saved_replies_without_freeing_dma(){run(7,true,true,|data,_|{
    ipc::reset();let mut service=Service::new(data).unwrap();let wait=ipc::words(6,0);assert!(service.step());
    let send=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());drop(service);
    assert_eq!(words(wait),[0,0]);error(send,wlanhw::Error::Failed);assert_eq!(ipc::dropped(send),1);clean();
    assert!(data.advance(100_000).is_err());
});}

#[test]
fn service_refuses_occupied_receive_slot_before_serving(){run(0,true,true,|data,_|{
    ipc::reset();ipc::occupy(10);assert!(matches!(Service::new(data),Err(crate::sys::Error::NoSlot)));
    assert!(ipc::receives().is_empty());ipc::drop_cap(10).unwrap();clean();
});}

#[test]
fn service_progress_accepts_real_btc_ack_after_fifteen_milliseconds_then_serves_tx(){run(0,true,true,|data,c|{
    ipc::reset();let mut service=Service::new(data).unwrap();
    {let mut c=c.borrow_mut();c.bt=Some(2|1<<30);c.delay_next_command=15_000;}
    let info=ipc::buffer(1,&[]);assert!(service.step());assert_eq!(words(info)[0]&255,wire::STATUS_OK);
    assert!(c.borrow().delayed_ack);assert!(!c.borrow().coex_packets.is_empty());assert_eq!(ipc::dropped(info),1);
    let sent=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());assert!(ipc::response(sent).is_none());
    assert!(service.step());let(w,b)=ipc::response(sent).unwrap();assert_eq!(w,[2<<16,0]);
    let mut r=Reader::new(&b[..2]);assert_eq!(wlanhw::Sent::decode(&mut r).unwrap(),wlanhw::Sent{acked:true,attempts:1});
    let stopped=service.stop();assert!(stopped.failure.is_none());assert_eq!(ipc::dropped(sent),1);clean();
});}

fn country(code:&str,limits:&[(u8,u8,bool)])->Vec<u8>{
    let mut out=vec![0;wlanhw::REQUEST_MAX];let mut w=Writer::new(&mut out);
    codec::encode_str::<2>(code,&mut w).unwrap();2u8.encode(&mut w).unwrap();
    let list:Vec<_>=limits.iter().map(|&(channel,eirp,listen)|wlanhw::Limit{channel,eirp,listen}).collect();
    codec::encode_slice::<wlanhw::Limit,64>(&list,&mut w).unwrap();let length=w.len();out.truncate(length);out
}
fn channels(id:usize)->Vec<u8>{
    let(w,b)=ipc::response(id).unwrap();assert_eq!(w[0]&255,0);let mut r=Reader::new(&b[..w[0]>>16]);
    let info=wlanhw::Info::decode(&mut r).unwrap();assert!(r.done());info.channels.as_slice().to_vec()
}
fn sends(service:&mut Service<'_,'_,'_,'_,'_,'_,'_,Device>)->bool{
    let id=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());
    if ipc::response(id).is_some(){error(id,wlanhw::Error::Failed);return false}
    assert!(service.step());assert_eq!(words(id),[2<<16,0]);true
}

// 213-DRV-0032: channel 6 admitted at 20 dBm (40 half-dBm); the station's country may change while it is joined.
#[test]
fn service_keeps_its_channel_under_a_country_that_allows_its_admitted_power_and_sends_nothing_under_one_that_does_not(){run(0,true,true,|data,c|{
    use crate::receive_service::Event;
    ipc::reset();let mut service=Service::new(data).unwrap().under(40,1).strict(true);
    let all=|eirp|(1..=13).map(|ch|(ch,eirp,false)).collect::<Vec<_>>();
    let id=ipc::buffer(9,&country("UZ",&all(20)));assert!(service.step());assert_eq!(channels(id),[6]);
    assert_eq!(service.take_event(),Some(Event::DataCountry{code:*b"UZ",channel:6,kept:true,enforced:true}));assert!(sends(&mut service));
    // 10 dBm on 6 is less than it was admitted at; listening only there is no sending either.
    let mut listen=all(20);listen[5].2=true;
    for limits in[all(10),listen]{
        let id=ipc::buffer(9,&country("UZ",&limits));assert!(service.step());error(id,wlanhw::Error::Failed);
        assert_eq!(service.take_event(),Some(Event::DataCountry{code:*b"UZ",channel:6,kept:false,enforced:true}));
        assert!(!sends(&mut service),"nothing is sent under a country it cannot keep");
    }
    // A malformed request changes nothing; a country it can keep lets it send again: US caps 1-11 at 30 dBm.
    let id=ipc::buffer(9,&country("uz",&all(20)));assert!(service.step());error(id,wlanhw::Error::Invalid);
    assert_eq!(service.take_event(),None);assert!(!sends(&mut service));
    let us:Vec<_>=(1..=11).map(|ch|(ch,30,false)).collect();
    let id=ipc::buffer(9,&country("US",&us));assert!(service.step());assert_eq!(channels(id),[6]);
    assert_eq!(service.take_event(),Some(Event::DataCountry{code:*b"US",channel:6,kept:true,enforced:true}));assert!(sends(&mut service));
    assert_eq!(c.borrow().published,2);
    let stopped=service.stop();assert!(stopped.failure.is_none());clean();
});
    // The debug mode (KEEP_SENDING, the default until hardening): the refusal is logged, answered as applied, and sent on.
    run(0,true,true,|data,c|{
        use crate::receive_service::Event;
        ipc::reset();let mut service=Service::new(data).unwrap().under(40,1);
        let id=ipc::buffer(9,&country("UZ",&(1..=13).map(|ch|(ch,10,false)).collect::<Vec<_>>()));assert!(service.step());
        assert_eq!(channels(id),[6]);
        assert_eq!(service.take_event(),Some(Event::DataCountry{code:*b"UZ",channel:6,kept:false,enforced:false}));
        assert!(sends(&mut service));assert_eq!(c.borrow().published,1);drop(service);clean();
    });
    // A channel admitted under no known country keeps none.
    run(0,true,true,|data,_|{
        ipc::reset();let mut service=Service::new(data).unwrap();
        let id=ipc::buffer(9,&country("UZ",&(1..=13).map(|ch|(ch,20,false)).collect::<Vec<_>>()));assert!(service.step());
        error(id,wlanhw::Error::Failed);assert_eq!(service.take_event(),None);drop(service);clean();
    });
}

// 213-DRV-0032: the firmware's L0 recovery zeroes the scheduler 100 ms after admission (the Ryzen, 2026-11-08); the debug
// mode (OUTLAST_SER) waits for the firmware, then restores it, and the data mode goes on.
#[test]
fn service_outlasts_the_firmwares_recovery_of_the_scheduler(){
    for heal in[None,Some(2)]{run(0,true,true,|data,c|{
        ipc::reset();let mut service=Service::new(data).unwrap();
        {let mut c=c.borrow_mut();c.ser=true;c.ser_heal=heal;}
        let id=ipc::buffer(1,&[]);assert!(service.step());assert_eq!(channels(id),[6]);
        let recovery=service.take_recovery().unwrap();
        assert_eq!((recovery.count,recovery.ser,recovery.cmac,recovery.restored),(1,1,8,heal.is_none()));
        assert!(sends(&mut service),"the scheduler is back");assert_eq!(service.take_recovery(),None);
        let stopped=service.stop();assert!(stopped.failure.is_none());clean();
    });}
}

// OUTLAST_SER: a recovery longer than what is left of a pending frame's 100 ms moves the frame's time and the operation's
// by its wait and SER_GRACE_US (the Ryzen, 2026-11-08: the station's check ran out of the frame's time at the first traffic).
#[test]
fn service_a_pending_frame_outlasts_a_recovery_longer_than_its_time_left(){run(0,true,true,|data,c|{
    ipc::reset();let mut service=Service::new(data).unwrap();
    let id=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());assert!(ipc::response(id).is_none());
    {let mut c=c.borrow_mut();c.advance_us=60_000;c.ser=true;c.ser_heal=Some(100);}ipc::advance(60);
    assert!(service.step());assert_eq!(words(id),[2<<16,0]);
    let recovery=service.take_recovery().unwrap();assert!(!recovery.restored&&recovery.waited_us>=40_000,"{recovery:?}");
    assert!(sends(&mut service));let stopped=service.stop();assert!(stopped.failure.is_none());clean();
});}

// The service's own deadline for a frame moves by the same grace; past it, the frame still times out.
#[test]
fn service_a_frame_held_across_a_recovery_times_out_only_after_its_grace(){run(7,true,true,|data,c|{
    ipc::reset();let mut service=Service::new(data).unwrap().keep_late(false);
    let send=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());
    {let mut c=c.borrow_mut();c.ser=true;c.ser_heal=Some(100);}ipc::advance(80);assert!(service.step());
    assert!(service.take_recovery().is_some());ipc::advance(30);assert!(service.step(),"110 ms, inside the grace");
    assert!(ipc::response(send).is_none());ipc::advance(60);assert!(!service.step());error(send,wlanhw::Error::Failed);
    let stopped=service.stop();assert!(matches!(stopped.failure,Some(Fatal::TransmitTimeout(_))));clean();
});}

// No change is watched after the data mode's own shutdown: the BB and DMA it stopped are not the firmware's (the Ryzen's
// log of 2026-11-08 had read them as a firmware stop). Fault 12: the radio's power cycle reads as off.
#[test]
fn service_watches_no_change_after_the_data_modes_own_shutdown(){run(0,true,true,|data,c|{
    ipc::reset();let mut service=Service::new(data).unwrap();assert!(service.step());
    c.borrow_mut().fault=12;ipc::advance(10);assert!(!service.step(),"the next checks, 10 ms on");
    let stopped=service.stop();assert!(matches!(stopped.failure,Some(Fatal::Radio(_))));
    assert!(stopped.changes.0[8].is_none()&&stopped.changes.0[10].is_none(),"{:?}",stopped.changes);
    let found=stopped.owner.snapshot().unwrap();assert_eq!(found.0[8]>>16&3,3,"the BB on before the shutdown");clean();
});}

// DEBUG KEEP_LATE (the Ryzen, 2026-11-08): a send that outruns its time is answered as not acknowledged and the service
// goes on, sending nothing else until its release; past LATE_MS more it stops. Fault 7: no final report ever comes.
#[test]
fn service_a_late_send_is_answered_unacknowledged_and_its_release_awaited(){run(7,true,true,|data,c|{
    use crate::receive_service::Event;
    ipc::reset();let mut service=Service::new(data).unwrap();
    let send=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());assert!(ipc::response(send).is_none());
    ipc::advance(91);assert!(service.step(),"the service goes on");
    let(w,b)=ipc::response(send).unwrap();assert_eq!(w,[2<<16,0]);let mut r=Reader::new(&b[..2]);
    assert!(!wlanhw::Sent::decode(&mut r).unwrap().acked);
    let event=service.take_event();assert!(matches!(event,Some(Event::LateFrame{count:1,length:64,..})),"{event:?}");
    let busy=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());error(busy,wlanhw::Error::Busy);
    ipc::advance(1001);assert!(service.step(),"a frame the hardware still holds is waited for");
    ipc::advance(4000);assert!(!service.step());assert_eq!(c.borrow().published,1);
    let stopped=service.stop();assert!(matches!(stopped.failure,Some(Fatal::TransmitTimeout(_))));clean();
});}

// OUTLAST_SER: a recovery that begins after the checks and holds the radio off in the coexistence reconcile is outlasted,
// and the reconcile made again (the Ryzen, 2026-11-08: Coexistence(RadioOff) after two recoveries).
#[test]
fn service_outlasts_a_recovery_that_begins_in_the_coexistence_reconcile(){run(0,true,true,|data,c|{
    ipc::reset();let mut service=Service::new(data).unwrap();assert!(service.step());
    c.borrow_mut().bb_off=Some((2,3));ipc::advance(10);assert!(service.step(),"{:?}",service.take_recovery());
    let recovery=service.take_recovery().unwrap();assert!(recovery.after_reconcile&&!recovery.restored,"{recovery:?}");
    assert!(sends(&mut service));let stopped=service.stop();assert!(stopped.failure.is_none());clean();
});}

// OUTLAST_SER: a recovery that begins after the first check and zeroes the scheduler under the session's permit check is
// outlasted, and the checks made again (the Ryzen, 2026-11-08: Session(Permit(PowerState at 0xc348)) after 42 minutes).
#[test]
fn service_outlasts_a_recovery_that_begins_in_the_permit_check(){for skip in[2,3]{run(0,true,true,|data,c|{
    ipc::reset();let mut service=Service::new(data).unwrap();assert!(service.step());
    {let mut c=c.borrow_mut();c.ser=true;c.ser_skip=skip;c.ser_heal=Some(20);}
    let mut recovery=None;for _ in 0..3{ipc::advance(10);assert!(service.step(),"skip {skip}");recovery=recovery.or(service.take_recovery());}
    assert!(recovery.is_some_and(|r|!r.restored),"skip {skip}: {recovery:?}");
    assert!(sends(&mut service));let stopped=service.stop();assert!(stopped.failure.is_none());clean();
});}}

// One step drains what waits (DRAIN passes), not one descriptor: the Ryzen's packet buffer filled at one a step.
#[test]
fn service_one_step_takes_every_waiting_frame(){run(0,true,true,|data,c|{
    ipc::reset();let mut service=Service::new(data).unwrap();c.borrow_mut().incoming=vec![packet();3];
    let wait=ipc::words(6,0);assert!(service.step());assert_eq!(words(wait),[3<<16,0]);drop(service);clean();
});}

// 801-DRV-0071: a receive that gives no Wi-Fi frame for QUIET_MS stops the service with its line, so the driver restarts.
#[test]
fn service_a_receive_silent_for_ten_seconds_stops_with_its_line(){run(0,true,true,|data,c|{
    use crate::{receive_service::Event,service::{QUIET_MS,Stall}};
    ipc::reset();let mut service=Service::new(data).unwrap();
    // Each step's receive wait moves this clock 2 ms: 100 ms of margin either side.
    for _ in 0..3{c.borrow_mut().incoming.push(packet());ipc::advance(QUIET_MS-100);assert!(service.step(),"a frame each 9.9 s");}
    let sent=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());assert!(service.step());assert_eq!(words(sent)[0]&255,0);
    ipc::advance(QUIET_MS-100);assert!(service.step());assert!(service.take_event().is_none());
    ipc::advance(200);assert!(!service.step(),"10 s with no frame");
    let Some(Event::Stalled{channel:6,quiet_ms,frames:3,sent:1,acked:1})=service.take_event()else{panic!("no stall line")};
    assert!((QUIET_MS..QUIET_MS+120).contains(&quiet_ms));
    let stopped=service.stop();
    assert!(matches!(stopped.failure,Some(Fatal::ReceiveStalled(Stall{frames:3,sent:1,acked:1,..}))));
    assert!(stopped.owner.snapshot().is_some(),"the registers as the stall found them");clean();
});}

// A filter that lets no network in arms nothing: a station with no network hears nothing by its own choice.
#[test]
fn service_a_filter_with_no_network_does_not_watch_the_receive(){run(0,true,true,|data,_|{
    use crate::{receive_service::Event,service::QUIET_MS};
    ipc::reset();let mut service=Service::new(data).unwrap();
    let mut none=vec![0;17];let mut w=Writer::new(&mut none);wlanhw::Filter{all_beacons:false,bssid:0,own:0}.encode(&mut w).unwrap();
    let id=ipc::buffer(3,&none);assert!(service.step());assert_eq!(words(id),[0,0]);
    ipc::advance(3*QUIET_MS);assert!(service.step());assert!(service.take_event().is_none());
    // A scan's filter arms it from its own start.
    let id=ipc::buffer(3,&filter(true));assert!(service.step());assert_eq!(words(id),[0,0]);
    ipc::advance(QUIET_MS-100);assert!(service.step());ipc::advance(200);assert!(!service.step());
    assert!(matches!(service.take_event(),Some(Event::Stalled{frames:0,..})));drop(service);clean();
});}

// UNHEARD unicast sends in a row not acknowledged, over UNHEARD_MS, are logged once; the service goes on.
#[test]
fn service_a_run_of_unacknowledged_sends_is_logged_once(){run(0,true,true,|data,c|{
    use crate::{receive_service::Event,service::{UNHEARD,UNHEARD_MS}};
    ipc::reset();let mut service=Service::new(data).unwrap();c.borrow_mut().unacked=true;
    let gap=UNHEARD_MS/u64::from(UNHEARD-1);
    for n in 0..UNHEARD+4{
        if n>0{ipc::advance(gap);}c.borrow_mut().incoming.push(packet());
        let sent=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());assert!(service.step());
        let(w,b)=ipc::response(sent).unwrap();assert_eq!(w[0]&255,0);
        let mut r=Reader::new(&b[..2]);assert_eq!(wlanhw::Sent::decode(&mut r).unwrap(),wlanhw::Sent{acked:false,attempts:1});
        let take=ipc::buffer(5,&[]);assert!(service.step());let _=ipc::response(take);
        let event=service.take_event();
        if n+1==UNHEARD{assert!(matches!(event,Some(Event::Unheard{channel:6,count,ms,..})if count==UNHEARD&&ms>=UNHEARD_MS),"{event:?}");}
        else{assert!(event.is_none(),"send {n}: {event:?}");}
    }
    // An acknowledgement ends the run; the next run is logged again.
    c.borrow_mut().unacked=false;let sent=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());assert!(service.step());words(sent);
    let stopped=service.stop();assert!(stopped.failure.is_none());clean();
});}

// 213-DRV-0087: in data mode on a listen-only channel (100 here), sends go to the admitted access point alone while its
// beacon is heard: no probe request, nothing to another receiver, nothing after LISTEN_BEACON_MS of silence; no CCK rate.
#[test]
fn service_on_a_listen_only_channel_sends_to_the_heard_access_point_alone(){run_at(5500,0,true,true,|data,c|{
    ipc::reset();let now=ipc::clock()/1_000_000;let ap=[2,1,1,1,1,1];
    let mut service=Service::new(data).unwrap().listening_to(ap,now);
    let info=ipc::buffer(1,&[]);assert!(service.step());let(w,b)=ipc::response(info).unwrap();let mut r=Reader::new(&b[..w[0]>>16]);
    let info=wlanhw::Info::decode(&mut r).unwrap();assert_eq!((info.channel,info.bands),(100,7));
    let cck=ipc::buffer(4,&body(&frame(),2,1));assert!(service.step());error(cck,wlanhw::Error::Invalid);
    let mut probe=frame();probe[0]=0x40;probe[4..10].fill(0xff);
    let id=ipc::buffer(4,&body(&probe,12,1));assert!(service.step());error(id,wlanhw::Error::Failed);
    let mut other=frame();other[4..10].copy_from_slice(&[2,9,9,9,9,9]);
    let id=ipc::buffer(4,&body(&other,12,1));assert!(service.step());error(id,wlanhw::Error::Failed);
    assert_eq!(c.borrow().published,0,"nothing published for the refused sends");
    let sent=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());
    let info=ipc::buffer(1,&[]);assert!(service.step());assert_eq!(words(sent)[1],0,"to the access point: sent");ipc::response(info).unwrap();
    assert_eq!(c.borrow().published,1);
    // Its beacon unheard for longer than the station's silence: nothing more is sent there.
    ipc::advance(crate::service::LISTEN_BEACON_MS+1);
    let late=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());error(late,wlanhw::Error::Failed);
    assert_eq!(c.borrow().published,1);drop(service);clean();
});}

// 213-DRV-0087: a frame waits in the inbox for its PPDU status report while the drain goes on, and the station gets it
// with its signal; a report after the frame was handed on, or after another frame, is counted and put on none.
fn header(service:&mut Service<'_,'_,'_,'_,'_,'_,'_,Device>)->Vec<u8>{
    let id=ipc::buffer(5,&[]);assert!(service.step());let(w,b)=ipc::response(id).unwrap();let mut r=Reader::new(&b[..w[0]>>16]);
    let received=codec::decode_bytes::<2354>(&mut r).unwrap();assert!(r.done());received[..8].to_vec()
}
#[test]
fn service_a_frame_carries_the_signal_of_the_report_after_it(){run(0,true,true,|data,c|{
    use crate::{captures::{bytes,frame as buffer,FIVE},ppdu::Signals,receive_service::Event,service::QUIET_MS};
    ipc::reset();let mut service=Service::new(data).unwrap();
    let mut own=packet();own.extend_from_slice(&[0;4]);let report=bytes(FIVE);
    let mut foreign=packet();foreign[4..10].copy_from_slice(&[2,8,7,6,5,4]);foreign.extend_from_slice(&[0;4]);
    c.borrow_mut().buffers=vec![buffer(&own,1,4),report.clone()];
    let first=header(&mut service);assert_eq!((first[2],first[3],first[4]),(63,6,12),"-63 dBm, channel 6, 6 Mb/s");
    // Alone in the ring: handed on when the drain finds it empty; the report a step later is late.
    c.borrow_mut().buffers=vec![buffer(&own,1,4)];assert_eq!(header(&mut service)[2],0);
    c.borrow_mut().buffers=vec![report.clone()];assert!(service.step());
    // A frame the filter drops comes between: the report is that frame's.
    c.borrow_mut().buffers=vec![buffer(&own,1,4),buffer(&foreign,1,4),report.clone()];assert_eq!(header(&mut service)[2],0);
    // Logged once a minute while it changes, frames coming meanwhile.
    let mut logged=None;
    for n in 0..8u64{c.borrow_mut().incoming.push(packet());ipc::advance(QUIET_MS-100);assert!(service.step());
        if let Some(event)=service.take_event(){logged=Some((n,event));break}}
    let Some((n,Event::Signals{channel:6,kept,measured:1,late:1,unmatched:1}))=logged else{panic!("no signal line: {logged:?}")};
    assert_eq!(kept,3+n+1);
    let stopped=service.stop();assert!(stopped.failure.is_none());
    assert_eq!(stopped.owner.counts().signals,Signals{measured:1,late:1,unmatched:1});assert_eq!(stopped.owner.counts().filtered,1);clean();
});}

// 213-DRV-0090: a second with no beacon of the joined BSS, once one was heard, is a hole: logged when it opens and when
// its next beacon closes it, with the counts since and the registers; every minute a line of the traffic.
fn bss_beacon()->Vec<u8>{
    let mut f=vec![0x80,0,0,0,0xff,0xff,0xff,0xff,0xff,0xff,2,9,8,7,6,5,2,9,8,7,6,5,0x10,0];
    f.extend_from_slice(&[0;8]);f.extend_from_slice(&[100,0,0x11,4,0,3,b'n',b'e',b't']);f
}
#[test]
fn service_a_missed_beacon_opens_a_hole_and_the_next_one_closes_it(){run(0,true,true,|data,c|{
    use crate::{btc_session::data_mode::{PROBE_MAC,PROBE_RF},receive_service::Event};
    ipc::reset();let mut service=Service::new(data).unwrap();
    let id=ipc::buffer(3,&filter(false));assert!(service.step());assert_eq!(words(id),[0,0]);
    // No beacon heard yet: no hole, however long.
    ipc::advance(1500);assert!(service.step());assert!(service.take_event().is_none());
    c.borrow_mut().incoming.push(bss_beacon());assert!(service.step());assert!(service.take_event().is_none());
    // Then only frames to this station, for 1.2 s.
    let mut events=vec![];
    for _ in 0..3{c.borrow_mut().incoming.push(packet());ipc::advance(400);assert!(service.step());events.extend(service.take_event());}
    let [Event::Hole{n:1,channel:6,bssid,ms,wifi:3,kept:3,pending:false,probe:Some(probe),..}]=events.as_slice()else{panic!("no hole: {events:?}")};
    assert_eq!(*bssid,[2,9,8,7,6,5]);assert!((1000..1300).contains(ms),"{ms} ms");
    let at=|address:usize|probe.mac[PROBE_MAC.iter().position(|&a|a==address).unwrap()];
    assert_eq!(at(0xc348)&0xffff,0x10f,"the scheduler as admitted");
    let rx=at(0x1050);assert_eq!(rx&0xfff,(rx>>16)&0xfff,"the receive ring read to its end");
    let rf18=PROBE_RF.iter().position(|&a|a==0x18).unwrap();assert_eq!(probe.rf[0][rf18]&0xff,6,"RF 0x18 on channel 6");
    // Its next beacon closes it; another hole is counted from it.
    c.borrow_mut().incoming.push(bss_beacon());assert!(service.step());
    let Some(Event::HoleEnded{n:1,ms,wifi,kept,probe:Some(_),..})=service.take_event()else{panic!("no end")};
    assert!((1200..1300).contains(&ms),"{ms} ms");assert_eq!((wifi,kept),(1,1));
    // The minute's line: its one hole and the longest, and a good link's registers on its first.
    let mut traffic=None;let mut other=vec![];
    for _ in 0..70{c.borrow_mut().incoming.push(bss_beacon());ipc::advance(900);assert!(service.step());
        match service.take_event(){Some(e@Event::Traffic{..})=>{traffic=Some(e);break},Some(e)=>other.push(e),None=>{}}}
    assert!(other.is_empty(),"{other:?}");
    let Some(Event::Traffic{channel:6,holes:1,longest_ms,probe:Some(_),beacons,..})=traffic else{panic!("no traffic line")};
    assert_eq!(longest_ms,ms);assert!(beacons>=60);drop(service);clean();
});}
