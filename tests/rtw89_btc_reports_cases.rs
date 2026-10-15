use super::*;
fn packet(category:u8,class:u8,function:u8,body:&[u8])->Vec<u8>{
    let mut out=Vec::new();out.extend_from_slice(&(category as u32|(class as u32)<<2|(function as u32)<<8).to_le_bytes());
    out.extend_from_slice(&((8+body.len())as u32).to_le_bytes());out.extend_from_slice(body);out
}
fn report(kind:u8,body:&[u8])->Vec<u8>{let mut v=vec![kind];v.extend_from_slice(&(body.len()as u16).to_le_bytes());v.extend_from_slice(body);v}
fn slots()->Vec<u8>{let mut b=vec![1,18,0xa5,0x5a];b.extend_from_slice(&0x80020001u32.to_le_bytes());
    for i in 0..18u16{b.extend_from_slice(&(100+i).to_le_bytes());b.extend_from_slice(&(0x12340000+i as u32).to_le_bytes());b.extend_from_slice(&(i%2).to_le_bytes());}b}
fn btver()->Vec<u8>{let mut b=vec![1,9,8,7];for v in [0x12345678u32,0x89abcdef,0xfedcba98]{b.extend_from_slice(&v.to_le_bytes());}b}

#[test]
fn complete_reports_preserve_all_tdma_fields_slots_and_version_without_alignment_assumptions(){
    let tdma=[3,0x55,0xaa,0xff,0,1,2,3,4,5,6,0x81];let mut b=report(1,&tdma);b.extend(report(2,&slots()));b.extend(report(8,&btver()));
    let p=packet(2,0x12,0,&b);let Event::Reports(reports)=parse_105(&p).unwrap()else{panic!()};let mut it=reports.iter();
    let Report::Tdma(t)=it.next().unwrap()else{panic!()};assert_eq!(t.fields,[0,1,2,3,4,5,6,0x81]);
    let Report::Slots(s)=it.next().unwrap()else{panic!()};assert_eq!((s.count(),s.update_map()),(18,0x80020001));
    for i in 0..18{assert_eq!(s.get(i),Some(Slot{duration:100+i as u16,table:0x12340000+i as u32,kind:i as u16%2}));}
    assert!(s.get(18).is_none()&&s.get(usize::MAX).is_none());
    let Report::BtVersion(v)=it.next().unwrap()else{panic!()};assert_eq!(v,BtVersion{coex_version:0x12345678,firmware_version:0x89abcdef,features:0xfedcba98});
    assert!(it.next().is_none()&&it.next().is_none());
}
#[test]
fn legacy_report_id_mapping_keeps_unknown_records_explicit(){
    let mut b=Vec::new();for ty in [0,3,4,5,6,7,9,10,11,12,13,255]{b.extend(report(ty,&[0xa5,0x5a]));}
    let p=packet(2,0x12,0,&b);let Event::Reports(r)=parse_105(&p).unwrap()else{panic!()};
    for item in r.iter(){let Report::Other{wire_type,canonical_type,bytes}=item else{panic!()};
        assert_eq!(canonical_type,match wire_type{0..=5=>Some(wire_type),6..=12=>Some(wire_type+1),_=>None});assert_eq!(bytes,[0xa5,0x5a]);}
}
#[test]
fn wrong_version_or_length_of_every_typed_report_is_rejected_before_iteration(){
    for(kind,body)in[(1,vec![3;12]),(2,slots()),(8,btver())]{
        let mut version=body.clone();version[0]=7;let p=packet(2,0x12,0,&report(kind,&version));assert!(matches!(parse_105(&p),Err(Error::Version)));
        for n in 0..body.len(){let p=packet(2,0x12,0,&report(kind,&body[..n]));assert!(matches!(parse_105(&p),Err(Error::Length)));}
        let mut long=body;long.push(0);let p=packet(2,0x12,0,&report(kind,&long));assert!(matches!(parse_105(&p),Err(Error::Length)));
    }
    let mut b=slots();b[1]=19;assert!(matches!(parse_105(&packet(2,0x12,0,&report(2,&b))),Err(Error::Slots)));
}
#[test]
fn malformed_suffix_invalidates_whole_event_and_all_truncations_are_bounded(){
    let valid=report(1,&[3;12]);for tail in [&[0][..],&[0,0],&[0,1,0]]{
        let mut b=valid.clone();b.extend_from_slice(tail);assert!(parse_105(&packet(2,0x12,0,&b)).is_err());}
    let p=packet(2,0x12,0,&valid);for n in 0..p.len(){assert!(parse_105(&p[..n]).is_err());}
    let mut b=valid;b.extend(report(8,&[7;16]));assert!(matches!(parse_105(&packet(2,0x12,0,&b)),Err(Error::Version)));
}
#[test]
fn exact_body_bound_and_zero_length_unknown_records_always_make_progress(){
    let body=[0;MAX_BODY];let p=packet(2,0x12,0,&body);let Event::Reports(r)=parse_105(&p).unwrap()else{panic!()};
    assert_eq!(r.iter().count(),600);assert!(matches!(parse_105(&packet(2,0x12,0,&[0;MAX_BODY+1])),Err(Error::TooLong)));
    let p=packet(2,0x12,0,&[]);let Event::Reports(r)=parse_105(&p).unwrap()else{panic!()};assert_eq!(r.iter().count(),0);
}
#[test]
fn btinfo_and_scoreboard_remain_visible_and_btinfo_shape_is_checked(){
    let body=[4,6,0xff,0xee,0xdd,0xcc,0xbb,0xaa];let p=packet(2,0x12,1,&body);
    let Event::BtInfo(actual)=parse_105(&p).unwrap()else{panic!()};assert_eq!(actual,&body);
    for n in 0..8{assert!(matches!(parse_105(&packet(2,0x12,1,&body[..n])),Err(Error::BtInfo)));}
    let mut bad=body;bad[1]=5;assert!(matches!(parse_105(&packet(2,0x12,1,&bad)),Err(Error::BtInfo)));
    let p=packet(2,0x12,2,&body);let Event::Scoreboard(actual)=parse_105(&p).unwrap()else{panic!()};assert_eq!(actual,body);
}
#[test]
fn profile105_overflow_loopback_and_unhandled_events_are_not_confused_with_reports(){
    let bytes=[1,2,3,4];for function in 3..=10{let p=packet(2,0x12,function,&bytes);let event=parse_105(&p).unwrap();
        assert!(match function{3=>matches!(event,Event::BtRegister(_)),4=>matches!(event,Event::RunInfo(_)),5=>matches!(event,Event::Psd(_)),
            6=>matches!(event,Event::DeviceInfo(_)),7=>matches!(event,Event::Overflow(_)),8=>matches!(event,Event::Loopback(_)),_=>matches!(event,Event::Other{..})});}
    assert!(matches!(parse_105(&packet(2,0x12,3,&[0;3])),Err(Error::Truncated)));
    assert!(matches!(parse_105(&packet(2,0x12,8,&[])),Err(Error::Truncated)));
    for(category,class)in[(1,0x12),(2,0x10),(2,0x13)]{let p=packet(category,class,1,&bytes);
        let Event::Other{category:c,class:k,function,bytes:b}=parse_105(&p).unwrap()else{panic!()};assert_eq!((c,k,function,b),(category,class,1,&bytes[..]));}
}
