//! `wifi`'s `Station` over the station service's client (`wlan`, idl/wifi.wit 1.0, 550-NET-0018).
use crate::station::{Link, Network, Reason, Refusal, Security, Station};
use alloc::vec::Vec;
use mind::idl::wifi;
use mind::ipc::Endpoint;

/// The station behind a client lent to the program (`BADGE_WIFI`; with `BADGE_WIFI_CHANGE` it may also change).
pub struct Service(pub Endpoint);

fn refusal(error: wifi::Error) -> Refusal {
    match error {
        wifi::Error::NoDevice => Refusal::NoStation(alloc::string::String::from("the station runs, but it has no radio")),
        wifi::Error::Busy => Refusal::Busy,
        wifi::Error::Invalid => Refusal::Invalid,
        wifi::Error::Denied => Refusal::Denied,
        wifi::Error::Unsupported => Refusal::Unsupported,
        wifi::Error::NotFound => Refusal::NotFound,
    }
}

// The station's answer, or why there was none.
fn answer<T>(reply: mind::sys::Result<Result<T, wifi::Error>>) -> Result<T, Refusal> {
    reply.map_err(|_| Refusal::Lost)?.map_err(refusal)
}

fn security(s: wifi::Security) -> Security {
    match s {
        wifi::Security::Open => Security::Open, wifi::Security::Wep => Security::Wep, wifi::Security::Wpa2 => Security::Wpa2,
        wifi::Security::Wpa3 => Security::Wpa3, wifi::Security::Enterprise => Security::Enterprise, wifi::Security::Other => Security::Other,
    }
}

fn reason(r: wifi::Reason) -> Reason {
    match r {
        wifi::Reason::WrongPassphrase => Reason::WrongPassphrase, wifi::Reason::NoAnswer => Reason::NoAnswer,
        wifi::Reason::Refused => Reason::Refused, wifi::Reason::NotFound => Reason::NotFound,
        wifi::Reason::Unsupported => Reason::Unsupported, wifi::Reason::Lost | wifi::Reason::None => Reason::Lost,
    }
}

impl Station for Service {
    fn scan(&mut self) -> Result<Vec<Network>, Refusal> {
        let heard = answer(wifi::scan(self.0))?;
        Ok(heard.as_slice().iter().map(|n| Network {
            ssid: Vec::from(n.ssid.as_slice()), bssid: n.bssid, channel: n.channel, weakness: n.weakness, security: security(n.security), remembered: n.remembered,
        }).collect())
    }
    fn join(&mut self, ssid: &[u8], passphrase: &str, remember: bool) -> Result<(), Refusal> { answer(wifi::join(self.0, ssid, passphrase, remember)) }
    fn status(&mut self) -> Result<Link, Refusal> {
        let s = wifi::status(self.0).map_err(|_| Refusal::Lost)?;
        let ssid = Vec::from(s.ssid.as_slice());
        Ok(match s.state {
            wifi::State::Off => Link::Off,
            wifi::State::Scanning => Link::Scanning,
            wifi::State::Joining if s.reason == wifi::Reason::Lost => Link::Lost { ssid },
            wifi::State::Joining => Link::Joining { ssid },
            wifi::State::Joined => Link::Joined { ssid, bssid: s.bssid, channel: s.channel, weakness: s.weakness, rate: s.rate },
            wifi::State::Failed => Link::Failed { ssid, reason: reason(s.reason) },
        })
    }
    fn leave(&mut self) -> Result<(), Refusal> { answer(wifi::leave(self.0)) }
    fn forget(&mut self, ssid: &[u8]) -> Result<bool, Refusal> { answer(wifi::forget(self.0, ssid)) }
}
