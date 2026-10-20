#[path="../rtw89_wifi/src/station_filter.rs"]mod filter;
use filter::Filter;
const OWN:[u8;6]=[2,1,2,3,4,5];
const PEER:[u8;6]=[2,8,7,6,5,4];
const OTHER:[u8;6]=[2,6,6,6,6,6];
const GROUP:[u8;6]=[1,0,94,0,0,1];
fn frame(fc:u8,ds:u8,ra:[u8;6],ta:[u8;6],a3:[u8;6])->[u8;32]{
    let mut f=[0;32];f[0]=fc;f[1]=ds;f[4..10].copy_from_slice(&ra);
    f[10..16].copy_from_slice(&ta);f[16..22].copy_from_slice(&a3);f
}
#[test]fn zero_own_uses_radio_and_override_replaces_it(){
    let default=Filter::new(OWN,false,[0;6],[0;6]);let overridden=Filter::new(OWN,false,[0;6],OTHER);
    assert_eq!(default.own(),OWN);assert_eq!(overridden.own(),OTHER);assert_eq!(default.bssid(),[0;6]);
    assert!(default.accepts(&frame(0x08,2,OWN,PEER,OTHER)));
    assert!(!overridden.accepts(&frame(0x08,2,OWN,PEER,OTHER)));
    assert!(overridden.accepts(&frame(0xd0,0,OTHER,PEER,OWN)));
}
#[test]fn scan_accepts_only_beacons_and_probe_responses_from_other_bss(){
    let scan=Filter::new(OWN,true,[0;6],[0;6]);let quiet=Filter::new(OWN,false,[0;6],[0;6]);
    assert!(scan.all_beacons());assert!(!quiet.all_beacons());
    for subtype in 0..16{let f=frame(subtype<<4,0,OTHER,PEER,PEER);
        assert_eq!(scan.accepts(&f),subtype==5||subtype==8);assert!(!quiet.accepts(&f));}
    assert!(!scan.accepts(&frame(0x08,2,GROUP,PEER,OWN)));
}
#[test]fn selected_bssid_accepts_group_management_and_ds_zero_data(){
    let selected=Filter::new(OWN,false,PEER,[0;6]);assert_eq!(selected.bssid(),PEER);
    for fc in [0x00,0x50,0x80,0xb0,0xc0,0xd0,0x08,0x88]{
        assert!(selected.accepts(&frame(fc,0,GROUP,OTHER,PEER)));
        assert!(selected.accepts(&frame(fc,0,[255;6],OTHER,PEER)));
        assert!(!selected.accepts(&frame(fc,0,GROUP,PEER,OTHER)));
        assert!(!selected.accepts(&frame(fc,0,OTHER,PEER,PEER)));
    }
}
#[test]fn from_ds_group_requires_transmitter_bssid(){
    let selected=Filter::new(OWN,false,PEER,[0;6]);
    for fc in [0x08,0x88]{assert!(selected.accepts(&frame(fc,2,GROUP,PEER,OTHER)));
        assert!(!selected.accepts(&frame(fc,2,GROUP,OTHER,PEER)));}
    for ds in [1,3]{assert!(!selected.accepts(&frame(0x08,ds,GROUP,PEER,PEER)));}
}
#[test]fn no_bssid_rejects_group_but_keeps_own_frames(){
    let disconnected=Filter::new(OWN,false,[0;6],[0;6]);
    for ds in 0..4{assert!(!disconnected.accepts(&frame(0x08,ds,GROUP,PEER,PEER)));
        assert!(disconnected.accepts(&frame(0x08,ds,OWN,PEER,PEER)));}
}
#[test]fn control_reserved_version_and_truncated_frames_are_rejected(){
    let scan=Filter::new(OWN,true,PEER,[0;6]);
    for subtype in 0..16{for kind in [1,3]{assert!(!scan.accepts(&frame(subtype<<4|kind<<2,0,OWN,PEER,PEER)));}}
    for version in 1..4{assert!(!scan.accepts(&frame(0x80|version,0,OWN,PEER,PEER)));}
    let f=frame(0x80,0,OWN,PEER,PEER);for end in 0..24{assert!(!scan.accepts(&f[..end]));}
}
#[test]fn immutable_filter_snapshot_preserves_scan_epoch(){
    let old=Filter::new(OWN,true,[0;6],[0;6]);let current=Filter::new(OWN,false,PEER,[0;6]);
    let f=frame(0x80,0,[255;6],OTHER,OTHER);
    assert!(old.accepts(&f));assert!(old.all_beacons());assert!(!current.accepts(&f));assert!(!current.all_beacons());
}
