use super::*;
use crate::c2h::Ack;
const COMMAND:Command=Command{category:2,class:16,function:5};
const NONE:Acks=Acks{received:false,done:false};
const BOTH:Acks=Acks{received:true,done:true};
#[derive(Default)]
struct Sender{now:u64,count:usize,header:[u8;8],full:bool}
impl Transport for Sender{
    fn now_us(&mut self)->u64{self.now}
    fn submit(&mut self,packet:&[u8],_:u32)->Result<(),fwcmd::Error>{
        if self.full{return Err(fwcmd::Error::Full)}self.count+=1;self.header.copy_from_slice(&packet[..8]);Ok(())
    }
}
fn done(ticket:Ticket,status:u8)->Ack{Ack::Done{command:ticket.identity(),status}}
fn receipt(engine:&mut Engine,ticket:Ticket){engine.acknowledge(0,Ack::Received(ticket.identity())).unwrap();}

#[test]
fn serial_wrap_preserves_generation_and_real_count_past_u16(){
    let mut engine=Engine::new(0);let mut sender=Sender::default();engine.enable_serial(0).unwrap();let mut old=None;
    for count in 0..65_538u64{
        let ticket=engine.submit(&mut sender,COMMAND,&[],NONE,100).unwrap();
        assert_eq!(ticket.identity().sequence,count as u8);assert_eq!(ticket.generation,count);assert_ne!(sender.header[5]&0x40,0);
        if count==0{old=Some(ticket)}else if count%256==0{
            assert_eq!(ticket.identity(),old.unwrap().identity());assert_eq!(engine.progress(0,old.unwrap()),Err(Error::Unknown));assert_eq!(engine.take(0,old.unwrap()),Err(Error::Unknown));
        }
        assert_eq!(engine.take(0,ticket),Ok(None));receipt(&mut engine,ticket);assert_eq!(engine.take(0,ticket),Ok(Some(Outcome::Received)));
    }
    assert_eq!(engine.submitted(),65_538);assert_eq!(sender.count,65_538);
}

#[test]
fn serial_enable_requires_every_startup_slot_taken_without_resetting_sequence(){
    let mut engine=Engine::new(0);let mut sender=Sender::default();let ticket=engine.submit(&mut sender,COMMAND,&[],BOTH,100).unwrap();
    assert_eq!(engine.enable_serial(0),Err(Error::Full));receipt(&mut engine,ticket);assert_eq!(engine.enable_serial(0),Err(Error::Full));
    engine.acknowledge(0,done(ticket,0)).unwrap();assert_eq!(engine.enable_serial(0),Err(Error::Full));engine.take(0,ticket).unwrap();engine.enable_serial(0).unwrap();
    let next=engine.submit(&mut sender,COMMAND,&[],NONE,100).unwrap();assert_eq!(next.identity().sequence,1);assert_eq!(next.generation,1);
}

#[test]
fn serial_receipt_and_done_each_gate_reuse_even_when_completion_is_out_of_order(){for done_first in [false,true]{
    let mut engine=Engine::new(0);let mut sender=Sender::default();engine.enable_serial(0).unwrap();
    let ticket=engine.submit(&mut sender,COMMAND,&[],BOTH,100).unwrap();
    let first=if done_first{done(ticket,0)}else{Ack::Received(ticket.identity())};let second=if done_first{Ack::Received(ticket.identity())}else{done(ticket,0)};
    assert_eq!(engine.acknowledge(0,first),Ok(Observation::Progress));assert_eq!(engine.take(0,ticket),Ok(None));
    assert_eq!(engine.submit(&mut sender,COMMAND,&[],NONE,100),Err(Error::Full));assert_eq!(engine.submitted(),1);
    assert_eq!(engine.acknowledge(0,second),Ok(Observation::Complete));assert_eq!(engine.submit(&mut sender,COMMAND,&[],NONE,100),Err(Error::Full));
    assert_eq!(engine.take(0,ticket),Ok(Some(Outcome::Done)));assert!(engine.submit(&mut sender,COMMAND,&[],NONE,100).is_ok());
}}

#[test]
fn serial_rejected_done_keeps_receipt_pending_and_cannot_publish_a_new_command(){
    let mut engine=Engine::new(0);let mut sender=Sender::default();engine.enable_serial(0).unwrap();let ticket=engine.submit(&mut sender,COMMAND,&[],BOTH,100).unwrap();
    assert_eq!(engine.acknowledge(0,done(ticket,9)),Ok(Observation::Progress));assert_eq!(engine.take(0,ticket),Ok(None));
    assert_eq!(engine.submit(&mut sender,COMMAND,&[],NONE,100),Err(Error::Full));receipt(&mut engine,ticket);
    assert_eq!(engine.take(0,ticket),Ok(Some(Outcome::Rejected(9))));assert_eq!(sender.count,1);
}

#[test]
fn serial_full_and_exhausted_generation_never_publish_or_consume_the_counter(){
    let mut engine=Engine::new(0);let mut sender=Sender{full:true,..Sender::default()};engine.enable_serial(0).unwrap();
    assert_eq!(engine.submit(&mut sender,COMMAND,&[],NONE,100),Err(Error::Full));assert_eq!(engine.submitted(),0);
    sender.full=false;engine.next=u64::MAX;assert_eq!(engine.submit(&mut sender,COMMAND,&[],NONE,100),Err(Error::Exhausted));
    assert_eq!(engine.enable_serial(0),Err(Error::Exhausted));assert_eq!(sender.count,0);
}

#[test]
fn serial_missing_receipt_or_requested_done_remains_terminal_at_deadline(){for missing_done in [false,true]{
    let mut engine=Engine::new(0);let mut sender=Sender::default();engine.enable_serial(0).unwrap();let ticket=engine.submit(&mut sender,COMMAND,&[],BOTH,100).unwrap();
    if missing_done{receipt(&mut engine,ticket)}else{engine.acknowledge(0,done(ticket,0)).unwrap();}
    assert_eq!(engine.poll(100),Err(Error::Failed(Failure::Timeout)));assert_eq!(engine.enable_serial(100),Err(Error::Failed(Failure::Timeout)));
    sender.now=100;assert_eq!(engine.submit(&mut sender,COMMAND,&[],NONE,100),Err(Error::Failed(Failure::Timeout)));assert_eq!(sender.count,1);
}}

#[test]
fn serial_late_previous_ack_is_not_a_receipt_barrier_for_the_next_command(){
    let mut engine=Engine::new(0);let mut sender=Sender::default();engine.enable_serial(0).unwrap();
    let old=engine.submit(&mut sender,COMMAND,&[],BOTH,100).unwrap();receipt(&mut engine,old);engine.acknowledge(0,done(old,0)).unwrap();engine.take(0,old).unwrap();
    let current=engine.submit(&mut sender,COMMAND,&[],BOTH,100).unwrap();assert_eq!(engine.acknowledge(0,done(old,0)),Ok(Observation::Unrelated));
    assert_eq!(engine.acknowledge(0,Ack::Received(old.identity())),Ok(Observation::Unrelated));assert_eq!(engine.take(0,current),Ok(None));
    receipt(&mut engine,current);assert_eq!(engine.take(0,current),Ok(None));engine.acknowledge(0,done(current,0)).unwrap();assert_eq!(engine.take(0,current),Ok(Some(Outcome::Done)));
}
