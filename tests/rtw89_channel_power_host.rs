#![allow(dead_code)]
#[path="../rtw89_wifi/src/io.rs"]mod io;
#[path="../rtw89_wifi/src/registers.rs"]mod registers;
#[path="../rtw89_wifi/src/power.rs"]mod power;
#[path="../rtw89_wifi/src/efuse.rs"]mod efuse;
#[path="../rtw89_wifi/src/board.rs"]mod board;
#[path="../rtw89_wifi/src/channel_policy.rs"]mod channel_policy;
#[path="../rtw89_wifi/src/power_tables.rs"]mod power_tables;
#[path="../rtw89_wifi/src/channel_power.rs"]mod channel_power;
use channel_power::*;
use channel_policy::{Group,Error};
fn limits()->Constraints{Constraints{per_ntx:[40,34],per_path:[40,40]}}
fn word(image:&Image,at:usize)->u32{image.writes().iter().find(|w|w.at==at).unwrap().bits}
#[test]
fn exact_source_register_regions_have_no_overlap_and_fit_bar(){
    let image=build(Group::FCC,6,limits()).unwrap();let writes=image.writes();assert_eq!(writes.len(),58);
    let unique:std::collections::BTreeSet<_>=writes.iter().map(|w|w.at).collect();assert_eq!(unique.len(),58);
    assert!(writes.iter().all(|w|w.at%4==0&&w.at+4<=0x20000&&w.bits&!w.mask==0));
    for(start,end,count)in[(0xd2c0,0xd2e8,11),(0xd2ec,0xd338,20),(0xd33c,0xd368,12),(0x12300,0x1231c,8)]{
        assert_eq!(writes.iter().filter(|w|(start..=end).contains(&w.at)).count(),count);
    }
}
#[test]
fn packed_rates_keep_little_endian_signed_units_and_source_section_order(){
    let image=build(Group::FCC,1,limits()).unwrap();
    assert_eq!(word(&image,0xd2c0),0x28282828);assert_eq!(word(&image,0xd2c4),0x28282828);
    assert_eq!(word(&image,0xd2c8),0x24262828);assert_eq!(word(&image,0xd2d0),0x22242628);
    assert_eq!(word(&image,0xd2d4),0x1a1c1e20);assert_eq!(word(&image,0xd204),0);
}
#[test]
fn one_and_two_stream_pages_apply_independent_caps_and_zero_unused_twenty_mhz_slots(){
    let image=build(Group::FCC,1,Constraints{per_ntx:[20,10],per_path:[20,20]}).unwrap();
    for(base,cap)in[(0xd2ec,20),(0xd314,10)]{
        // Source layout is CCK20[nonBF,BF], CCK40[nonBF,BF], OFDM[nonBF,BF], MCS20[nonBF,BF].
        assert_eq!(word(&image,base).to_le_bytes(),[cap,0,cap,0]);
        assert_eq!(word(&image,base+4).to_le_bytes(),[cap,0,cap,if base==0xd314{cap}else{0}]);
        for off in (8..40).step_by(4){assert_eq!(word(&image,base+off),0);}
    }
    for(base,cap)in[(0xd33c,20),(0xd354,10)]{
        for off in [0,8,16]{assert_eq!(word(&image,base+off),cap as u32);assert_eq!(word(&image,base+off+4),0);}
    }
}
#[test]
fn negative_caps_remain_signed_bytes_including_legitimate_all_ones_words(){
    let image=build(Group::ETSI,6,Constraints{per_ntx:[-1,-64],per_path:[0,0]}).unwrap();
    assert_eq!(word(&image,0xd2f0),u32::MAX);assert_eq!(word(&image,0xd318),0xc0c0c0c0);
    assert_eq!(word(&image,0xd33c),255);assert_eq!(word(&image,0xd354),192);
}
#[test]
fn region_shape_selects_real_dfir_and_triangular_filter(){
    let fcc=build(Group::FCC,1,limits()).unwrap();let etsi=build(Group::ETSI,1,limits()).unwrap();
    assert_eq!(word(&fcc,0x12300),0x023d83ff);assert_eq!(word(&etsi,0x12300),0x023d23ff);
    assert_eq!(word(&fcc,0x1231c),0x00003ff5);assert_eq!(word(&etsi,0x1231c),0x00ffdff5);
    assert_eq!(word(&fcc,0x14494),3<<24);assert_eq!(word(&etsi,0x14494),0);
}
#[test]
fn reference_difference_decreases_only_weaker_path_and_preserves_ofdm_cck_pairs(){
    for(a,b)in[(40,34),(34,40)]{
        let image=build(Group::ETSI,6,Constraints{per_ntx:[30,24],per_path:[a,b]}).unwrap();
        for(at,dec)in[(0x15804,if a<b{24}else{0}),(0x17804,if b<a{24}else{0})]{
            let value=word(&image,at);assert_eq!((value>>18)&511,172-dec);assert_eq!((value>>9)&511,312-dec);
            assert_eq!(value&511,0);assert_eq!(word(&image,at+4),value);
        }
    }
}
#[test]
fn extreme_path_difference_refuses_before_unsigned_tssi_wrap(){
    for(a,b)in[(-64,63),(63,-64),(0,44),(44,0)]{assert_eq!(build(Group::ETSI,1,Constraints{per_ntx:[0,0],per_path:[a,b]}).unwrap_err(),Error::PowerRange);}
    assert!(build(Group::ETSI,1,Constraints{per_ntx:[0,0],per_path:[0,43]}).is_ok());
}
#[test]
fn unknown_channels_and_out_of_range_caps_never_generate_an_image(){
    for ch in [0,14,15,255]{assert_eq!(build(Group::ETSI,ch,limits()).unwrap_err(),Error::Index);}
    for value in [-128,-65,64,127]{for path in [false,true]{let mut c=limits();if path{c.per_path[0]=value;}else{c.per_ntx[1]=value;}
        assert_eq!(build(Group::ETSI,1,c).unwrap_err(),Error::PowerRange);}}
}

// 801-DRV-0071: the most a rate sends at, per NTX, from the same tables `build` writes.
fn written_maximum(image:&Image,streams:usize)->i8{
    let rates:Vec<i8>=(0..11).flat_map(|i|word(image,0xd2c0+4*i).to_le_bytes()).map(|b|b as i8).collect();
    let page:Vec<i8>=(0..10).flat_map(|i|word(image,0xd2ec+40*(streams-1)+4*i).to_le_bytes()).map(|b|b as i8).collect();
    // By-rate bytes: CCK 0..4, OFDM 4..12, MCS 1SS 12..24, HE-DCM 1SS 24..28, MCS 2SS 28..40, HE-DCM 2SS 40..44.
    let(cck,ofdm,mcs)=(page[0],page[4],page[6]);
    let sections:[(&[i8],i8);4]=if streams==1{[(&rates[0..4],cck),(&rates[4..12],ofdm),(&rates[12..24],mcs),(&rates[24..28],mcs)]}
        else{[(&rates[0..4],cck),(&rates[4..12],ofdm),(&rates[28..40],mcs),(&rates[40..44],mcs)]};
    sections.iter().flat_map(|(r,cap)|r.iter().map(move|v|(*v).min(*cap))).max().unwrap()
}
#[test]
fn an_unknown_countrys_cap_is_its_maximum_less_three_half_db_steps(){
    assert_eq!(UNKNOWN_COUNTRY_STEPS,3,"75 % is -1.249 dB: 2 steps (-1 dB, 79.4 %) would pass it, 3 (-1.5 dB, 70.8 %) do not");
    let world=Constraints{per_ntx:[40,40],per_path:[63,63]};
    let mut seen=vec![];
    for channel in 1..=13{
        let most=maximum(Group::WORLDWIDE,channel,world).unwrap();
        let plain=build(Group::WORLDWIDE,channel,world).unwrap();
        assert_eq!(most,[written_maximum(&plain,1),written_maximum(&plain,2)],"channel {channel}: the maximum is what the MAC is given");
        let(capped,again)=unknown_country(Group::WORLDWIDE,channel,world).unwrap();assert_eq!(again,most);
        assert_eq!(capped.per_ntx,[most[0]-3,most[1]-3],"channel {channel}");assert_eq!(capped.per_path,world.per_path);
        let image=build(Group::WORLDWIDE,channel,capped).unwrap();
        assert_eq!([written_maximum(&image,1),written_maximum(&image,2)],[most[0]-3,most[1]-3],"channel {channel}: held exactly there");
        seen.push((channel,most));
    }
    // The world group's maxima: 14.5 dBm on one TX on channels 1 to 11, so 13 dBm under the cap.
    let wanted:Vec<(u8,[i8;2])>=(1..=13).map(|ch|(ch,match ch{12=>[28,23],13=>[28,21],_=>[29,23]})).collect();
    assert_eq!(seen,wanted);
    // A lower EIRP than the tables is the maximum itself: the cap is 3 below it.
    let low=Constraints{per_ntx:[20,20],per_path:[63,63]};
    assert_eq!(unknown_country(Group::WORLDWIDE,6,low).unwrap(),(Constraints{per_ntx:[17,17],per_path:[63,63]},[20,20]));
}
