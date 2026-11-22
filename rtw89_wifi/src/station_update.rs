//! Active CAM/filter update through the sealed packet-mode owner; no standalone transport callback.

use super::{cam,cam_for,field,join_info,rx_filter,Filter,Prepared,Stage,RX_FILTER,RX_FILTER_MASK};
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
            update.registers(|bus|field(bus,RX_FILTER,RX_FILTER_MASK,rx_filter(&filter)))?;
            stage=Stage::Verify;let mut check_failure=None;
            update.registers(|bus|self.check_registers_for(bus,Some(filter)).map_err(|e|{
                selector_restore=e.selector_restore;check_failure=Some(e);e.cause
            })).map_err(|cause|check_failure.map_or(cause,Cause::Station))?;
            update.finish()?;Ok(())
        })();
        match result{Ok(())=>{self.filter=Some(filter);Ok(())},Err(cause)=>Err(FilterFailure{stage,cause,selector_restore})}
    }
    /// The join info and the address CAM of the installed filter's BSS, connected with `aid` or not (213-DRV-0091),
    /// in Linux's order (rtw89_core_sta_link_assoc: join info, then the CAM); the station's registers checked after.
    pub(crate) fn connect<B:DeviceIo>(&mut self,update:&mut FilterChange<'_,'_,'_,'_,'_,B>,aid:Option<u16>)
        ->Result<(),FilterFailure>{
        let bare=|cause|FilterFailure{stage:Stage::Guard,cause,selector_restore:None};
        let Some(filter)=self.filter else{return Err(bare(Cause::State))};
        if self.attempt==0||self.session==0||self.attempt!=update.attempt()||self.session!=update.session()
            ||self.channel!=update.channel()||self.policy_version!=update.policy_version()||self.mac!=update.physical_board_mac()
            ||aid.is_some()&&filter.bssid()==[0;6]{return Err(bare(Cause::Identity));}
        let mut stage=Stage::Join;let mut selector_restore=None;
        let result=(||{
            update.command(8,0,&join_info(aid.is_some()))?;
            stage=Stage::Cam;update.command_cam(&cam_for(filter.own(),filter.bssid(),aid))?;
            stage=Stage::Verify;let mut check_failure=None;
            update.registers(|bus|self.check_registers_for(bus,Some(filter)).map_err(|e|{
                selector_restore=e.selector_restore;check_failure=Some(e);e.cause
            })).map_err(|cause|check_failure.map_or(cause,Cause::Station))?;Ok(())
        })();
        result.map_err(|cause|FilterFailure{stage,cause,selector_restore})
    }
}
