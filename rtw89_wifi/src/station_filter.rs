//! The wlanhw address policy for an already validated MPDU without FCS; no hardware authority.

#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) struct Filter{all_beacons:bool,bssid:[u8;6],own:[u8;6]}
impl Filter{
    pub(crate) fn new(radio_mac:[u8;6],all_beacons:bool,bssid:[u8;6],own:[u8;6])->Self{
        Self{all_beacons,bssid,own:if own==[0;6]{radio_mac}else{own}}
    }
    pub(crate) fn all_beacons(&self)->bool{self.all_beacons}
    pub(crate) fn bssid(&self)->[u8;6]{self.bssid}
    pub(crate) fn own(&self)->[u8;6]{self.own}
    /// The frame decoder checks complete variable headers, FCS and descriptor errors before this predicate.
    pub(crate) fn accepts(&self,mpdu:&[u8])->bool{
        if mpdu.len()<24||mpdu[0]&3!=0{return false;}
        let kind=mpdu[0]>>2&3;let subtype=mpdu[0]>>4;
        if kind==1||kind==3{return false;}
        if kind==0&&(subtype==8||subtype==5)&&self.all_beacons{return true;}
        if mpdu[4..10]==self.own{return true;}
        if self.bssid==[0;6]||mpdu[4]&1==0{return false;}
        let bssid=match(kind,mpdu[1]&3){(0,_)|(2,0)=>&mpdu[16..22],(2,2)=>&mpdu[10..16],_=>return false};
        bssid==self.bssid
    }
}
