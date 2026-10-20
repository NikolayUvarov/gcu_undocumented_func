//! One initial admitted channel and data owner; the caller supplies verified policy and retains all backing memory.

use crate::{board::Board, btc_session::{DeviceIo, Session, data_mode::{self, DataSession, Shutdown}},
    channel_rfk, download::DefaultTables, initial_phy, phy::{Phy, channel::{self, calibration_policy::{self, Resolved}}},
    phycap::Capabilities, report_tail, station, tx};

#[derive(Debug)]
pub(crate) enum Failure {
    Initial(initial_phy::Failure), Channel(channel::Failure), Admission(calibration_policy::Error),
    Calibration(channel_rfk::Failure), Station(station::Failure), Reports(report_tail::Failure), Activation(data_mode::Failure),
}

pub(crate) struct Completed<T> {
    pub value: T,
    pub initial: initial_phy::Observations,
    pub shutdown: Shutdown,
}

/// Consumes the actual startup owner and nonclone policy; no input defaults or successful stage substitutes exist here.
/// The callback cannot return borrowed stage state; after it ends packet shutdown is mandatory, even for an error value.
/// Caller must still stop RX/WCPU, verify PCI quiescence and retain allocation on every result; this never frees DMA.
pub(crate) fn run<B: DeviceIo, T>(mut session: Session<'_, '_, '_, B>, queue: &mut tx::Queue<'_>,
    board: &Board, caps: &Capabilities, tables: &DefaultTables, resolved: Resolved,
    serve: impl FnOnce(&mut DataSession<'_, '_, '_, '_, '_, '_, B>) -> T) -> Result<Completed<T>, Failure> {
    let mut phy = Phy::new(&mut session, board.identity);
    let mut initial = initial_phy::prepare(&mut phy, board, caps).map_err(|cause| Failure::Initial(initial_phy::Failure {
        cause, coexistence: crate::btc_context::last_rfk_failure(phy.bus_between_stages()),
    }))?;
    let mut tuned = channel::run(&mut phy, board, &initial.prefix, &initial.paths, tables, resolved.policy())
        .map_err(Failure::Channel)?;
    let [iqk, tssi, dpk] = resolved.grant(&mut phy, board, tables, &mut tuned, &initial.owner).map_err(Failure::Admission)?;
    let calibrated = channel_rfk::run(&mut phy, board, &initial.prefix, &tuned, &initial.owner,
        channel_rfk::Authorities { iqk, tssi, dpk }).map_err(Failure::Calibration)?;
    let mut station = station::prepare(&mut phy, board, &tuned, &calibrated, &initial.owner, caps).map_err(Failure::Station)?;
    let filter = station.filter(true, [0; 6], [0; 6]);
    station.configure_filter(&mut phy, board, &tuned, &initial.owner, filter).map_err(Failure::Station)?;
    let activation = data_mode::prepare(&mut phy, board, &tuned, &calibrated, &initial.owner, station, queue)
        .map_err(Failure::Station)?;
    drop(phy);
    let reports = report_tail::configure(session.bus, session.queue, session.rings, session.receiving, None)
        .map_err(Failure::Reports)?;
    initial.observations.encoded_rts = reports.encoded_rts;
    let mut data = match DataSession::activate(session, queue, activation) {
        Ok(data) => data,
        Err(failed) => return Err(Failure::Activation(failed.failure)),
    };
    let value = serve(&mut data);
    let shutdown = data.shutdown();
    drop(data);
    Ok(Completed { value, initial: initial.observations, shutdown })
}
