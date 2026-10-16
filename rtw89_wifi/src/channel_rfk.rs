//! Private first-channel RF calibration coordination; no native calibration-power authority provider exists.

use crate::{board::Board, btc_context::{self, Initialized}, btc_session::{DeviceIo, Session},
    dack::Band, dpk, io::{Fault, FaultKind, RegisterIo}, iqk, phy::{self, Phy, PhyIo, State, channel, dm}, tssi};

const TOTAL_US:u64=15_000_000;
const CALLBACK_US:u32=100_000;
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) enum Stage{Guard,ConnectionBegin,Iqk,PreserveBeforeTssi,Tssi,PreserveBeforeDpk,Dpk,ConnectionEnd,Mcc,Verify}
#[derive(Debug)]
pub(crate) enum Cause{Register(Fault),Owner(btc_context::Error),Iqk(iqk::Failure),Tssi(tssi::Failure),Dpk(dpk::Failure)}
#[derive(Debug)]
pub(crate) struct Failure{pub stage:Stage,pub cause:Cause,pub radio_stop:Option<Fault>,
    pub connection_end:Option<btc_context::Error>,pub coexistence:Option<btc_context::RfkFailure>}
/// Separate non-clone authorities are consumed before any calibration; none has a native constructor.
pub(crate) struct Authorities{pub iqk:channel::CalibrationTransmit,pub tssi:channel::CalibrationTransmit,pub dpk:channel::CalibrationTransmit}
#[derive(Debug)]
pub(crate) struct Completed{attempt:u64,session:u64,channel:u8,policy_version:u64,
    pub iqk:iqk::Completed,pub tssi:tssi::Completed,pub dpk:dpk::Completed}
impl Completed{
    pub(crate) fn matches(&self,tuned:&channel::Tuned)->bool{
        self.attempt==tuned.attempt&&self.session==tuned.session&&self.channel==tuned.channel&&self.policy_version==tuned.policy_version
            &&self.iqk.matches(self.attempt,self.session,self.channel,self.policy_version)&&self.tssi.matches(tuned)&&self.dpk.matches(tuned)
    }
}
fn fault(kind:FaultKind)->Cause{Cause::Register(Fault::new(kind,0,0))}
fn left(bus:&mut impl RegisterIo,start:u64,reserve:u64)->Result<u32,Cause>{
    let elapsed=bus.now_us().checked_sub(start).ok_or_else(||fault(FaultKind::Clock))?;
    TOTAL_US.checked_sub(elapsed).filter(|v|*v>reserve).map(|v|v as u32).ok_or_else(||fault(FaultKind::Timeout))
}
fn guard<B:DeviceIo>(phy:&mut Phy<'_,Session<'_,'_,'_,B>>,board:&Board,owner:&Initialized,start:u64)->Result<(),Cause>{
    let timeout=left(phy.bus_between_stages(),start,0)?.min(10_000);
    owner.validate(phy,timeout).map_err(Cause::Owner)?;
    phy::guard(phy.bus_between_stages(),&board.identity,phy::RegisterPhase::AfterDack).map_err(Cause::Register)?;
    left(phy.bus_between_stages(),start,0).map(|_|())
}

/// Same cold PHY/session, initial 2 GHz/20 MHz slot zero only; ordinary TX stays stopped and DMA remains owned.
/// This does not associate, enable packet transmission, or replace actual RFK power/SAR/regulatory admission.
pub(crate) fn run<B:DeviceIo>(phy:&mut Phy<'_,Session<'_,'_,'_,B>>,board:&Board,prefix:&dm::Prefix,
    tuned:&channel::Tuned,owner:&Initialized,authorities:Authorities)->Result<Completed,Failure>{
    let attempt=phy.attempt();let session=PhyIo::command_session(phy.bus_between_stages());
    let bare=|cause|Failure{stage:Stage::Guard,cause,radio_stop:None,connection_end:None,coexistence:None};
    if phy.state()!=State::ChannelTuned||phy.identity()!=board.identity||attempt!=tuned.attempt||session!=tuned.session
        ||prefix.attempt()!=attempt||prefix.band!=Band::Ghz2||!owner.matches(attempt,session)
        ||!authorities.iqk.matches(tuned,channel::CalibrationEngine::Iqk)
        ||!authorities.tssi.matches(tuned,channel::CalibrationEngine::Tssi)
        ||!authorities.dpk.matches(tuned,channel::CalibrationEngine::Dpk){return Err(bare(fault(FaultKind::PowerState)));}
    let start=phy.bus_between_stages().now_us();let mut stage=Stage::Guard;let mut begun=false;let mut ended=false;
    let result=(||{
        guard(phy,board,owner,start)?;
        stage=Stage::ConnectionBegin;let timeout=left(phy.bus_between_stages(),start,0)?.min(CALLBACK_US);begun=true;
        phy.bus_between_stages().connection_rfk(true,timeout).map_err(Cause::Owner)?;
        stage=Stage::Iqk;left(phy.bus_between_stages(),start,6_000_000)?;
        let iqk=iqk::run(phy,board,tuned,owner,authorities.iqk).map_err(Cause::Iqk)?;guard(phy,board,owner,start)?;
        stage=Stage::PreserveBeforeTssi;let timeout=left(phy.bus_between_stages(),start,0)?.min(CALLBACK_US);
        phy.bus_between_stages().preserve_bt_time(timeout).map_err(Cause::Owner)?;guard(phy,board,owner,start)?;
        stage=Stage::Tssi;left(phy.bus_between_stages(),start,4_500_000)?;
        let tssi=tssi::run(phy,board,prefix,tuned,owner,&iqk,authorities.tssi).map_err(Cause::Tssi)?;guard(phy,board,owner,start)?;
        stage=Stage::PreserveBeforeDpk;let timeout=left(phy.bus_between_stages(),start,0)?.min(CALLBACK_US);
        phy.bus_between_stages().preserve_bt_time(timeout).map_err(Cause::Owner)?;guard(phy,board,owner,start)?;
        stage=Stage::Dpk;left(phy.bus_between_stages(),start,3_000_000)?;
        let dpk=dpk::run(phy,board,tuned,owner,&tssi,authorities.dpk).map_err(Cause::Dpk)?;guard(phy,board,owner,start)?;
        stage=Stage::ConnectionEnd;let timeout=left(phy.bus_between_stages(),start,0)?.min(CALLBACK_US);ended=true;
        phy.bus_between_stages().connection_rfk(false,timeout).map_err(Cause::Owner)?;
        stage=Stage::Mcc;let timeout=left(phy.bus_between_stages(),start,0)?.min(CALLBACK_US);
        phy.bus_between_stages().notify_mcc(tuned.channel,timeout).map_err(Cause::Owner)?;
        stage=Stage::Verify;guard(phy,board,owner,start)?;
        Ok(Completed{attempt,session,channel:tuned.channel,policy_version:tuned.policy_version,iqk,tssi,dpk})
    })();
    match result{
        Ok(done)=>Ok(done),
        Err(cause)=>{
            let radio_stop=phy::disable_bb_rf(phy.bus_between_stages()).err();phy.mark_failed();
            let connection_end=if begun&&!ended{phy.bus_between_stages().connection_rfk(false,CALLBACK_US).err()}else{None};
            let coexistence=btc_context::last_rfk_failure(phy.bus_between_stages());
            Err(Failure{stage,cause,radio_stop,connection_end,coexistence})
        }
    }
}

impl<B:DeviceIo> channel::ChannelIo for Session<'_ ,'_ ,'_ ,B>{
    fn scheduler_zero(&mut self,timeout_us:u32)->crate::io::Result<()>{crate::rxdck::RxdckIo::scheduler_zero(self,timeout_us)}
}
