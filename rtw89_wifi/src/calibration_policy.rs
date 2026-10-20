//! Private source-matched calibration permission; physical RF output is not a measured guarantee.

use super::{Access,CalibrationEngine,CalibrationTransmit,Platform,Policy,PolicyError,Rule,Tuned};
use crate::{board::{self,Board},btc_context::{self,Initialized},btc_session::{DeviceIo,Session},
    calibration,channel_policy as defaults,channel_power::Constraints,download::DefaultTables,
    io::RegisterIo,phy::{Phy,State}};

const VALIDATE_US:u32=10_000;
const TSSI_MAX_QUARTER_DBM:i16=48;

/// This profile selects only the compiled pinned initial cut-1/RFE-1/internal-PA, 2 GHz/20 MHz sequence.
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) enum SourceCalibrationProfile{Rtl8852bCbvRfe1Initial2g20V1}

/// A scoped input consumer must establish these facts; a missing or failed collection is not CheckedAbsent.
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) enum PlatformInput{Unknown,Invalid,CheckedAbsent,Limits(Constraints)}
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) enum PlatformBasis{NoAdditionalSar,ExplicitLimits}
#[derive(Debug)]
pub(crate) enum Error{Board,UnknownPlatform,InvalidPlatform,Policy(PolicyError),Passive,TssiPower,
    State,Identity,PolicyChanged,AlreadyAttempted,Firmware,Clock,Timeout,Owner(btc_context::Error)}

/// Not Clone: the actual scoped consumer resolves one policy for one initial calibration attempt.
#[derive(Debug)]
pub(crate) struct Resolved{policy:Policy,board:[u8;32],basis:PlatformBasis}

pub(super) fn board_fingerprint(board:&Board)->[u8;32]{
    let mut hash=mind::sha256::Sha256::new();let id=&board.identity;
    hash.update(&[id.cut,id.rfe,id.xtal,id.channel_plan]);hash.update(&id.mac);hash.update(&id.country);
    hash.update(&board.maps.logical);hash.update(&board.maps.dav_logical);hash.update(&board.maps.phycap);hash.finish()
}

fn supported_board(board:&Board)->Result<(),Error>{
    if board.identity.cut!=1||board.identity.rfe!=1
        ||board::identity(board.identity.cut,&board.maps).map_err(|_|Error::Board)?!=board.identity
        ||board.maps.phycap[0x5ec-0x580]!=255||board.maps.phycap[0x5ed-0x580]!=255{return Err(Error::Board)}
    calibration::decode(board).map_err(|_|Error::Board)?;Ok(())
}

fn source_tssi_limit(group:defaults::Group,channel:u8)->Result<i16,Error>{
    // The actual source PMAC template is HT20 MCS7, one TX path and no beamforming.
    defaults::default_limit_2g(0,1,2,false,group,channel).map(|v|i16::from(v.quarter_dbm)).map_err(|_|Error::TssiPower)
}

fn tssi_cap(table:i16,limits:Constraints)->Result<(),Error>{
    let cap=table.min(i16::from(limits.per_ntx[0])*2)
        .min(i16::from(limits.per_path[0])*2).min(i16::from(limits.per_path[1])*2);
    if cap<TSSI_MAX_QUARTER_DBM{Err(Error::TssiPower)}else{Ok(())}
}

impl Resolved{
    /// Rule includes the real country and conducted/EIRP relation; the native scoped-input consumer is separate.
    pub(crate) fn resolve(board:&Board,rule:Rule,platform:PlatformInput)->Result<Self,Error>{
        supported_board(board)?;
        let(basis,limits)=match platform{
            PlatformInput::Unknown=>return Err(Error::UnknownPlatform),PlatformInput::Invalid=>return Err(Error::InvalidPlatform),
            PlatformInput::CheckedAbsent=>(PlatformBasis::NoAdditionalSar,Constraints{per_ntx:[63;2],per_path:[63;2]}),
            PlatformInput::Limits(limits)=>(PlatformBasis::ExplicitLimits,limits),
        };
        let policy=Policy::resolve(board,rule,Platform::Resolved(limits)).map_err(Error::Policy)?;
        if policy.access!=Access::InitiateRadiation{return Err(Error::Passive)}
        tssi_cap(source_tssi_limit(policy.group,policy.channel)?,policy.limits)?;
        Ok(Self{policy,board:board_fingerprint(board),basis})
    }
    pub(crate) fn policy(&self)->&Policy{&self.policy}
    pub(crate) fn profile(&self)->SourceCalibrationProfile{SourceCalibrationProfile::Rtl8852bCbvRfe1Initial2g20V1}
    pub(crate) fn platform_basis(&self)->PlatformBasis{self.basis}

    /// Consume the resolved input and allow one live admission attempt per actual tuned result; no RF writes occur here.
    pub(crate) fn grant<B:DeviceIo>(self,phy:&mut Phy<'_,Session<'_,'_,'_,B>>,board:&Board,
        defaults:&DefaultTables,tuned:&mut Tuned,initialized:&Initialized)->Result<[CalibrationTransmit;3],Error>{
        if tuned.admission_attempted{return Err(Error::AlreadyAttempted)}
        if phy.state()!=State::ChannelTuned{return Err(Error::State)}
        if !tuned.policy_matches(&self.policy){return Err(Error::PolicyChanged)}
        let attempt=phy.attempt();let session=phy.bus_between_stages().command_session();
        if attempt==0||session==0||tuned.attempt!=attempt||tuned.session!=session||!initialized.matches(attempt,session)
            ||phy.identity!=board.identity||self.board!=tuned.board_fingerprint||self.board!=board_fingerprint(board){return Err(Error::Identity)}
        if defaults.session()!=session||phy.bus_between_stages().firmware_version()!=[0,29,29,15]
            ||phy.bus_between_stages().failed().is_some(){return Err(Error::Firmware)}
        tuned.admission_attempted=true;
        let start=phy.bus_between_stages().now_us();
        initialized.validate(phy,VALIDATE_US).map_err(Error::Owner)?;
        let elapsed=phy.bus_between_stages().now_us().checked_sub(start).ok_or(Error::Clock)?;
        if elapsed>=u64::from(VALIDATE_US){return Err(Error::Timeout)}
        Ok([CalibrationEngine::Iqk,CalibrationEngine::Tssi,CalibrationEngine::Dpk].map(|engine|CalibrationTransmit{
            attempt,session,channel:tuned.channel,policy_version:tuned.policy_version,engine,
            policy:tuned.policy,board_fingerprint:self.board}))
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn actual_pmac_mcs7_limit_is_selected_when_ofdm_limit_differs(){
        let ofdm=defaults::default_limit_2g(0,1,1,false,defaults::Group::FCC,9).unwrap().quarter_dbm;
        assert_eq!(ofdm,78);assert_eq!(source_tssi_limit(defaults::Group::FCC,9).unwrap(),76);
    }
    #[test]
    fn quarter_dbm_table_boundary_is_not_rounded_up(){
        let limits=Constraints{per_ntx:[63;2],per_path:[63;2]};
        assert!(matches!(tssi_cap(47,limits),Err(Error::TssiPower)));tssi_cap(48,limits).unwrap();
        assert!(matches!(tssi_cap(-1,limits),Err(Error::TssiPower)));
    }
}
