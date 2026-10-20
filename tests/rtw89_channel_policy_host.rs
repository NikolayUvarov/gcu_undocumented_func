#![allow(dead_code)]
#[path="../rtw89_wifi/src/io.rs"]mod io;
#[path="../rtw89_wifi/src/registers.rs"]mod registers;
#[path="../rtw89_wifi/src/power.rs"]mod power;
#[path="../rtw89_wifi/src/efuse.rs"]mod efuse;
#[path="../rtw89_wifi/src/board.rs"]mod board;
#[path="../rtw89_wifi/src/channel_policy.rs"]mod channel_policy;
#[path="../rtw89_wifi/src/power_tables.rs"]mod power_tables;
use channel_policy::*;

fn board(country:[u8;2],plan:u8)->board::Board{
    board::Board{identity:board::Identity{cut:1,rfe:1,mac:[2,1,2,3,4,5],xtal:32,country,channel_plan:plan},
        maps:efuse::Maps{logical:[255;2048],dav_logical:[255;16],phycap:[255;128]}}
}

#[test]
fn exact_country_groups_preserve_heterogeneous_bands_and_flags(){
    for (cc,groups,flags) in [(*b"US",[2,2,2],1),(*b"RU",[1,1,4],0),(*b"UZ",[1,1,1],0),
                            (*b"GB",[14,14,14],0),(*b"JP",[3,3,3],0),(*b"DE",[1,1,1],2)]{
        let c=country_default(cc).unwrap();assert_eq!(c.groups.map(Group::id),groups);assert_eq!(c.function_flags,flags);
        assert_eq!(board_country(&board(cc,0x7f)).unwrap().hint,Hint::Programmed(c));
    }
}
#[test]
fn worldwide_erased_unknown_and_lowercase_never_select_a_country(){
    for cc in [*b"00",[255,255],[0,0],*b"ZZ",*b"us",[0,255]]{
        let observed=board_country(&board(cc,0xff)).unwrap();
        assert_eq!(observed.hint,Hint::ExternalPolicyRequired);assert_eq!(observed.raw,cc);assert_eq!(observed.raw_channel_plan,255);
    }
}
#[test]
fn raw_channel_plan_is_preserved_without_inventing_a_frequency_mapping(){
    let expected=board_country(&board(*b"JP",0)).unwrap().hint;
    for plan in 0..=255{let value=board_country(&board(*b"JP",plan)).unwrap();assert_eq!(value.raw_channel_plan,plan);assert_eq!(value.hint,expected);}
}
#[test]
fn unsupported_board_and_group_ids_are_rejected(){
    for(cut,rfe)in[(0,1),(2,1),(1,0),(1,2)]{let mut b=board(*b"US",0);b.identity.cut=cut;b.identity.rfe=rfe;assert_eq!(board_country(&b),Err(Error::Board));}
    for id in 16..=255{assert!(Group::from_id(id).is_none());}
}
#[test]
fn source_limits_keep_stream_beamforming_and_channel_dimensions(){
    assert_eq!(default_limit_2g(0,1,0,false,Group::FCC,1).unwrap().quarter_dbm,78);
    assert_eq!(default_limit_2g(0,2,0,false,Group::FCC,1).unwrap().quarter_dbm,74);
    assert_eq!(default_limit_2g(0,2,2,true,Group::FCC,1).unwrap().quarter_dbm,58);
    assert_eq!(default_limit_2g(0,1,0,false,Group::MKK,14).unwrap().quarter_dbm,76);
    assert_eq!(default_limit_2g(0,1,1,false,Group::MKK,14).unwrap().quarter_dbm,127);
}
#[test]
fn explicit_zero_and_implicit_zero_both_follow_source_worldwide_fallback(){
    let implicit=default_limit_2g(0,1,0,false,Group::from_id(4).unwrap(),1).unwrap();
    assert_eq!(implicit.requested,Entry{value:0,explicit:false});assert!(implicit.used_worldwide);assert_eq!(implicit.quarter_dbm,56);
    let explicit=default_limit_2g(1,1,0,false,Group::WORLDWIDE,1).unwrap();
    assert_eq!(explicit.requested,Entry{value:0,explicit:true});assert!(explicit.used_worldwide);assert_eq!(explicit.quarter_dbm,0);
    let ru=default_ru_2g(0,1,Group::WORLDWIDE,14).unwrap();assert!(ru.requested.explicit&&ru.used_worldwide);assert_eq!(ru.quarter_dbm,0);
}
#[test]
fn all_default_data_keys_are_unique_ordered_and_fit_reviewed_dimensions(){
    for(rows,end)in[(power_tables::LIMIT_2G,5376),(power_tables::RU_2G,2240),
                    (power_tables::BYRATE_2G,120),(power_tables::SHAPE_2G,32),(power_tables::SHAPE_RU_2G,16)]{
        assert!(rows.windows(2).all(|w|w[0].0<w[1].0));assert!(rows.iter().all(|v|v.0<end));
    }
    assert_eq!(power_tables::COUNTRIES.len(),242);assert!(power_tables::COUNTRIES.windows(2).all(|w|w[0].0<w[1].0));
    assert_eq!(power_tables::LIMIT_2G.len(),2184);assert_eq!(power_tables::RU_2G.len(),1092);
}
#[test]
fn packed_byrate_bytes_keep_source_order_and_all_five_offsets(){
    let got=(0..8).map(|i|default_byrate_2g(1,1,i).unwrap().value).collect::<Vec<_>>();assert_eq!(got,[80,80,80,80,80,80,76,72]);
    for n in 1..=2{assert_eq!((0..12).map(|i|default_byrate_2g(n,2,i).unwrap().value).collect::<Vec<_>>(),[80,80,80,80,80,76,72,68,64,60,56,52]);}
    for i in 0..5{assert_eq!(default_byrate_2g(1,4,i).unwrap(),Entry{value:0,explicit:true});}
}
#[test]
fn transmit_shape_uses_region_and_rate_without_worldwide_substitution(){
    assert_eq!(default_shape_2g(0,Group::FCC).unwrap().value,1);assert_eq!(default_shape_2g(1,Group::FCC).unwrap().value,3);
    assert_eq!(default_ru_shape_2g(Group::FCC).value,3);assert_eq!(default_shape_2g(0,Group::ETSI).unwrap(),Entry{value:0,explicit:true});
    assert_eq!(default_shape_2g(0,Group::WORLDWIDE).unwrap(),Entry{value:0,explicit:false});
}
#[test]
fn invalid_coordinates_never_fall_back_to_channel_one_or_another_rate(){
    for ch in [0,15,255]{assert_eq!(default_limit_2g(0,1,0,false,Group::FCC,ch),Err(Error::Index));assert_eq!(default_ru_2g(0,1,Group::FCC,ch),Err(Error::Index));}
    for(bw,n,rs)in[(2,1,0),(0,0,0),(0,3,0),(0,1,3),(255,255,255)]{assert!(default_limit_2g(bw,n,rs,false,Group::FCC,1).is_err());}
    for(ru,n)in[(3,1),(4,1),(0,0),(0,3)]{assert!(default_ru_2g(ru,n,Group::FCC,1).is_err());}
    for(n,rs,i)in[(2,0,0),(2,1,0),(2,4,0),(1,0,4),(1,1,8),(1,2,12),(1,3,4),(1,4,5),(1,5,0),(255,255,255)]{assert!(default_byrate_2g(n,rs,i).is_err());}
    assert!(default_shape_2g(2,Group::FCC).is_err());
}
#[test]
fn signed_power_units_round_down_and_saturate_only_the_source_dbm_conversion(){
    for value in -128i16..=127{assert_eq!(rf_to_mac(value as i8)as i16,value.div_euclid(2));assert_eq!(dbm_to_mac(value as i8)as i16,(value*2).clamp(-64,63));}
    assert_eq!(rf_to_mac(-1),-1);assert_eq!(rf_to_mac(127),63);assert_eq!(dbm_to_mac(-33),-64);
}
#[test]
fn constraints_combine_without_wrapping_or_assuming_missing_platform_inputs(){
    assert_eq!(constrained_mac(78,9,None,63,63),Ok(39));
    assert_eq!(constrained_mac(78,9,Some(80),63,63),Ok(40));
    assert_eq!(constrained_mac(78,-9,Some(80),63,63),Ok(34));
    assert_eq!(constrained_mac(78,9,Some(80),30,20),Ok(20));
    assert_eq!(constrained_mac(-1,0,None,63,63),Ok(-1));
    for sentinel in [0,127]{assert_eq!(constrained_mac(78,9,Some(sentinel),63,63),Ok(39));}
    assert_eq!(constrained_mac(-128,-1,Some(80),63,63),Err(Error::PowerRange));
    for(sar,tpe)in[(64,63),(63,-65)]{assert_eq!(constrained_mac(0,0,None,sar,tpe),Err(Error::PowerRange));}
}
