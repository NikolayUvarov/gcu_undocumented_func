#![allow(dead_code)]
#[path="../rtw89_wifi/src/io.rs"]mod io;
#[path="../rtw89_wifi/src/registers.rs"]mod registers;
#[path="../rtw89_wifi/src/power.rs"]mod power;
#[path="../rtw89_wifi/src/efuse.rs"]mod efuse;
#[path="../rtw89_wifi/src/board.rs"]mod board;
#[path="../rtw89_wifi/src/calibration.rs"]mod calibration;

fn board()->board::Board{
    board::Board{identity:board::Identity{cut:1,rfe:1,mac:[2,1,2,3,4,5],xtal:32,country:*b"US",channel_plan:0x7f},
        maps:efuse::Maps{logical:[255;2048],dav_logical:[255;16],phycap:[255;128]}}
}
#[test]
fn erased_maps_preserve_each_distinct_upstream_sentinel_rule(){
    let r=calibration::decode(&board()).unwrap();
    assert!(!r.power_k_valid);assert_eq!(r.thermal_reference,[255;2]);assert_eq!(r.tssi_cck,[[-1;6];2]);assert_eq!(r.tssi_mcs,[[-1;19];2]);
    assert_eq!(r.tssi_trim,[[0;8];2]);assert_eq!(r.thermal_setting,None);assert_eq!(r.pa_bias_setting,None);
    assert!(!r.gain_offset_valid&&!r.gain_comp_valid);assert_eq!(r.gain_offset,[[-1;5];2]);assert_eq!(r.gain_comp,[[-1,-1,0,-1,-1];2]);
}
#[test]
fn logical_tssi_paths_skip_reserved_bytes_and_preserve_signed_data(){
    let mut b=board();for (i,byte) in b.maps.logical.iter_mut().enumerate(){*byte=(i%256)as u8;}
    let r=calibration::decode(&b).unwrap();assert_eq!(r.thermal_reference,[0xd0,0xd1]);
    assert_eq!(r.tssi_cck[0],[16,17,18,19,20,21]);assert_eq!(r.tssi_cck[1],[58,59,60,61,62,63]);
    assert_eq!(r.tssi_mcs[0],[22,23,24,25,26,34,35,36,37,38,39,40,41,42,43,44,45,46,47]);
    assert_eq!(r.tssi_mcs[1],[64,65,66,67,68,76,77,78,79,80,81,82,83,84,85,86,87,88,89]);
    b.maps.logical[0x210]=128;b.maps.logical[0x24c]=254;let r=calibration::decode(&b).unwrap();assert_eq!(r.tssi_cck[0][0],-128);assert_eq!(r.tssi_mcs[1][5],-2);
}
#[test]
fn one_programmed_tssi_trim_preserves_other_ff_entries_and_descending_addresses(){
    let mut b=board();b.maps.phycap[0x5d6-0x580]=128;b.maps.phycap[0x5d6-0x580-7]=127;b.maps.phycap[0x5ab-0x580-3]=254;
    let r=calibration::decode(&b).unwrap();assert_eq!(r.tssi_trim[0],[-128,-1,-1,-1,-1,-1,-1,127]);assert_eq!(r.tssi_trim[1],[-1,-1,-1,-2,-1,-1,-1,-1]);
}
#[test]
fn global_trim_flags_apply_both_paths_even_when_one_is_erased(){
    let mut b=board();b.maps.phycap[0x5df-0x580]=0x12;b.maps.phycap[0x5db-0x580]=0x8c;
    let r=calibration::decode(&b).unwrap();assert_eq!(r.thermal_raw,[0x12,255]);assert_eq!(r.thermal_setting,Some([9,15]));
    assert_eq!(r.pa_bias_raw,[255,0x8c]);assert_eq!(r.pa_bias_setting,Some([[15,15],[12,8]]));
}
#[test]
fn all_thermal_bytes_follow_bit_permutation_without_touching_reserved_high_bits(){
    for raw in 0u8..=255{
        let mut b=board();b.maps.phycap[0x5df-0x580]=raw;b.maps.phycap[0x5dc-0x580]=0;
        let got=calibration::decode(&b).unwrap().thermal_setting.unwrap()[0];
        let expected=((raw>>1)&15)|((raw&1)<<3);assert_eq!(got,expected);assert!(got<16);
    }
}
#[test]
fn gain_offsets_decode_both_nibbles_in_chip_band_order(){
    let mut b=board();for(at,value)in[(0x2d6,0x87),(0x2d4,0x1e),(0x2d8,0xf0),(0x2da,0x4b),(0x2dc,0x2d)]{b.maps.logical[at]=value;}
    let r=calibration::decode(&b).unwrap();assert!(r.gain_offset_valid);assert_eq!(r.gain_offset,[[-8,1,-1,4,2],[7,-2,0,-5,-3]]);
}
#[test]
fn physical_gain_uses_low_nibble_and_keeps_missing_subband_zero(){
    let mut b=board();for(at,value)in[(0x5bb,0xa8),(0x5ba,0x27),(0x5b9,0xfe),(0x5b8,0x10),(0x590,0xc1),(0x58f,0xa9),(0x58e,0x0c),(0x58d,0xbf)]{b.maps.phycap[at-0x580]=value;}
    let r=calibration::decode(&b).unwrap();assert!(r.gain_comp_valid);assert_eq!(r.gain_comp,[[-8,7,0,-2,0],[1,-7,0,-4,-1]]);
}
#[test]
fn power_cal_marker_is_exact_and_does_not_replace_firmware_capabilities(){
    for raw in [0,0xaa,0xab,255]{let mut b=board();b.maps.phycap[0x5e9-0x580]=raw;assert_eq!(calibration::decode(&b).unwrap().power_k_valid,raw==0xaa);}
}
#[test]
fn foreign_cut_and_rfe_refuse_before_decoding(){
    for(cut,rfe)in[(0,1),(2,1),(1,0),(1,2),(1,255)]{let mut b=board();b.identity.cut=cut;b.identity.rfe=rfe;assert!(calibration::decode(&b).is_err());}
}
