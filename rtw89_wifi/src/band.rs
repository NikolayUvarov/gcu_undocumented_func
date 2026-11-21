// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! The radio's bands and 20 MHz channels (213-DRV-0087): channel numbers, centres and Linux's sub-bands.

/// The band of a channel, as Linux's `rtw89_band` numbers it (also the BTC phy map's band bits).
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
#[repr(u8)]
pub enum Band { Ghz2=0, Ghz5=1 }

/// Linux's `rtw89_subband` for this chip: 2.4 GHz; 5 GHz channels 36-64, 100-144 and 149-177.
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub enum Subband { Ghz2, Ghz5Low, Ghz5Middle, Ghz5High }

impl Subband {
    pub fn band(self)->Band{if self==Self::Ghz2{Band::Ghz2}else{Band::Ghz5}}
    /// The board's gain-compensation index: Linux's `rtw89_subband` value (2G 0, 5G band 1, 3, 4).
    pub fn comp(self)->usize{match self{Self::Ghz2=>0,Self::Ghz5Low=>1,Self::Ghz5Middle=>3,Self::Ghz5High=>4}}
    /// The board's OFDM gain-offset index (`rtw89_subband_to_gain_offset_band_of_ofdm`).
    pub fn gain(self)->usize{match self{Self::Ghz2=>1,Self::Ghz5Low=>2,Self::Ghz5Middle=>3,Self::Ghz5High=>4}}
    /// The BB gain table's band (`rtw89_subband_to_bb_gain_band`).
    pub fn bb_gain(self)->usize{match self{Self::Ghz2=>0,Self::Ghz5Low=>1,Self::Ghz5Middle=>2,Self::Ghz5High=>3}}
}

/// The 5 GHz channels of Linux's `rtw89_channels_5ghz` (core.c), 20 MHz each.
pub const CHANNELS_5G:[u8;28]=[36,40,44,48,52,56,60,64,100,104,108,112,116,120,124,128,132,136,140,144,
    149,153,157,161,165,169,173,177];
/// Every channel this driver tunes: 2.4 GHz 1 to 13, then 5 GHz.
pub const SUPPORTED:[u8;41]={
    let mut all=[0u8;41];let mut i=0;
    while i<13{all[i]=i as u8+1;i+=1;}
    while i<41{all[i]=CHANNELS_5G[i-13];i+=1;}
    all
};

/// The band of one of this driver's channels; None for any other number (14 is outside its 2.4 GHz scope).
pub fn band(channel:u8)->Option<Band>{
    if (1..=13).contains(&channel){Some(Band::Ghz2)}else if CHANNELS_5G.contains(&channel){Some(Band::Ghz5)}else{None}
}
/// Linux's `rtw89_get_subband_type` for one of this driver's channels.
pub fn subband(channel:u8)->Option<Subband>{
    Some(match (band(channel)?,channel){
        (Band::Ghz2,_)=>Subband::Ghz2,
        (Band::Ghz5,36..=64)=>Subband::Ghz5Low,
        (Band::Ghz5,100..=144)=>Subband::Ghz5Middle,
        _=>Subband::Ghz5High,
    })
}
/// The centre frequency in MHz of one of this driver's channels.
pub fn center_mhz(channel:u8)->Option<u16>{
    match band(channel)?{Band::Ghz2=>Some(2407+5*u16::from(channel)),Band::Ghz5=>Some(5000+5*u16::from(channel))}
}
/// The channel whose centre is `mhz`, if it is one of this driver's.
pub fn channel_at(mhz:u16)->Option<u8>{
    let (base,band)=if mhz<5000{(2407,Band::Ghz2)}else{(5000,Band::Ghz5)};
    let delta=mhz.checked_sub(base)?;
    if delta%5!=0||delta/5>255{return None;}
    let channel=(delta/5)as u8;
    (self::band(channel)==Some(band)).then_some(channel)
}
