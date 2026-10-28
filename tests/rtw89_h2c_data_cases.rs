use super::*;

#[test]
fn active_transition_requires_actual_command_dma_idle_before_serial_enable(){run(15,true,true,|_,_|panic!("busy command DMA exposed active owner"));}

#[test]
fn actual_data_owner_wraps_twice_with_receipt_each_time_and_continuous_wifi(){run(0,true,true,|data,c|{
    {let mut c=c.borrow_mut();c.continuous_wifi=8192;c.old=1;}
    let request=h2c::Command{category:2,class:10,function:2};
    for n in 0..514{
        assert_eq!(data.command(request,&[0;20],h2c::Acks{received:false,done:false},100_000).unwrap(),h2c::Outcome::Received);
        let c=c.borrow();assert!(c.continuous_wifi>0);assert_eq!(c.wifi_pending,1);
        assert_eq!(c.wifi_max_pending,2);assert!(c.wifi_refills>=n+1);
    }
    {let c=c.borrow();assert_eq!(c.normal_packets.len(),514);let first=c.normal_packets[0][3];
        for(n,packet)in c.normal_packets.iter().enumerate(){assert_eq!(packet[3],first.wrapping_add(n as u8));assert_eq!(packet[5]&0xc0,0x40);}
        assert!(c.continuous_wifi>0);
    }
    assert!(data.counts().pressure>0);data.begin(&frame(),12,1,100_000).unwrap();
    for _ in 0..64{if let Some(done)=data.advance(100_000).unwrap(){assert_eq!(done.attempts,tx::AttemptCount::Known(1));return}}
    panic!("actual TX did not complete after wrapped command traffic");
});}

#[test]
fn wrapped_runtime_ack_without_command_dma_consumption_cannot_release_owner(){run(0,true,true,|data,c|{
    for _ in 0..260{command(data);}
    c.borrow_mut().hold_command_dma=true;
    let failure=data.command(h2c::Command{category:2,class:10,function:2},&[0;20],h2c::Acks{received:true,done:true},1000).unwrap_err();
    assert!(failure.shutdown.is_some());let count=c.borrow().normal_packets.len();assert_eq!(count,261);
    assert!(data.command(h2c::Command{category:2,class:10,function:2},&[0;20],h2c::Acks{received:true,done:true},1000).is_err());
    assert_eq!(c.borrow().normal_packets.len(),count);
});}
