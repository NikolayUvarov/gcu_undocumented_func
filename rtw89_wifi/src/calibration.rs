// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2024 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! Decode the pinned RTL8852B board calibration maps; these are not firmware PHY capabilities or regulatory limits.

use crate::{board::Board,io::{self,Fault,FaultKind}};

#[derive(Debug,Eq,PartialEq)]
pub struct Calibration{
    pub power_k_valid:bool,
    pub thermal_reference:[u8;2],
    pub tssi_cck:[[i8;6];2],
    pub tssi_mcs:[[i8;19];2],
    pub tssi_trim:[[i8;8];2],
    pub thermal_raw:[u8;2],
    pub thermal_setting:Option<[u8;2]>,
    pub pa_bias_raw:[u8;2],
    pub pa_bias_setting:Option<[[u8;2];2]>,
    pub gain_offset_valid:bool,
    pub gain_offset:[[i8;5];2],
    pub gain_comp_valid:bool,
    pub gain_comp:[[i8;5];2],
}

fn signed_nibble(value:u8)->i8{((value<<4)as i8)>>4}
fn thermal(raw:u8)->u8{((raw&1)<<3)|((raw&31)>>1)}

/// Preserve the source's global programmed flags; a lone 0xff field is not replaced with a guessed zero.
/// The caller retains the Board obtained from the same device; this pure decoder grants no hardware readiness.
pub fn decode(board:&Board)->io::Result<Calibration>{
    if board.identity.cut!=1||board.identity.rfe!=1{
        return Err(Fault::new(FaultKind::Rfe,crate::efuse::RFE_AT,board.identity.rfe as u32));
    }
    let logical=&board.maps.logical;let physical=&board.maps.phycap;
    let mut cck=[[0i8;6];2];let mut mcs=[[0i8;19];2];let mut trim=[[0i8;8];2];
    for (path,start) in [0x210,0x23a].into_iter().enumerate(){
        for j in 0..6{cck[path][j]=logical[start+j]as i8;}
        for j in 0..5{mcs[path][j]=logical[start+6+j]as i8;}
        for j in 0..14{mcs[path][j+5]=logical[start+18+j]as i8;}
    }
    let mut trim_programmed=false;
    for (path,start) in [0x5d6,0x5ab].into_iter().enumerate(){
        for j in 0..8{let raw=physical[start-0x580-j];trim[path][j]=raw as i8;trim_programmed|=raw!=255;}
    }
    if !trim_programmed{trim=[[0;8];2];}
    let thermal_raw=[physical[0x5df-0x580],physical[0x5dc-0x580]];
    let thermal_setting=thermal_raw.iter().any(|raw|*raw!=255).then(||thermal_raw.map(thermal));
    let pa_bias_raw=[physical[0x5de-0x580],physical[0x5db-0x580]];
    let pa_bias_setting=pa_bias_raw.iter().any(|raw|*raw!=255).then(||pa_bias_raw.map(|raw|[raw&15,raw>>4]));
    let mut gain_offset=[[0;5];2];let mut gain_offset_valid=false;
    for (index,at) in [0x2d6,0x2d4,0x2d8,0x2da,0x2dc].into_iter().enumerate(){
        let raw=logical[at];gain_offset_valid|=raw!=255;
        gain_offset[0][index]=signed_nibble(raw>>4);gain_offset[1][index]=signed_nibble(raw);
    }
    let mut gain_comp=[[0;5];2];let mut gain_comp_valid=false;
    for (path,addresses) in [[0x5bb,0x5ba,0,0x5b9,0x5b8],[0x590,0x58f,0,0x58e,0x58d]].into_iter().enumerate(){
        for (index,at) in addresses.into_iter().enumerate(){
            if at==0{continue;}
            let raw=physical[at-0x580];gain_comp_valid|=raw!=255;gain_comp[path][index]=signed_nibble(raw);
        }
    }
    Ok(Calibration{power_k_valid:physical[0x5e9-0x580]==0xaa,thermal_reference:[logical[0x2d0],logical[0x2d1]],
        tssi_cck:cck,tssi_mcs:mcs,tssi_trim:trim,thermal_raw,thermal_setting,pa_bias_raw,pa_bias_setting,
        gain_offset_valid,gain_offset,gain_comp_valid,gain_comp})
}
