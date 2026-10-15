//! One diagnostic continuation through actual owned BTC and initial PHY calibration, followed by mandatory shutdown.

use crate::{board::Board, btc_context, btc_session::{self, DeviceIo, Session}, dack, download,
    fwcmd::Queue, io::Fault, phy::{self, Phy}, phycap::Capabilities, post_mac, report_tail, rx, rxdck};

#[derive(Debug)]
pub enum Cause {
    Capabilities,
    Session(btc_session::Error),
    Phy(phy::Failure),
    Coexistence(btc_context::Failure),
    Validation(btc_context::Error),
    DynamicManagement { stage: &'static str, cause: Fault, radio_stop: Option<Fault> },
    Dack(dack::Failure),
    Rxdck(rxdck::Failure),
    Paths { stage: &'static str, cause: Fault, radio_stop: Option<Fault> },
    Reports { stage: &'static str, cause: post_mac::Error, report_stop: Option<Fault>,
        receive_stop: Option<rx::Stop>, radio_stop: Option<Fault>, command_stop: Option<download::Stop> },
}

#[derive(Debug)]
pub struct Failure { pub cause: Cause, pub coexistence: Option<btc_context::RfkFailure> }

/// Measurements from an ended diagnostic session; this cannot resume the command engine or authorize transmission.
#[derive(Debug)]
pub struct Observations {
    pub thermal: [u8; 2], pub rck: phy::RckResult, pub dack: dack::ResultData,
    pub rxdck: rxdck::Completed, pub rx_path: u8, pub rx_nss: u8,
    pub submitted: u16, pub received: btc_session::Counts, pub encoded_rts: u16,
}

fn dm_failure(error: phy::dm::Failure) -> Cause {
    use phy::dm::Stage;
    let stage = match error.stage {
        Stage::Guard => "guard", Stage::ThermalA => "thermal-a", Stage::ThermalB => "thermal-b",
        Stage::Baseband => "baseband", Stage::Environment => "environment", Stage::PhyStatus => "phy-status",
        Stage::Dig => "dig", Stage::Crystal => "crystal", Stage::Dcfo => "dcfo", Stage::Edcca => "edcca", Stage::UlTb => "ul-tb",
    };
    Cause::DynamicManagement { stage, cause: error.cause, radio_stop: error.radio_stop }
}

fn tail_failure(error: phy::tail::Failure) -> Cause {
    use phy::tail::Stage;
    let stage = match error.stage {
        Stage::Guard => "guard", Stage::PowerReference => "power-reference", Stage::ThermalTrim => "thermal-trim",
        Stage::PaTrim => "pa-trim", Stage::ReceivePath => "receive-path", Stage::GainOffset => "gain-offset",
        Stage::Btg => "btg", Stage::PowerReset => "power-reset", Stage::RadioMode => "radio-mode",
        Stage::Streams => "streams", Stage::MacMode => "mac-mode", Stage::Verify => "verify",
    };
    Cause::Paths { stage, cause: error.cause, radio_stop: error.radio_stop }
}

fn initial<B: DeviceIo>(session: &mut Session<'_, '_, '_, B>, board: &Board, caps: &Capabilities) -> Result<Observations, Cause> {
    let mut phy = Phy::new(session, board.identity);
    let _gain = phy.baseband().map_err(Cause::Phy)?;
    phy.radio().map_err(Cause::Phy)?;
    let coexistence = btc_context::initialize(&mut phy, board).map_err(Cause::Coexistence)?;
    coexistence.validate(&mut phy, 10_000).map_err(Cause::Validation)?;
    let prefix = phy::dm::run(&mut phy, dack::Band::Ghz2).map_err(dm_failure)?;
    phy.nctl().map_err(Cause::Phy)?;
    let rck = phy.rck().map_err(Cause::Phy)?;
    let dack = phy.dack(dack::Band::Ghz2).map_err(Cause::Dack)?;
    coexistence.validate(&mut phy, 10_000).map_err(Cause::Validation)?;
    let rxdck = phy.rxdck(dack::Band::Ghz2).map_err(Cause::Rxdck)?;
    coexistence.validate(&mut phy, 10_000).map_err(Cause::Validation)?;
    let paths = phy::tail::run(&mut phy, board, &prefix, caps, phy::tail::Subband::Ghz2).map_err(tail_failure)?;
    coexistence.validate(&mut phy, 10_000).map_err(Cause::Validation)?;
    let session = phy.bus_between_stages();
    Ok(Observations { thermal: prefix.thermal, rck, dack, rxdck, rx_path: paths.rx_path, rx_nss: paths.rx_nss,
        submitted: session.submitted(), received: session.counts(), encoded_rts: 0 })
}

/// Consume this successful firmware startup once; retain command/RX backing memory and shut down after either result.
/// The caller owns the same verified board, PCI function, coherent DMA regions and exclusive register access throughout.
/// All stages use their existing finite budgets; no channel tuning, IQK/TSSI/DPK, packet TX or WLAN endpoint is enabled.
pub fn diagnose<B: DeviceIo>(bus: &mut B, queue: &mut Queue<'_>, rings: &mut rx::Rings<'_>,
    receiving: &post_mac::Receiving, firmware: download::Ready, board: &Board, caps: &Capabilities,
    scratch: &mut [u8]) -> Result<Observations, Failure> {
    let fail = |cause| Failure { cause, coexistence: None };
    if !caps.belongs_to(&firmware) { return Err(fail(Cause::Capabilities)); }
    let mut observed = {
        let mut session = Session::new(bus, queue, rings, receiving, firmware, scratch).map_err(|e| fail(Cause::Session(e)))?;
        initial(&mut session, board, caps).map_err(|cause| Failure { cause, coexistence: btc_context::last_rfk_failure(&session) })?
    };
    // The consumed Ready and ended command engine cannot be recreated; only the final register tail and shutdown follow.
    let reports = report_tail::configure(bus, queue, rings, receiving, None).map_err(|error| {
        use report_tail::Stage;
        let stage = match error.stage { Stage::Guard => "guard", Stage::Ppdu => "ppdu", Stage::Forward => "forward", Stage::Rts => "rts", Stage::Verify => "verify" };
        fail(Cause::Reports { stage, cause: error.cause, report_stop: error.report_stop,
            receive_stop: error.receive_stop, radio_stop: error.radio_stop, command_stop: error.command_stop })
    })?;
    observed.encoded_rts = reports.encoded_rts;
    Ok(observed)
}
