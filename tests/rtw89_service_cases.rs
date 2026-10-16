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
    assert_eq!(info.bands,5);assert_eq!(info.channels.as_slice(),&[6]);assert_eq!(info.channel,6);assert_eq!(info.frame_max,2346);
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
    ipc::reset();let mut service=Service::new(data).unwrap();let ring=ipc::enqueue([7|1<<8,0],abi::CAP_KIND_MEMORY,&[],true);
    assert!(service.step());error(ring,wlanhw::Error::Invalid);assert_eq!(ipc::dropped(ring),1);
    let kick=ipc::words(8,0);assert!(service.step());error(kick,wlanhw::Error::Invalid);
    for words in[[1|2<<8,0],[99|1<<8,0],[2|1<<8|6<<16,0]]{
        let id=ipc::enqueue(words,abi::CAP_KIND_MEMORY,&[],true);assert!(service.step());assert!(self::words(id)[0]>=wire::STATUS_INVALID);assert_eq!(ipc::dropped(id),1);
    }
    let noncall=ipc::enqueue([7|1<<8,0],abi::CAP_KIND_MEMORY,&[],false);assert!(service.step());assert!(ipc::response(noncall).is_none());assert_eq!(ipc::dropped(noncall),1);
    drop(service);clean();
});}

#[test]
fn service_continuous_ipc_and_rx_each_progress_with_fixed_storage(){run(0,true,true,|data,c|{
    ipc::reset();let mut service=Service::new(data).unwrap();c.borrow_mut().incoming=vec![packet();20];
    for _ in 0..20{let id=ipc::buffer(1,&[]);assert!(service.step());assert!(ipc::response(id).is_some());}
    for _ in 0..4{let id=ipc::buffer(5,&[]);assert!(service.step());assert_eq!(words(id)[0]&255,0);}
    assert_eq!(ipc::receives().len(),24);assert!(ipc::receives().iter().all(|(_,ms)|*ms==2));
    let stopped=service.stop();assert!(stopped.failure.is_none());assert!(stopped.owner.counts().pressure>0);clean();
});}

#[test]
fn service_no_final_tx_report_times_out_answers_every_owner_and_retains_data(){run(7,true,true,|data,c|{
    ipc::reset();let mut service=Service::new(data).unwrap();let wait=ipc::words(6,0);assert!(service.step());
    let send=ipc::buffer(4,&body(&frame(),12,1));assert!(service.step());let busy=ipc::buffer(4,&body(&frame(),12,1));
    assert!(service.step());error(busy,wlanhw::Error::Busy);ipc::advance(101);assert!(!service.step());
    error(send,wlanhw::Error::Failed);assert_eq!(words(wait),[0,0]);assert_eq!(c.borrow().published,1);
    let stopped=service.stop();assert!(matches!(stopped.failure,Some(Fatal::TransmitTimeout)));assert!(stopped.owner.poll_packets(10_000).is_err());clean();
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
