//! Which of the Wi-Fi station's clients a start gets, by the bits it asks for (550-APP-0124, wifi.wit 1.3). No system
//! calls: tests/shell_host.rs.

/// None; the shell's changing client (`REQUEST_WIFI`); or the reading one the station mints (`REQUEST_WIFI_READ`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lend { None, Changing, Reading }

/// `change` and `read`: the bits asked for; `held`: the shell holds the station's client (a radio runs); `granted`: a
/// script declared `wifi`, or no script runs it. Both bits get the changing client.
pub fn lend(change: bool, read: bool, held: bool, granted: bool) -> Lend {
    match (held && granted, change, read) {
        (false, _, _) => Lend::None,
        (true, true, _) => Lend::Changing,
        (true, false, true) => Lend::Reading,
        (true, false, false) => Lend::None,
    }
}
