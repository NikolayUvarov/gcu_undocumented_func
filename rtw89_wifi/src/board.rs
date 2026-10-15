//! A single initial board-read cycle, with an independent shutdown budget on every modified path.

use crate::efuse::{self, Maps};
use crate::io::{modify8, read32, Fault, FaultKind, RegisterIo, Result, TimedIo};
use crate::power;
use crate::registers::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage { Identify, PowerOn, Efuse, Board, PowerOff }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Failure {
    pub stage: Stage,
    pub cause: Fault,
    pub efuse_restore: Option<Fault>,
    pub shutdown: Option<Fault>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Identity { pub cut: u8, pub mac: [u8; 6], pub rfe: u8, pub xtal: u8, pub country: [u8; 2], pub channel_plan: u8 }

pub struct Board { pub identity: Identity, pub maps: Maps }

pub fn identity(cut: u8, maps: &Maps) -> Result<Identity> {
    let mut mac = [0; 6];
    mac.copy_from_slice(&maps.logical[efuse::MAC_AT..efuse::MAC_AT + 6]);
    if mac[0] & 1 != 0 || mac == [0; 6] { return Err(Fault::new(FaultKind::Mac, efuse::MAC_AT, 0)); }
    let rfe = maps.logical[efuse::RFE_AT];
    if rfe != 1 { return Err(Fault::new(FaultKind::Rfe, efuse::RFE_AT, rfe as u32)); }
    let xtal = maps.logical[efuse::XTAL_AT];
    if xtal == 0xff { return Err(Fault::new(FaultKind::Xtal, efuse::XTAL_AT, xtal as u32)); }
    Ok(Identity { cut, mac, rfe, xtal, country: [maps.logical[efuse::COUNTRY_AT], maps.logical[efuse::COUNTRY_AT + 1]], channel_plan: maps.logical[efuse::CHANNEL_PLAN_AT] })
}

/// The caller owns a verified MEM-only BAR grant; an already-active MAC is left untouched for diagnosis.
pub fn read(io: &mut impl RegisterIo) -> core::result::Result<Board, Failure> {
    let fail = |cause| Failure { stage: Stage::Identify, cause, efuse_restore: None, shutdown: None };
    let cut = power::identify(io).map_err(fail)?;
    let state = read32(io, R_AX_IC_PWR_STATE).map_err(fail)?;
    if state & B_AX_WLMAC_PWR_STE_MASK != 0 {
        return Err(fail(Fault::new(FaultKind::PowerState, R_AX_IC_PWR_STATE, state)));
    }
    let mut stage = Stage::PowerOn;
    let mut efuse_restore = None;
    let mut rfe = None;
    let result = (|| {
        let mut timed = TimedIo::new(io, 1_000_000);
        modify8(&mut timed, R_AX_SYS_FUNC_EN, (B_AX_FEN_BB_GLB_RSTN | B_AX_FEN_BBRSTB) as u8, 0)?;
        let bb = timed.read8(R_AX_SYS_FUNC_EN)?;
        if bb & (B_AX_FEN_BB_GLB_RSTN | B_AX_FEN_BBRSTB) as u8 != 0 {
            return Err(Fault::new(FaultKind::PowerState, R_AX_SYS_FUNC_EN, bb as u32));
        }
        power::on_for_board_read(&mut timed)?;
        let bb = timed.read8(R_AX_SYS_FUNC_EN)?;
        if bb & (B_AX_FEN_BB_GLB_RSTN | B_AX_FEN_BBRSTB) as u8 != 0 {
            return Err(Fault::new(FaultKind::PowerState, R_AX_SYS_FUNC_EN, bb as u32));
        }
        stage = Stage::Efuse;
        let maps = efuse::read_maps(io).map_err(|error| { efuse_restore = error.restore; error.cause })?;
        rfe = Some(maps.logical[efuse::RFE_AT]);
        stage = Stage::Board;
        let identity = identity(cut, &maps)?;
        Ok(Board { identity, maps })
    })();
    let shutdown = power::off(&mut TimedIo::new(io, 1_000_000), rfe).err();
    match (result, shutdown) {
        (Ok(board), None) => Ok(board),
        (Ok(_), Some(cause)) => Err(Failure { stage: Stage::PowerOff, cause, efuse_restore, shutdown: Some(cause) }),
        (Err(cause), shutdown) => Err(Failure { stage, cause, efuse_restore, shutdown }),
    }
}
