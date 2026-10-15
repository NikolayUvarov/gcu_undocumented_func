//! Initial TSSI entry binds actual channel IQK, coexistence and single-use calibration power admission.

use crate::{board::Board,btc_context::Initialized,iqk,phy::{self,Phy,State,channel,dm},rxdck::RxdckIo,
    io::{Fault,FaultKind}};
pub(crate) use phy::tssi::{Completed,Failure};

/// Initial 2 GHz/20 MHz only; the caller retains the same exclusive device, firmware and DMA session.
/// There is no native CalibrationTransmit provider until the manual PMAC levels and platform policy are resolved.
pub(crate) fn run<B:RxdckIo>(phy:&mut Phy<'_,B>,board:&Board,prefix:&dm::Prefix,tuned:&channel::Tuned,
    initialized:&Initialized,iqk:&iqk::Completed,admission:channel::CalibrationTransmit)->Result<Completed,Failure>{
    let attempt=phy.attempt();let session=phy.bus_between_stages().command_session();
    if phy.state()!=State::ChannelTuned||!initialized.matches(attempt,session)
        ||!iqk.matches(attempt,session,tuned.channel,tuned.policy_version)
        ||!admission.matches(tuned,channel::CalibrationEngine::Tssi){
        return Err(Failure::guard(Fault::new(FaultKind::PowerState,0,0)));
    }
    phy::tssi::engine(phy,board,prefix,tuned,&admission)
}
