//! Active CAM/filter update through the sealed packet-mode owner; no standalone transport callback.

use super::{cam,field,Filter,Prepared,Stage,RX_FILTER,RX_FILTER_HOST,RX_FILTER_MASK};
use crate::{btc_session::{DeviceIo,data_mode::{Cause,FilterChange}},io::Fault};

#[derive(Debug)]
pub(crate) struct FilterFailure{pub stage:Stage,pub cause:Cause,pub selector_restore:Option<Fault>}

impl Prepared{
    /// The actual data-mode owner supplies exclusive queues, admission, deadline, epoch drain and terminal cleanup.
    pub(crate) fn reconfigure<B:DeviceIo>(&mut self,update:&mut FilterChange<'_,'_,'_,'_,'_,B>,filter:Filter)
        ->Result<(),FilterFailure>{
        let bare=|cause|FilterFailure{stage:Stage::Guard,cause,selector_restore:None};
        if self.filter.is_none()||self.attempt==0||self.session==0||self.attempt!=update.attempt()
            ||self.session!=update.session()||self.channel!=update.channel()||self.policy_version!=update.policy_version()
            ||self.mac!=update.physical_board_mac(){return Err(bare(Cause::Identity));}
        let mut stage=Stage::Cam;let mut selector_restore=None;
        let result=(||{
            update.command_cam(&cam(filter.own(),filter.bssid()))?;
            stage=Stage::Filter;
            update.registers(|bus|field(bus,RX_FILTER,RX_FILTER_MASK,RX_FILTER_HOST))?;
            stage=Stage::Verify;let mut check_failure=None;
            update.registers(|bus|self.check_registers(bus).map_err(|e|{
                selector_restore=e.selector_restore;check_failure=Some(e);e.cause
            })).map_err(|cause|check_failure.map_or(cause,Cause::Station))?;
            update.finish()?;Ok(())
        })();
        match result{Ok(())=>{self.filter=Some(filter);Ok(())},Err(cause)=>Err(FilterFailure{stage,cause,selector_restore})}
    }
}
