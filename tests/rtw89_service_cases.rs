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
// While no beacon of the BSS reaches the host (the Ryzen, 2026-11-21: none counted in data mode), 3 s with no Wi-Fi frame
// at all is a hole, closed by the next frame (a second or so of an idle link is none); a refused beacon of another network
// is counted on the minute's line, and the first minute's line has the refused frames' headers.
#[test]
fn service_with_no_beacon_of_its_bss_a_second_without_any_frame_is_a_hole(){run(0,true,true,|data,c|{
    use crate::receive_service::Event;
    ipc::reset();let mut service=Service::new(data).unwrap();
    let id=ipc::buffer(3,&filter(false));assert!(service.step());assert_eq!(words(id),[0,0]);
    let mut other=bss_beacon();other[10..22].copy_from_slice(&[2,7,7,7,7,7,2,7,7,7,7,7]);
    c.borrow_mut().incoming.extend([packet(),other]);assert!(service.step());assert!(service.take_event().is_none());
    ipc::advance(1500);assert!(service.step());assert!(service.take_event().is_none(),"1.5 s idle is no hole");
    ipc::advance(1600);assert!(service.step());
    let Some(Event::Hole{n:1,by_beacons:false,ms,kept:0,probe:Some(_),..})=service.take_event()else{panic!("no hole by frames")};
    assert!((3100..3200).contains(&ms),"{ms} ms");
    c.borrow_mut().incoming.push(packet());assert!(service.step());
    let Some(Event::HoleEnded{n:1,ms,kept:1,..})=service.take_event()else{panic!("no end")};assert!((3100..3200).contains(&ms));
    let mut traffic=None;
    for _ in 0..70{c.borrow_mut().incoming.push(packet());ipc::advance(900);assert!(service.step());
        if let Some(e@Event::Traffic{..})=service.take_event(){traffic=Some(e);break}}
    let Some(Event::Traffic{beacons:0,other_beacons:1,bssid:[2,9,8,7,6,5],holes:1,refused:Some(refused),..})=traffic else{panic!("{traffic:?}")};
    assert_eq!(refused.count,1);assert_eq!(refused.frames[0].1[..2],[0x80,0]);assert_eq!(refused.frames[0].1[16..22],[2,7,7,7,7,7]);
    drop(service);clean();
});}

// 213-DRV-0091: the access point's association response tells the firmware of the link: the join info connected as
// INFRA, then the address CAM as INFRA with the AID (Linux's rtw89_core_sta_link_assoc); its deauthentication, and a
// new filter, tell it of no link again.
fn to_station(subtype:u8,body:&[u8])->Vec<u8>{
    let mut f=vec![subtype<<4,0,0,0,2,3,4,5,6,7,2,9,8,7,6,5,2,9,8,7,6,5,0x20,0];f.extend_from_slice(body);f.extend_from_slice(&[0;4]);f
}
#[test]
fn service_an_association_response_tells_the_firmware_of_the_link(){run(0,true,true,|data,c|{
    use crate::receive_service::Event;
    let h2c=|c:&std::rc::Rc<std::cell::RefCell<Control>>,class:u8|c.borrow().normal_packets.iter().filter(|p|(p[0]&3,p[0]>>2,p[1])==(1,class,0))
        .map(|p|p[8..].to_vec()).collect::<Vec<_>>();
    ipc::reset();let mut service=Service::new(data).unwrap();
    let id=ipc::buffer(3,&filter(false));assert!(service.step());assert_eq!(words(id),[0,0]);
    let(joins,cams)=(h2c(c,8).len(),h2c(c,6).len());
    // A refused association (status 17) tells nothing.
    c.borrow_mut().buffers=vec![crate::captures::frame(&to_station(1,&[0x11,4,17,0,1,0xc0]),0,0)];assert!(service.step());
    assert!(service.take_event().is_none());assert_eq!((h2c(c,8).len(),h2c(c,6).len()),(joins,cams));
    // Status 0, AID 1.
    c.borrow_mut().buffers=vec![crate::captures::frame(&to_station(1,&[0x11,4,0,0,1,0xc0]),0,0)];assert!(service.step());
    let Some(Event::Linked{channel:6,bssid:[2,9,8,7,6,5],aid:Some(1),was:None,before:Some(_),after:Some(_)})=service.take_event()else{panic!("no link")};
    let(join,cam)=(h2c(c,8),h2c(c,6));assert_eq!((join.len(),cam.len()),(joins+1,cams+1));
    assert_eq!(join.last().unwrap()[..4],[0,0,0,6],"OP 0, NET_TYPE INFRA, WIFI_ROLE STATION");
    let cam=cam.last().unwrap();let w=|i:usize|u32::from_le_bytes(cam[4*i..4*i+4].try_into().unwrap());
    assert_eq!(((w(2)>>1)&3,w(9)&0xfff,w(13)&1),(2,1,1),"NET_TYPE INFRA, AID 1, the BSSID valid");
    assert_eq!(cam[22..28],[2,9,8,7,6,5]);
    // Its deauthentication: no link again.
    c.borrow_mut().buffers=vec![crate::captures::frame(&to_station(12,&[3,0]),0,0)];assert!(service.step());
    let Some(Event::Linked{aid:None,was:Some(1),before:Some(_),after:Some(_),..})=service.take_event()else{panic!("no unlink")};
    assert_eq!(h2c(c,8).last().unwrap()[..4],[0,1,0,4],"OP 1, NO_LINK");
    let cam=h2c(c,6);let cam=cam.last().unwrap();let w2=u32::from_le_bytes(cam[8..12].try_into().unwrap());assert_eq!((w2>>1)&3,0);
    // Linked again, then a new filter: no link first, then the filter's own CAM entry.
    c.borrow_mut().buffers=vec![crate::captures::frame(&to_station(3,&[0x11,4,0,0,2,0xc0]),0,0)];assert!(service.step());
    assert!(matches!(service.take_event(),Some(Event::Linked{aid:Some(2),..})));
    let joins=h2c(c,8).len();let id=ipc::buffer(3,&filter(true));assert!(service.step());assert_eq!(words(id),[0,0]);
    assert_eq!(h2c(c,8).len(),joins+1);assert_eq!(h2c(c,8).last().unwrap()[..4],[0,1,0,4]);
    drop(service);clean();
});}

// 213-DRV-0091 step 2: a beacon interval and a millisecond after the link's H2Cs, port 0 is set for INFRA as Linux's
// rtw89_mac_port_update does; the station's checks then expect it; the deauthentication sets it for no link again.
#[test]
fn service_the_link_sets_port_0_for_infra_after_a_beacon_interval(){run(0,true,true,|data,c|{
    use crate::{btc_session::data_mode::PROBE_MAC,receive_service::Event};
    let port=|probe:&crate::btc_session::data_mode::Probe|probe.mac[PROBE_MAC.iter().position(|&a|a==0xc400).unwrap()];
    ipc::reset();let mut service=Service::new(data).unwrap();
    let id=ipc::buffer(3,&filter(false));assert!(service.step());assert_eq!(words(id),[0,0]);
    c.borrow_mut().buffers=vec![crate::captures::frame(&to_station(1,&[0x11,4,0,0,7,0xc0]),0,0)];assert!(service.step());
    let Some(Event::Linked{aid:Some(7),after:Some(after),..})=service.take_event()else{panic!("no link")};
    assert_eq!(port(&after)&0x13e1f,4,"the H2Cs first, the port untouched");
    ipc::advance(50);assert!(service.step());assert!(service.take_event().is_none(),"not before a beacon interval");
    ipc::advance(60);assert!(service.step());
    let Some(Event::Port{channel:6,linked:true,after:Some(after)})=service.take_event()else{panic!("no port")};
    assert_eq!(port(&after)&0x13e1f,0x1281c,"FUNC_EN, NET_TYPE INFRA, TBTT_PROHIB, BRK_SETUP, BSSID fit, TSF update");
    // The data mode's checks go on expecting it: steps with traffic run on.
    for _ in 0..3{c.borrow_mut().incoming.push(packet());ipc::advance(20);assert!(service.step());}
    assert!(service.take_event().is_none());
    c.borrow_mut().buffers=vec![crate::captures::frame(&to_station(10,&[8,0]),0,0)];assert!(service.step());
    assert!(matches!(service.take_event(),Some(Event::Linked{aid:None,..})));
    ipc::advance(110);assert!(service.step());
    let Some(Event::Port{linked:false,after,..})=service.take_event()else{panic!("no port back")};
    if let Some(after)=after{assert_eq!(port(&after)&0x13e1f,4);}
    drop(service);clean();
});}

// 213-DRV-0091: the firmware's error codes are read at each data-mode check as Linux's rtw89_mac_get_err_status reads
// them (HALT_C2H_CTRL set, then HALT_C2H, then the CTRL cleared), named with their level, counted in the minute's line,
// and never answered.
#[test]
fn service_reads_the_firmwares_error_codes_names_their_level_and_answers_nothing(){run(0,true,true,|data,c|{
    use crate::{btc_session::data_mode::{Errors,Level},receive_service::Event};
    ipc::reset();let mut service=Service::new(data).unwrap();assert!(service.step());
    assert!(service.take_firmware_error().is_none(),"no code posted, none read");
    // An L0 recovery's two codes, an L1 request, an assertion by its scenario, and a code ser.c does not know.
    c.borrow_mut().halt_codes=vec![0x0001,0x0003,0x1000,0x3000_0012,0x8000_0000];
    let mut seen=vec![];
    for _ in 0..20{ipc::advance(20);assert!(service.step());while let Some(e)=service.take_firmware_error(){seen.push(e)}if seen.len()==5{break}}
    let got:Vec<_>=seen.iter().map(|e|(e.count,e.raw,e.code,e.name,e.level)).collect();
    assert_eq!(got,[(1,1,1,"L0_ERR_CMAC0",Level::L0),(2,3,3,"L0_RESET_DONE",Level::L0),(3,0x1000,0x1000,"L1_ERR_DMAC",Level::L1),
        (4,0x3000_0012,0x4000,"ASSERTION",Level::L2),(5,0x8000_0000,0x8000_0000,"UNKNOWN",Level::Unknown)]);
    assert_eq!((seen[0].event,seen[2].event,seen[3].event),("SER_EV_L0_RESET","SER_EV_L1_RESET (M1)","SER_EV_L2_RESET"));
    assert!(seen.iter().all(|e|e.ctrl==1),"{seen:?}");
    assert_eq!(c.borrow().halt_h2c_writes,0,"nothing answered: no M2, no M4");
    // The minute's line counts them by level, and traffic runs on.
    let mut traffic=None;
    for _ in 0..70{c.borrow_mut().incoming.push(packet());ipc::advance(900);assert!(service.step());
        if let Some(e@Event::Traffic{..})=service.take_event(){traffic=Some(e);break}}
    let Some(Event::Traffic{errors,..})=traffic else{panic!("no traffic line")};
    assert_eq!(errors,Errors{l0:2,l1:1,l2:1,unknown:1,hisr_halt:5,hisr_wdt:0},"each posted code raised HISR0's HALT_C2H");
    c.borrow_mut().incoming.push(packet());let id=ipc::buffer(5,&[]);assert!(service.step());assert_eq!(words(id)[0]&255,0);
    let _=Event::FirmwareError{count:0,ctrl:0,raw:0,ser:0,code:0,name:"",level:Level::L0,event:""};drop(service);clean();
});}

// 213-DRV-0091: HISR0's halt bits are read at each check under this driver's mask, as rtw89_pci_recognize_intrs reads
// them, cleared by writing them back and counted; nothing is answered to the firmware.
#[test]
fn service_reads_hisr0s_halt_bits_under_the_mask_and_answers_nothing(){run(0,true,true,|data,c|{
    use crate::{btc_session::data_mode::Errors,receive_service::Event};
    ipc::reset();let mut service=Service::new(data).unwrap();assert!(service.step());
    assert!(service.take_halt_status().is_none(),"nothing raised, nothing seen");
    // The halt status with no code, as a silent recovery's might be, then the watchdog's.
    c.borrow_mut().hisr_raise=vec![1<<21,1<<22];
    let mut seen=vec![];
    for _ in 0..20{ipc::advance(20);assert!(service.step());seen.extend(service.take_halt_status());if seen.len()==2{break}}
    let got:Vec<_>=seen.iter().map(|h|(h.count,h.hisr,h.ctrl,h.intx)).collect();
    assert_eq!(got,[(1,1<<21,0,Some(false)),(2,1<<22,0,Some(false))],"HIMR0 masked: INTx not asserted");
    for _ in 0..3{ipc::advance(20);assert!(service.step());}assert!(service.take_halt_status().is_none(),"cleared as Linux clears it");
    // A posted code: its HISR0 bit at a check, and the code read and named.
    c.borrow_mut().halt_codes=vec![0x0003];let(mut halt,mut code)=(None,None);
    for _ in 0..20{ipc::advance(20);assert!(service.step());halt=halt.or(service.take_halt_status());code=code.or(service.take_firmware_error());
        if halt.is_some()&&code.is_some(){break}}
    assert_eq!(halt.map(|h|(h.count,h.hisr)),Some((3,1<<21)));assert_eq!(code.map(|e|e.name),Some("L0_RESET_DONE"));
    assert_eq!(c.borrow().halt_h2c_writes,0,"nothing answered");
    let mut traffic=None;
    for _ in 0..70{c.borrow_mut().incoming.push(packet());ipc::advance(900);assert!(service.step());
        if let Some(e@Event::Traffic{..})=service.take_event(){traffic=Some(e);break}}
    let Some(Event::Traffic{errors,..})=traffic else{panic!("no traffic line")};
    assert_eq!(errors,Errors{l0:1,l1:0,l2:0,unknown:0,hisr_halt:2,hisr_wdt:1});
    let _=Event::HaltStatus{count:0,hisr:0,ctrl:0,ser:0,intx:None};drop(service);clean();
});}

// 213-DRV-0102: the detection floor is Linux's arithmetic for the joined access point's RSSI (phy.c at 05f7e89): at
// -60 dBm OFDM PD 9, CCK PD 0xAD and EDCCA 66, and each clamp at its edge.
#[test]
fn the_detection_floor_is_linuxs_arithmetic_for_the_aps_rssi(){
    use crate::service::detection_floor::*;
    assert!(!DETECTION_FLOOR_AS_LINUX,"committed as the driver was");
    assert_eq!(for_rssi(-60),Floor{ofdm_pd:9,cck_pd:0xad,edcca:66});
    // OFDM: the lower clamp (8 + 23) down to -79 dBm, then 2 dB a step, the upper (70 + 23) from -17 dBm.
    assert_eq!([-100,-79,-77,-61,-17,0].map(|d|for_rssi(d).ofdm_pd),[0,0,1,9,31,31]);
    // CCK: max(igi - 23, -18) - 110 as a byte, -128 at and below -105 dBm.
    assert_eq!([-110,-105,-104,-60,0].map(|d|for_rssi(d).cck_pd),[0x80,0x80,0x81,0xad,0xe9]);
    // EDCCA: igi + 15, at least 66 (EDCCA_TH_L2H_LB).
    assert_eq!([-100,-59,-58,-20].map(|d|for_rssi(d).edcca),[66,66,67,105]);
    assert_eq!(fields(&for_rssi(-60)),[(0x4860,0x7c0,9<<6),(0x4860,1<<30,1<<30),(0x4b64,0xff00_0000,0xad00_0000),
        (0x4b74,1<<30,1<<30),(0x4884,0xff00_ffff,0x4200_4242)]);
}

// 213-DRV-0102: with the switch on, the floor is written once the joined BSS's beacons are measured (their reports'
// signal), read back, and counted in the minute line with every report by its signal.
#[test]
fn service_writes_the_detection_floor_once_the_aps_beacons_are_measured(){run(0,true,true,|data,c|{
    use crate::{captures::{bytes,frame as buffer,FIVE},receive_service::Event};
    ipc::reset();let mut service=Service::new(data).unwrap();service.floor_as_linux(true,false);
    let id=ipc::buffer(3,&filter(false));assert!(service.step());assert_eq!(words(id),[0,0]);
    c.borrow_mut().buffers=vec![crate::captures::frame(&to_station(1,&[0x11,4,0,0,1,0xc0]),0,0)];assert!(service.step());
    assert!(matches!(service.take_event(),Some(Event::Linked{aid:Some(1),..})));
    let mut beacon=bss_beacon();beacon.extend_from_slice(&[0;4]);
    let mut set=None;
    for _ in 0..12{c.borrow_mut().buffers=vec![buffer(&beacon,1,4),bytes(FIVE)];ipc::advance(10);assert!(service.step());
        if let Some(e@Event::FloorSet{..})=service.take_event(){set=Some(e);break}}
    // FIVE's report is -63 dBm: igi 47, OFDM PD (47 - 31) >> 1 = 8, CCK 24 - 110 = -86 (0xAA), EDCCA 66.
    let Some(Event::FloorSet{dbm:-63,measured:8,ofdm_pd:8,cck_pd:0xaa,edcca:66,written:true,registers,..})=set else{panic!("{set:?}")};
    assert_eq!((registers[0]>>6&31,registers[0]>>30&1,registers[1]>>24,registers[2]>>30&1,registers[3]&0xff,registers[3]>>24),(8,1,0xaa,1,66,66));
    // An OFDM report not to this radio (DATA's with to_self cleared) has no RPL: binned apart by its header, -56 dBm.
    let mut other=bytes(crate::captures::DATA);other[16]&=!1;
    c.borrow_mut().buffers=vec![other,bytes(crate::captures::DATA)];ipc::advance(10);assert!(service.step());
    let mut traffic=None;
    for _ in 0..70{c.borrow_mut().incoming.push(packet());ipc::advance(900);assert!(service.step());
        if let Some(e@Event::Traffic{..})=service.take_event(){traffic=Some(e);break}}
    let Some(Event::Traffic{floor,..})=traffic else{panic!("no traffic line")};
    assert!(floor.set&&floor.bss_dbm==Some(-63)&&floor.measured>=8,"{floor:?}");
    assert_eq!(floor.ppdu_rssi[8],floor.ppdu_rssi.iter().sum::<u32>(),"-63 dBm in the -60 to -64 bin: {floor:?}");
    assert!(floor.ppdu_rssi[8]>=8,"{floor:?}");assert_eq!(floor.registers,registers);
    assert_eq!((floor.no_rpl[7],floor.no_rpl.iter().sum::<u32>(),floor.header_over),(1,1,(9,1)),"-55 to -59 dBm, 4.5 dB: {floor:?}");
    let _=Event::FloorRefused{channel:36};drop(service);clean();
});}

// 213-DRV-0104, SEND_BEHIND_LATE: with the switch, a send while one is late (answered at its deadline, its release
// awaited) goes on the ring behind it with its call open, and is answered at its own release with its true outcome; the
// late one, parked, is counted released late and is not sent again. A third while one is parked and the next is late
// too is refused busy. Without the switch the second is refused busy, as before. RPQ alone, no firmware report (fault 7),
// the releases held back as the MAC holds the frames.
#[test]
fn service_sends_behind_a_late_frame_when_the_switch_is_on(){for behind in[false,true]{run(7,true,true,|data,c|{
    use crate::receive_service::Event;
    ipc::reset();let mut service=Service::new(data).unwrap().send_behind_late(behind);service.rpq_alone();assert!(service.step());
    let sent=|id:usize|{let(w,b)=ipc::response(id).unwrap();assert_eq!(w,[2<<16,0]);wlanhw::Sent::decode(&mut Reader::new(&b[..2])).unwrap()};
    c.borrow_mut().hold_rpq=true;
    let a=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());assert!(ipc::response(a).is_none());
    ipc::advance(91);assert!(service.step());assert!(!sent(a).acked,"A answered at its deadline");
    assert!(matches!(service.take_event(),Some(Event::LateFrame{count:1,..})));
    let b=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());
    if !behind{error(b,wlanhw::Error::Busy);assert_eq!(c.borrow().published,1);c.borrow_mut().hold_rpq=false;ipc::advance(5);assert!(service.step());
        drop(service);clean();return}
    assert!(ipc::response(b).is_none(),"B taken, its call open");assert_eq!(c.borrow().published,2,"B on the ring behind A");
    // Both released within B's time: B answered with its outcome, A counted as a late frame released.
    c.borrow_mut().hold_rpq=false;ipc::advance(10);assert!(service.step());
    assert_eq!(sent(b),wlanhw::Sent{acked:true,attempts:1});
    let event=service.take_event();assert!(matches!(event,Some(Event::LateRelease{acked:true,attempts:1,..})),"{event:?}");
    // D late, E behind it and late too: F is refused, one parked at a time; then both released, nothing sent again.
    c.borrow_mut().hold_rpq=true;
    let d=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());ipc::advance(91);assert!(service.step());assert!(!sent(d).acked);
    let e=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());assert!(ipc::response(e).is_none());
    ipc::advance(91);assert!(service.step());assert!(!sent(e).acked,"E answered at its deadline");
    let f=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());error(f,wlanhw::Error::Busy);
    c.borrow_mut().hold_rpq=false;ipc::advance(10);assert!(service.step());ipc::advance(5);assert!(service.step());
    assert_eq!(c.borrow().published,4,"A, B, D and E once each");
    let g=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());assert!(service.step());assert!(sent(g).acked,"sends go on");
    let stopped=service.stop();assert!(stopped.failure.is_none(),"{:?}",stopped.failure);clean();
});}}

// 213-DRV-0104, SEND_BEHIND_LATE_ALTERNATES: the partial minute as committed, then off (the send behind a late one refused
// busy), then on (parked, the next taken), then off; each minute's line counts what it did and says what it held; and
// the switch waits while a frame is pending or parked at the boundary.
#[test]
fn service_alternates_sending_behind_a_late_frame_by_the_minute(){run(7,true,true,|data,c|{
    use crate::receive_service::Event;
    ipc::reset();let mut service=Service::new(data).unwrap().send_behind_late_alternates(true);service.rpq_alone();assert!(service.step());
    macro_rules! minute{()=>{{let mut line=None;
        for _ in 0..80{c.borrow_mut().incoming.push(packet());ipc::advance(900);assert!(service.step());
            if let Some(Event::Traffic{tx,..})=service.take_event(){line=Some(tx);break}}
        line.expect("a minute line")}}}
    // A late send and one behind it, both released 10 ms after the second: whether the second was taken or refused.
    macro_rules! late_and_behind{()=>{{
        c.borrow_mut().hold_rpq=true;
        let a=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());ipc::advance(91);assert!(service.step());assert!(ipc::response(a).is_some());
        let _=service.take_event();let b=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());let taken=ipc::response(b).is_none();
        c.borrow_mut().hold_rpq=false;ipc::advance(10);assert!(service.step());ipc::advance(5);assert!(service.step());
        assert!(ipc::response(b).is_some());let _=service.take_event();taken}}}
    let first=minute!();assert_eq!(first.behind_label(),"BEHIND LATE NEVER","the partial minute");
    assert!(!late_and_behind!(),"off: refused");
    let off=minute!();assert_eq!(off.behind_label(),"THIS MINUTE BEHIND LATE OFF");
    assert_eq!((off.answered_late,off.busy,off.parked,off.parked_released),(1,1,0,0),"{off:?}");
    assert!(late_and_behind!(),"on: taken behind it");
    // A late frame pending at the boundary keeps the minute's kind until its release (made late 2 s before the line, as
    // a frame held past LATE_MS stops the service).
    for _ in 0..64{c.borrow_mut().incoming.push(packet());ipc::advance(900);assert!(service.step());
        assert!(!matches!(service.take_event(),Some(Event::Traffic{..})),"not yet the minute's line");}
    c.borrow_mut().hold_rpq=true;let d=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());ipc::advance(91);assert!(service.step());
    assert!(ipc::response(d).is_some());let _=service.take_event();
    let on=minute!();assert_eq!(on.behind_label(),"THIS MINUTE BEHIND LATE ON");
    assert_eq!((on.answered_late,on.busy,on.parked,on.parked_released,on.parked_acked),(2,0,1,1,1),"{on:?}");
    let e=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());assert!(ipc::response(e).is_none(),"still on: parked behind the pending one");
    c.borrow_mut().hold_rpq=false;ipc::advance(10);assert!(service.step());ipc::advance(5);assert!(service.step());assert!(ipc::response(e).is_some());
    let _=service.take_event();
    assert!(!late_and_behind!(),"switched off once nothing was pending or parked");
    let next=minute!();assert_eq!(next.behind_label(),"THIS MINUTE BEHIND LATE OFF");
    assert_eq!((next.parked,next.busy),(1,1),"D parked, E behind it, before the switch; then a refusal: {next:?}");
    let stopped=service.stop();assert!(stopped.failure.is_none(),"{:?}",stopped.failure);clean();
});}

// 213-DRV-0104, POLL_TIGHT_WHILE_PENDING: a send is answered in the step that took it, not the next; with its release held
// the tight poll gives up after TIGHT_US and the send is answered at its release; a frame that arrives while it polls is
// taken and its waiter woken in that step; and by the minute when it alternates.
#[test]
fn service_polls_tight_while_a_send_is_pending(){
    for tight in [false,true]{run(0,true,true,|data,_|{
        ipc::reset();let mut service=Service::new(data).unwrap().poll_tight(tight,false);assert!(service.step());
        let send=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());
        assert_eq!(ipc::response(send).is_some(),tight,"answered in its own step only with the tight poll");
        assert!(service.step());assert!(ipc::response(send).is_some());drop(service);clean();
    });}
    for tight in [false,true]{run(0,true,true,|data,c|{
        use crate::receive_service::Event;
        ipc::reset();let mut service=Service::new(data).unwrap().poll_tight(tight,false);assert!(service.step());
        let wait=ipc::words(6,0);assert!(service.step());assert!(ipc::response(wait).is_none(),"nothing received yet");
        // The release held; a frame for the station comes 600 register reads into the step, after the send was taken.
        c.borrow_mut().hold_reports=true;c.borrow_mut().incoming_after=Some((600,vec![packet()]));
        let send=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());assert!(ipc::response(send).is_none(),"held");
        assert_eq!(ipc::response(wait).is_some(),tight,"the frame taken and the waiter woken in that step only while polling");
        c.borrow_mut().hold_reports=false;ipc::advance(5);assert!(service.step());assert!(ipc::response(send).is_some(),"answered at its release");
        assert!(ipc::response(wait).is_some());
        let mut traffic=None;
        for _ in 0..80{c.borrow_mut().incoming.push(packet());ipc::advance(900);assert!(service.step());
            if let Some(Event::Traffic{tx,..})=service.take_event(){traffic=Some(tx);break}}
        let tx=traffic.expect("a minute line");
        assert_eq!(tx.tight_label(),if tight{"TIGHT ALWAYS"}else{"TIGHT NEVER"});
        if tight{assert!((10..=25).contains(&tx.tight_polls),"gave up after about TIGHT_US / TIGHT_PACE_US polls: {}",tx.tight_polls);}
        else{assert_eq!(tx.tight_polls,0);}
        drop(service);clean();
    });}
    // Alternating: the partial minute never, then off, then on. The frame buffer fills (nobody reads), so the tight poll
    // reads the release queue alone: data frames final on DMA and the RPQ, as on the Ryzen (RPQ_ALONE, no firmware report).
    run(7,true,true,|data,c|{
        use crate::receive_service::Event;
        ipc::reset();let mut service=Service::new(data).unwrap().poll_tight(false,true);service.rpq_alone();assert!(service.step());
        let mut labels=vec![];
        for _ in 0..3{
            let mut line=None;
            for _ in 0..80{c.borrow_mut().incoming.push(packet());ipc::advance(900);assert!(service.step());
                if let Some(Event::Traffic{tx,..})=service.take_event(){line=Some(tx);break}}
            let tx=line.expect("a minute line");labels.push(tx.tight_label());
            let send=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());let own_step=ipc::response(send).is_some();
            assert!(service.step());assert!(ipc::response(send).is_some());
            labels.push(if own_step{"ANSWERED IN ITS STEP"}else{"ANSWERED NEXT STEP"});
        }
        assert_eq!(labels,["TIGHT NEVER","ANSWERED NEXT STEP","THIS MINUTE TIGHT OFF","ANSWERED IN ITS STEP","THIS MINUTE TIGHT ON","ANSWERED NEXT STEP"]);
        drop(service);clean();
    });
}

// 213-DRV-0104, BEGIN_LIGHT: a send begun within CHECK_MS of the step's checks makes no liveness checks of its own (no
// radio check in its step) and is answered as any other; one begun later than that, the queue left unread, makes them as
// without the switch (its reconcile, over the frames waiting, validates once more).
#[test]
fn service_begins_a_send_light_within_check_ms_of_the_steps_checks(){
    let(mut made,mut late)=(vec![],vec![]);
    for light in [false,true]{run(0,true,true,|data,c|{
        // Data frames final on DMA and the RPQ, as on the Ryzen (RPQ_ALONE): a release is read with the frame buffer full.
        ipc::reset();let mut service=Service::new(data).unwrap().begin_light(light,false);service.rpq_alone();assert!(service.step());
        let before=c.borrow().radio_checks;
        let send=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());assert!(ipc::response(send).is_none(),"pending");
        made.push(c.borrow().radio_checks-before);
        assert!(service.step());assert!(ipc::response(send).is_some(),"answered at its release");
        // The frame buffer full (nobody reads), the queue is not read empty and the checks wait up to CHECK_LATEST_MS.
        for _ in 0..20{c.borrow_mut().incoming.push(packet());}for _ in 0..4{ipc::advance(1);assert!(service.step());}
        ipc::advance(12);let before=c.borrow().radio_checks;
        let send=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());
        late.push(c.borrow().radio_checks-before);
        assert!(service.step());assert!(ipc::response(send).is_some());
        drop(service);clean();
    });}
    assert!(made[0]>=1&&made[1]==0,"the send's own radio checks (its checks', its reconcile's), made without the switch only: {made:?}");
    assert!(late[0]>=1&&late[0]==late[1],"over CHECK_MS since the step's checks, begun with its own either way: {late:?}");
    // Alternating: the partial minute never, then off, then on; each minute's line counts the sends begun light.
    run(0,true,true,|data,c|{
        use crate::receive_service::Event;
        ipc::reset();let mut service=Service::new(data).unwrap().begin_light(false,true);assert!(service.step());
        let mut seen=vec![];
        for _ in 0..4{
            let send=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());assert!(service.step());assert!(ipc::response(send).is_some());
            let mut line=None;
            for _ in 0..80{c.borrow_mut().incoming.push(packet());ipc::advance(900);assert!(service.step());
                if let Some(Event::Traffic{tx,..})=service.take_event(){line=Some(tx);break}}
            let tx=line.expect("a minute line");seen.push((tx.light_label(),tx.begun_light));
        }
        assert_eq!(seen,[("LIGHT NEVER",0),("THIS MINUTE LIGHT OFF",0),("THIS MINUTE LIGHT ON",1),("THIS MINUTE LIGHT OFF",0)]);
        drop(service);clean();
    });
}

// 213-DRV-0102, DETECTION_FLOOR_ALTERNATES: off for the first whole minute after the beacons are measured, then Linux's,
// then the values found again, exactly; a send pending at a minute's boundary postpones the switch to the next poll.
#[test]
fn service_alternates_the_detection_floor_by_the_minute(){run(0,true,true,|data,c|{
    use crate::{captures::{bytes,frame as buffer,FIVE},receive_service::Event,service::detection_floor::{fields,for_rssi}};
    ipc::reset();let mut service=Service::new(data).unwrap();service.floor_as_linux(true,true);
    let id=ipc::buffer(3,&filter(false));assert!(service.step());assert_eq!(words(id),[0,0]);
    c.borrow_mut().buffers=vec![crate::captures::frame(&to_station(1,&[0x11,4,0,0,1,0xc0]),0,0)];assert!(service.step());
    assert!(matches!(service.take_event(),Some(Event::Linked{aid:Some(1),..})));
    let mut beacon=bss_beacon();beacon.extend_from_slice(&[0;4]);let mut found=None;
    for _ in 0..12{c.borrow_mut().buffers=vec![buffer(&beacon,1,4),bytes(FIVE)];ipc::advance(10);assert!(service.step());
        if let Some(Event::FloorAlternates{dbm:-63,measured:8,found:f,..})=service.take_event(){found=Some(f);break}}
    let found=found.expect("the alternation announced");assert_eq!(service.floor_registers(),found,"nothing written yet");
    let linux=fields(&for_rssi(-63));let is_linux=|r:[u32;4]|linux.iter().all(|&(at,mask,value)|{
        let i=[0x4860,0x4b64,0x4b74,0x4884].iter().position(|a|*a==at).unwrap();r[i]&mask==value});
    // Each minute's line: the kind it held and its registers.
    let mut minutes=vec![];
    for _ in 0..300{
        c.borrow_mut().incoming.push(packet());ipc::advance(900);assert!(service.step());
        if let Some(Event::Traffic{floor,..})=service.take_event(){minutes.push(floor);if minutes.len()==4{break}}
    }
    let kinds:Vec<_>=minutes.iter().map(|m|(m.on(),m.off())).collect();
    assert_eq!(kinds,[(false,false),(false,true),(true,false),(false,true)],"the partial minute, then off, on, off");
    assert_eq!(minutes[1].registers,found);assert!(is_linux(minutes[2].registers),"{:X?}",minutes[2].registers);
    assert_eq!(minutes[3].registers,found,"the values found, restored exactly");
    assert!(minutes.iter().all(|m|m.switch_failures==0));
    // A send pending at the boundary: the switch waits for the next poll with none pending, not for the next minute.
    // (Held within LATE_MS: sent a second before the boundary.)
    for _ in 0..59{c.borrow_mut().incoming.push(packet());ipc::advance(999);assert!(service.step());assert!(service.take_event().is_none());}
    c.borrow_mut().hold_reports=true;let send=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());
    let mut line=None;
    for _ in 0..40{ipc::advance(100);assert!(service.step());if let Some(e@Event::Traffic{..})=service.take_event(){line=Some(e);break}}
    let Some(Event::Traffic{floor,..})=line else{panic!("no line")};assert!(floor.on(),"the minute that ended held Linux's");
    assert!(is_linux(service.floor_registers()),"still Linux's while the send is pending");
    c.borrow_mut().hold_reports=false;ipc::advance(20);assert!(service.step());assert!(ipc::response(send).is_some());
    ipc::advance(5);assert!(service.step());assert_eq!(service.floor_registers(),found,"switched at the next poll");
    drop(service);clean();
});}

// 213-DRV-0102: every field of the floor is written and read back on its own. Two registers that keep their value whatever
// is written (the Ryzen's 0x4B64 and 0x4B74, boot 93) are named as read as before the write, and EDCCA's level after them is
// still written: boot 93's ON minutes stopped at 0x4B64 and never wrote 0x4B74 or 0x4884.
#[test]
fn service_writes_every_floor_field_and_names_those_not_read_back(){run(0,true,true,|data,c|{
    use crate::{captures::{bytes,frame as buffer,FIVE},receive_service::Event,service::detection_floor::for_rssi};
    ipc::reset();c.borrow_mut().write_ignored=vec![0x14b64,0x14b74];
    let mut service=Service::new(data).unwrap();service.floor_as_linux(true,true);
    let id=ipc::buffer(3,&filter(false));assert!(service.step());assert_eq!(words(id),[0,0]);
    c.borrow_mut().buffers=vec![crate::captures::frame(&to_station(1,&[0x11,4,0,0,1,0xc0]),0,0)];assert!(service.step());
    assert!(matches!(service.take_event(),Some(Event::Linked{aid:Some(1),..})));
    let mut beacon=bss_beacon();beacon.extend_from_slice(&[0;4]);let mut found=None;
    for _ in 0..12{c.borrow_mut().buffers=vec![buffer(&beacon,1,4),bytes(FIVE)];ipc::advance(10);assert!(service.step());
        if let Some(Event::FloorAlternates{found:f,..})=service.take_event(){found=Some(f);break}}
    let found=found.expect("the alternation announced");
    let mut minutes=vec![];
    for _ in 0..300{
        c.borrow_mut().incoming.push(packet());ipc::advance(900);assert!(service.step());
        if let Some(Event::Traffic{floor,..})=service.take_event(){minutes.push(floor);if minutes.len()==4{break}}
    }
    let linux=for_rssi(-63);let on=minutes[2];assert!(on.on(),"{on:?}");
    assert_eq!((on.registers[0]>>6&31,on.registers[0]>>30&1),(u32::from(linux.ofdm_pd),1),"the OFDM floor: {:X?}",on.registers);
    assert_eq!((on.registers[1],on.registers[2]),(found[1],found[2]),"0x4B64 and 0x4B74 keep what they held");
    assert_eq!((on.registers[3]>>24,on.registers[3]&0xffff),(u32::from(linux.edcca),u32::from(linux.edcca)*0x101),"EDCCA written after them");
    assert_eq!((on.written,on.switch_failures),(crate::btc_session::data_mode::FloorWrite{unread:0b0110,unchanged:0b0110},0),"named, not a failed switch");
    for off in [minutes[1],minutes[3]]{assert!(off.off());assert_eq!((off.written,off.switch_failures),(Default::default(),0));}
    assert_eq!(minutes[3].registers,found,"the values found, restored exactly");
    drop(service);clean();
});}

// 213-DRV-0091: a late frame the MAC holds is probed while it holds it: the first HELD_PROBES of a run, then one a minute;
// a frame released in time after the first is probed once beside them. The minute line counts the late frames released,
// those acknowledged at the first try and their median hold, and the frame buffer's full spells.
#[test]
fn service_probes_held_frames_and_counts_how_the_minutes_sends_ended(){run(0,true,true,|data,c|{
    use crate::receive_service::Event;
    ipc::reset();let mut service=Service::new(data).unwrap().minute_detail(true);assert!(service.step());
    let mut held=vec![];
    for n in 1..=10u32{
        c.borrow_mut().hold_reports=true;
        let send=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());assert!(ipc::response(send).is_none());
        ipc::advance(91);assert!(service.step(),"late {n}");assert!(ipc::response(send).is_some(),"answered at its deadline");
        // 213-DRV-0104: a send while the late one waits for its release is refused busy, and counted.
        if n==1{let busy=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());error(busy,wlanhw::Error::Busy);}
        held.extend(service.take_held());
        c.borrow_mut().hold_reports=false;ipc::advance(20);assert!(service.step());
        let _=service.take_event();
    }
    let probed:Vec<_>=held.iter().map(|e|match e{Event::Held{n,held:true,ms,..}=>{assert!(*ms>=90,"{e:?}");*n},e=>panic!("{e:?}")}).collect();
    assert_eq!(probed,[1,2,3,4,5,6,7,8,9],"eight, then one in the minute");
    // A frame released in time after them: probed once beside them.
    let send=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());assert!(service.step());assert!(ipc::response(send).is_some());
    assert!(matches!(service.take_held(),Some(Event::Held{held:false,..})),"the in-time frame's probe");
    let send=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());assert!(service.step());assert!(ipc::response(send).is_some());
    assert!(service.take_held().is_none(),"once only");
    // The frame buffer fills: twenty frames for the station and no reader.
    {let mut c=c.borrow_mut();let mut retried=packet();retried[1]|=8;c.incoming.push(retried);for _ in 0..20{c.incoming.push(packet());}}
    for _ in 0..4{ipc::advance(5);assert!(service.step());}
    let mut traffic=None;
    for _ in 0..70{c.borrow_mut().incoming.push(packet());ipc::advance(900);assert!(service.step());
        if let Some(e@Event::Traffic{..})=service.take_event(){traffic=Some(e);break}}
    let Some(Event::Traffic{tx,..})=traffic else{panic!("no traffic line")};
    assert_eq!((tx.late,tx.first_try,tx.not_acked,tx.busy,tx.answered_late,tx.parked),(10,10,0,1,10,0),"{tx:?}");
    assert_eq!(tx.behind_label(),"BEHIND LATE NEVER");
    // 213-DRV-0104: the two sends in time binned by their answer's time, every arrival after the first by its gap since
    // the answer before it (13 arrivals: ten late, one refused, two in time), and the busiest second.
    // 213-DRV-0104: every ended send's submission to fetch and fetch to release, by size: twelve of 64 bytes.
    let timed:Vec<(u32,u32)>=tx.fetch_us.iter().zip(tx.release_us.iter()).map(|(f,r)|(f.iter().sum(),r.iter().sum())).collect();
    assert_eq!(timed,vec![(12,12),(0,0),(0,0)],"{tx:?}");
    // Every send begun timed from its receive to its dispatch and to its submission. The model's receive waits its whole
    // 2 ms with a request queued too, so none is found waiting.
    assert_eq!((tx.to_dispatch_us.iter().sum::<u32>(),tx.to_submit_us.iter().sum::<u32>(),tx.found_waiting),(12,12,0),"{tx:?}");
    let in_time:Vec<u32>=tx.answer_ms.iter().map(|c|c.iter().sum()).collect();
    assert_eq!((in_time,tx.gap_ms.iter().sum::<u32>()),(vec![2,0,0],12),"two of 64 bytes in time, 13 arrivals: {tx:?}");
    assert!(tx.answer_ms[0][..5].iter().sum::<u32>()==2,"answered within 10 ms in the model: {:?}",tx.answer_ms);
    // Each unicast send ended at 6 Mb/s (rate 12, the fifth of LEGACY_RATES): 12 ended (ten late, two in time), all
    // acknowledged at their one try; the refused one did not end.
    assert_eq!((tx.by_rate[4],tx.by_tries[1],tx.by_tries.iter().sum::<u32>()),([12,12,12],12,12),"{tx:?}");
    // The frames for this station taken into the buffer (16, nobody reads them), at the model's rate 0 (CCK 1 Mb/s), the
    // first with the retry bit.
    assert_eq!((tx.rx_rate[0],tx.rx_rate.iter().sum::<u32>(),tx.rx_retry),(16,16,1),"{:?}",tx.rx_rate);
    // 213-DRV-0091: the steps' waits and work, in the model's clock: its receive wait ends at once, with the 2 ms asked.
    assert_eq!((tx.wait_over,tx.work_over,tx.work_longest_us),([0,0],[0,0],0),"{tx:?}");assert!(tx.wait_longest_us<=2_000,"{tx:?}");
    {use crate::btc_session::data_mode::{rx_rate_bin,size_class,ms_bin};
    assert_eq!([0,4,11,0x80,0x87,0x8f,0x90,0x100,0x180].map(rx_rate_bin),[0,4,11,12,19,27,28,28,28],"CCK, OFDM, HT MCS 0-15, other");
    assert_eq!([0,199,200,1000,1001,2346].map(size_class),[0,0,1,1,2,2]);
    assert_eq!([0,249,250,1999,2000,9999,10000,19999,20000,49999,50000].map(crate::btc_session::data_mode::us_bin),[0,0,1,7,8,15,16,16,17,17,18]);assert_eq!([0,1,2,29,30,39,89,90,500].map(ms_bin),[0,0,1,14,15,15,20,21,21]);}
    assert!(tx.peak_sends>=2&&tx.peak_sends<=13,"{tx:?}");assert!(tx.median_hold_ms>=90,"{tx:?}");
    assert!(tx.full_times>=1&&tx.full_longest_ms>0,"{tx:?}");
    let _=Event::Held{n:0,held:true,ms:0,submitted:Default::default(),at:Default::default(),registers:service.data_held()};
    drop(service);clean();
});}

// receive_service::MINUTE_DETAIL, committed false for a machine in use: without it no frame is probed and no release is
// placed in the beacon interval; with it both are. The minute's sends are counted either way.
#[test]
fn service_probes_and_places_releases_only_with_the_minute_detail(){for detail in[false,true]{run(0,true,true,|data,c|{
    use crate::receive_service::Event;
    ipc::reset();let mut service=Service::new(data).unwrap().minute_detail(detail);
    let id=ipc::buffer(3,&filter(false));assert!(service.step());assert_eq!(words(id),[0,0]);
    c.borrow_mut().incoming.push(bss_beacon());assert!(service.step());
    c.borrow_mut().hold_reports=true;
    let send=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());
    ipc::advance(91);assert!(service.step());assert!(ipc::response(send).is_some(),"answered at its deadline");
    assert_eq!(service.take_held().is_some(),detail,"the held frame's probe");
    c.borrow_mut().hold_reports=false;ipc::advance(20);assert!(service.step());
    let send=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());assert!(service.step());assert!(ipc::response(send).is_some());
    assert_eq!(service.take_held().is_some(),detail,"the in-time frame's probe");
    let mut traffic=None;
    for _ in 0..70{c.borrow_mut().incoming.push(packet());ipc::advance(900);assert!(service.step());
        if let Some(e@Event::Traffic{..})=service.take_event(){traffic=Some(e);break}}
    let Some(Event::Traffic{tx,..})=traffic else{panic!("no traffic line")};
    assert_eq!((tx.late,tx.answered_late,tx.by_tries[1]),(1,1,2),"{tx:?}");
    let placed=tx.clock.iter().chain(tx.tsf.iter()).sum::<u32>();
    assert_eq!(placed!=0,detail,"releases placed in the beacon interval: {tx:?}");
    drop(service);clean();
});}}

// 213-DRV-0091: the calibrations' lines say PASS or FAIL by Linux's criteria (rtw8852b_rfk.c): DPK's sync at most 200 of
// DC on I and Q and at least 170 of correlation; IQK's LOK in range and no one-shot failing.
#[test]
fn calibration_results_are_judged_by_linuxs_criteria(){
    use crate::receive_service::CalibrationPath;
    let good=CalibrationPath{dpk_correlation:170,dpk_dc:[200,200],..Default::default()};assert!(good.dpk_ok()&&good.iqk_ok());
    assert!(!CalibrationPath{dpk_correlation:169,..good}.dpk_ok());assert!(!CalibrationPath{dpk_dc:[201,0],..good}.dpk_ok());
    assert!(!CalibrationPath{dpk_dc:[0,201],..good}.dpk_ok());
    for bad in[CalibrationPath{lok_range_fail:true,..good},CalibrationPath{tx_fails:1,..good},CalibrationPath{rx_fails:1,..good},
        CalibrationPath{lok_fails:1,..good},CalibrationPath{restore_fail:true,..good}]{assert!(!bad.iqk_ok(),"{bad:?}");}
}

// 213-DRV-0091: every step of SER_DBG_INFO is placed at the check that sees it, with the time since the check before and
// the ring's polls so far; the minute line gives RXQ's and RPQ's longest gap between two polls and the most waiting at one.
#[test]
fn service_places_each_step_of_ser_dbg_info_and_counts_the_rings_polls(){run(0,true,true,|data,c|{
    use crate::receive_service::Event;
    ipc::reset();let mut service=Service::new(data).unwrap();assert!(service.step());
    assert!(service.take_ser_step().is_none(),"no step, none placed");
    // Three frames wait at one poll.
    {let mut c=c.borrow_mut();for _ in 0..3{c.incoming.push(packet());}}ipc::advance(20);assert!(service.step());
    // A silent recovery: SER_DBG_INFO steps from 0 to 1; the next check places it.
    {let mut c=c.borrow_mut();c.ser=true;c.ser_heal=Some(2);}
    let mut step=None;
    for _ in 0..10{ipc::advance(20);assert!(service.step());step=step.or(service.take_ser_step());if step.is_some(){break}}
    let Some(s)=step else{panic!("no step placed")};
    assert_eq!((s.count,s.from,s.to),(1,0,1));assert!(s.since_us>0&&s.since_us<1_000_000,"{s:?}");
    assert!(s.polls[0].most_waiting>=3,"{s:?}");
    // The minute line takes the polls; the next minute's start again.
    let mut traffic=None;
    for _ in 0..70{c.borrow_mut().incoming.push(packet());ipc::advance(900);assert!(service.step());
        if let Some(e@Event::Traffic{..})=service.take_event(){traffic=Some(e);break}}
    let Some(Event::Traffic{polls:[q,_],..})=traffic else{panic!("no traffic line")};
    // The model's bus clock moves by its own delays, not by the service's minute: the gap is the model's.
    assert!(q.most_waiting>=3&&q.longest_gap_us>0&&q.longest_at_us>0,"{q:?}");
    assert!(q.over_10ms<=q.over_5ms,"{q:?}");
    let mut next=None;
    for _ in 0..70{c.borrow_mut().incoming.push(packet());ipc::advance(900);assert!(service.step());
        if let Some(e@Event::Traffic{..})=service.take_event(){next=Some(e);break}}
    let Some(Event::Traffic{polls:[q,_],..})=next else{panic!("no second traffic line")};
    assert!(q.most_waiting<=1,"the second minute's own: {q:?}");
    let _=Event::SerStep{count:0,from:0,to:0,since_us:0,polls:Default::default()};drop(service);clean();
});}

// 213-DRV-0091: a station check that meets an unreadable CMAC window (DEADBEEF, the Ryzen's boot 62 twice in 98 minutes)
// reads again and goes on, logged; a firmware recovery that begins with it is outlasted first; one that never answers,
// or answers with a mismatch, still stops the data mode.
#[test]
fn service_a_check_that_meets_an_unreadable_cmac_window_reads_again_and_goes_on(){run(0,true,true,|data,c|{
    use crate::receive_service::Event;
    ipc::reset();let mut service=Service::new(data).unwrap();assert!(service.step());
    // The check first: one table's reads answer DEADBEEF, then the window answers.
    c.borrow_mut().window_dead=8;ipc::advance(20);assert!(service.step());
    let Some(u)=service.take_unreadable()else{panic!("no unreadable check")};
    assert_eq!((u.count,u.value,u.recovered),(1,0xdead_beef,false));assert!((1..=16).contains(&u.reads),"{u:?}");
    assert!((0x40000..0x40020).contains(&u.register),"{u:?}");
    // The check first, and the firmware's recovery begins with it: outlasted, then the check made again.
    {let mut c=c.borrow_mut();c.window_dead=8;c.window_dead_ser=true;c.ser_heal=Some(2);}ipc::advance(20);assert!(service.step());
    let Some(u)=service.take_unreadable()else{panic!("no unreadable check after a recovery")};
    assert_eq!(u.count,2);assert!(u.recovered,"{u:?}");assert_ne!(u.ser_after,u.ser_before);
    assert!(service.take_recovery().is_some(),"the recovery counted");
    // The recovery first: outlasted before the checks, then a dead read is met and read again.
    {let mut c=c.borrow_mut();c.ser=true;c.ser_heal=Some(2);c.window_dead=8;}ipc::advance(20);assert!(service.step());
    assert!(service.take_recovery().is_some());
    let Some(u)=service.take_unreadable()else{panic!("no unreadable check after the recovery")};assert_eq!(u.count,3);
    // Traffic runs on.
    c.borrow_mut().incoming.push(packet());let id=ipc::buffer(5,&[]);assert!(service.step());assert_eq!(words(id)[0]&255,0);
    // No answer at all: the data mode stops on the check, as before.
    c.borrow_mut().window_dead=1000;ipc::advance(20);assert!(!service.step());
    let stopped=service.stop();assert!(matches!(stopped.failure,Some(Fatal::Radio(_))),"{:?}",stopped.failure.is_some());
    let _=Event::Unreadable{count:0,register:0,value:0,reads:0,waited_us:0,ser_before:0,ser_after:0,recovered:false};clean();
});}
