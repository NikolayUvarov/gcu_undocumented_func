//! Host tests of the 802.11 station (550-NET-0017) against a simulated access point on one channel: a scan, joining
//! an open and a WPA2-PSK network (the access point's handshake from `wpa::authenticator`), data both ways under CCMP,
//! what makes the station refuse, fail or join again, and a radio without OFDM on 2.4 GHz (550-NET-0028).
use dot11::frame::{self, Mac, BROADCAST, ETHERTYPE_EAPOL, FROM_DS, PROTECTED, SNAP, TO_DS, TYPE_DATA};
use dot11::scan::{self, Security};
use dot11::station::{Out, Reason, Secret, State, Station, ETHERNET_MAX, FRAME_MAX, HANDSHAKE_MS, REJOIN_MS, SILENCE_MS};
use dot11::Error;
use wpa::authenticator::{Authenticator, FRAME_MAX as EAPOL_MAX};
use wpa::ccmp;
use wpa::eapol::Gtk;
use wpa::handshake::Pairwise;
use wpa::keys;

const OWN: Mac = [0x02, 0x00, 0x00, 0x00, 0x00, 0x01];
const BSSID: Mac = [0x02, 0xAA, 0xAA, 0xAA, 0xAA, 0x01];
const CHANNELS: [u8; 13] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];
const SSID: &[u8] = b"MIND-TEST";
const PASSPHRASE: &[u8] = b"correct horse battery";

/// A simulated access point: beacons, Open System authentication, association, the authenticator's handshake, data.
struct Ap {
    channel: u8,
    rsne: Option<Vec<u8>>,
    pmk: [u8; 32],
    authenticator: Option<Authenticator>,
    gtk: Gtk,
    tk: Option<[u8; 16]>,
    pn: u64,
    group_pn: u64,
    sequence: u16,
    silent: bool,
    refuse: bool,
    /// What the station sent through the access point (decrypted Ethernet type and payload).
    received: Vec<(u16, Vec<u8>)>,
    confirmed: bool,
    /// The EAPOL-Key frame the access point waits for, and an RSN element message 3 carries instead of the beacon's.
    expect: Expect,
    handshake_rsne: Option<Vec<u8>>,
    /// The rates an association request offered; the status and rate elements of the association response.
    offered: Vec<u8>,
    status: u8,
    rates: Vec<u8>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Expect { Nothing, Message2, Message4, Group2 }

impl Ap {
    fn new(rsne: Option<Vec<u8>>) -> Self {
        Ap { channel: 6, rsne, pmk: keys::pmk(PASSPHRASE, SSID).unwrap(), authenticator: None, gtk: Gtk { id: 1, key: [0x77; 16] },
             // The GTK's RSC in message 3 is 5: the last group PN the access point used.
             tk: None, pn: 0, group_pn: 5, sequence: 0, silent: false, refuse: false, received: Vec::new(), confirmed: false,
             expect: Expect::Nothing, handshake_rsne: None, offered: Vec::new(), status: 0,
             rates: vec![1, 4, 0x82, 0x84, 0x0C, 0x18, 50, 2, 0x30, 0x6C] }
    }

    fn header(&mut self, subtype: u8, to: &Mac) -> Vec<u8> {
        self.sequence += 1;
        let mut f = vec![subtype << 4, 0, 0, 0];
        f.extend_from_slice(to);
        f.extend_from_slice(&BSSID);
        f.extend_from_slice(&BSSID);
        f.extend_from_slice(&(self.sequence << 4).to_le_bytes());
        f
    }

    fn announcement(&mut self, subtype: u8, to: &Mac) -> Vec<u8> {
        let mut f = self.header(subtype, to);
        f.extend_from_slice(&[0; 8]);
        f.extend_from_slice(&100u16.to_le_bytes());
        let privacy = if self.rsne.is_some() { frame::CAPABILITY_PRIVACY } else { 0 };
        f.extend_from_slice(&(frame::CAPABILITY_ESS | privacy | frame::CAPABILITY_SHORT_SLOT).to_le_bytes());
        f.extend_from_slice(&[0, SSID.len() as u8]);
        f.extend_from_slice(SSID);
        f.extend_from_slice(&[1, 8, 0x82, 0x84, 0x8B, 0x96, 0x0C, 0x12, 0x18, 0x24, 3, 1, self.channel]);
        if let Some(rsne) = &self.rsne { f.extend_from_slice(rsne); }
        f.extend_from_slice(&[50, 4, 0x30, 0x48, 0x60, 0x6C]);
        f
    }

    fn beacon(&mut self) -> Vec<u8> { self.announcement(frame::BEACON, &BROADCAST) }

    fn data(&mut self, to: &Mac, source: &Mac, ethertype: u16, payload: &[u8], protect: bool) -> Vec<u8> {
        self.sequence += 1;
        let mut f = vec![TYPE_DATA << 2, FROM_DS, 0, 0];
        f.extend_from_slice(to);
        f.extend_from_slice(&BSSID);
        f.extend_from_slice(source);
        f.extend_from_slice(&(self.sequence << 4).to_le_bytes());
        f.extend_from_slice(&SNAP);
        f.extend_from_slice(&ethertype.to_be_bytes());
        f.extend_from_slice(payload);
        if !protect { return f; }
        let mut out = vec![0u8; f.len() + 16];
        let len = if frame::group(to) {
            self.group_pn += 1;
            ccmp::encrypt(&self.gtk.key, self.gtk.id, self.group_pn, &f, &mut out).unwrap()
        } else {
            self.pn += 1;
            ccmp::encrypt(&self.tk.unwrap(), 0, self.pn, &f, &mut out).unwrap()
        };
        out.truncate(len);
        out
    }

    fn eapol(&mut self, frame: &[u8]) -> Vec<u8> {
        let protect = self.tk.is_some();
        self.data(&OWN, &BSSID, ETHERTYPE_EAPOL, frame, protect)
    }

    /// The frames the access point answers a station's frame with.
    fn answer(&mut self, f: &[u8]) -> Vec<Vec<u8>> {
        if self.silent { return Vec::new(); }
        let h = frame::header(f).unwrap();
        if h.kind == frame::TYPE_MANAGEMENT {
            let body = &f[h.len..];
            return match h.subtype {
                frame::PROBE_REQUEST => vec![self.announcement(frame::PROBE_RESPONSE, &h.a2)],
                frame::AUTHENTICATION if h.a1 == BSSID => {
                    let mut r = self.header(frame::AUTHENTICATION, &h.a2);
                    r.extend_from_slice(&[0, 0, 2, 0, self.refuse as u8, 0]);
                    vec![r]
                }
                frame::ASSOCIATION_REQUEST if h.a1 == BSSID => {
                    let elements = &body[4..];
                    assert_eq!(frame::Elements::find(elements, frame::SSID), Some(SSID));
                    assert_eq!(frame::Elements::find_whole(elements, frame::RSN).map(|r| r.to_vec()),
                               self.rsne.as_ref().map(|_| scan::own_rsne().to_vec()));
                    self.offered = frame::Elements::new(elements).filter(|(id, _)| *id == frame::RATES || *id == frame::EXTENDED_RATES)
                        .flat_map(|(_, rates)| rates.iter().copied()).collect();
                    let mut r = self.header(frame::ASSOCIATION_RESPONSE, &h.a2);
                    r.extend_from_slice(&[0x01, 0x04, self.status, 0, 0x01, 0xC0]);
                    r.extend_from_slice(&self.rates);
                    if self.status != 0 { return vec![r]; }
                    let mut frames = vec![r];
                    if let Some(rsne) = self.handshake_rsne.clone().or(self.rsne.clone()) {
                        let mut a = Authenticator::new(self.pmk, BSSID, OWN, &rsne, [0xA5; 32]).unwrap();
                        self.expect = Expect::Message2;
                        let mut m = [0u8; EAPOL_MAX];
                        let len = a.message_1(&mut m).unwrap();
                        self.authenticator = Some(a);
                        frames.push(self.eapol(&m[..len]));
                    }
                    frames
                }
                frame::DEAUTHENTICATION => { self.tk = None; self.confirmed = false; self.expect = Expect::Nothing; Vec::new() }
                _ => Vec::new(),
            };
        }
        // A data frame to the distribution system.
        assert_eq!(h.flags & (TO_DS | FROM_DS), TO_DS);
        assert_eq!(h.a1, BSSID);
        let mut plain = vec![0u8; f.len()];
        let body = if h.flags & PROTECTED != 0 {
            let (len, _) = ccmp::decrypt(&self.tk.expect("a protected frame before the keys"), f, &mut plain).unwrap();
            plain[24..len].to_vec()
        } else {
            f[24..].to_vec()
        };
        assert_eq!(body[..6], SNAP);
        let ethertype = u16::from_be_bytes([body[6], body[7]]);
        let payload = body[8..].to_vec();
        if ethertype != ETHERTYPE_EAPOL {
            self.received.push((ethertype, payload));
            return Vec::new();
        }
        let a = self.authenticator.as_mut().unwrap();
        let mut m = [0u8; EAPOL_MAX];
        match self.expect {
            Expect::Message2 => {
                // A wrong passphrase: the MIC fails and nothing is answered.
                if a.message_2(&payload).is_err() { return Vec::new(); }
                let len = a.message_3(&self.gtk, [5, 0, 0, 0, 0, 0, 0, 0], &mut m).unwrap();
                self.expect = Expect::Message4;
                let reply = m[..len].to_vec();
                vec![self.eapol(&reply)]
            }
            Expect::Message4 | Expect::Group2 => {
                a.confirmation(&payload).expect("message 4 or group message 2");
                if self.expect == Expect::Message4 { self.confirmed = true; self.tk = Some(a.ptk().unwrap().tk); }
                self.expect = Expect::Nothing;
                Vec::new()
            }
            Expect::Nothing => panic!("an EAPOL-Key frame nobody asked for"),
        }
    }
}

/// A key service that keeps a network's PMK and answers each handshake's keys (550-NET-0024), or none.
struct Service { pmk: [u8; 32], asked: usize, refuse: bool }

impl Pairwise for Service {
    fn ptk(&mut self, ap: &[u8; 6], own: &[u8; 6], anonce: &[u8; 32], snonce: &[u8; 32]) -> Option<keys::Ptk> {
        self.asked += 1;
        (!self.refuse).then(|| keys::ptk(&self.pmk, ap, own, anonce, snonce))
    }
}

/// The station, the access point and the air between them, with the station's clock and its key service.
struct Air { station: Station, ap: Ap, now: u64, channel: u8, tx: Vec<u8>, rx: Vec<u8>, delivered: Vec<Vec<u8>>, sent: Vec<(Vec<u8>, u8)>, service: Service }

impl Air {
    fn new(ap: Ap) -> Self { Air::with(ap, true) }
    /// The air with a station whose radio has OFDM on 2.4 GHz or not.
    fn with(ap: Ap, ofdm: bool) -> Self {
        Air { station: Station::new(OWN, &CHANNELS, ofdm), ap, now: 1000, channel: 0, tx: vec![0; FRAME_MAX], rx: vec![0; ETHERNET_MAX],
              delivered: Vec::new(), sent: Vec::new(), service: Service { pmk: keys::pmk(PASSPHRASE, SSID).unwrap(), asked: 0, refuse: false } }
    }
    fn apply(&mut self, out: Out) {
        if let Some(channel) = out.channel { self.channel = channel; }
        if let Some(len) = out.deliver { self.delivered.push(self.rx[..len].to_vec()); }
        if let Some((len, rate)) = out.send {
            let f = self.tx[..len].to_vec();
            self.sent.push((f.clone(), rate));
            if self.channel == self.ap.channel { for reply in self.ap.answer(&f) { let _ = self.hear(&reply); } }
        }
    }
    fn hear(&mut self, f: &[u8]) -> Result<(), Error> {
        let mut out = Out::default();
        let result = self.station.receive(f, self.channel, 48, self.now, &[0x42; 32], &mut self.service, &mut out, &mut self.tx, &mut self.rx);
        self.apply(out);
        result
    }
    fn run(&mut self, call: impl FnOnce(&mut Station, u64, &mut Out, &mut [u8]) -> Result<(), Error>) -> Result<(), Error> {
        let mut out = Out::default();
        let result = call(&mut self.station, self.now, &mut out, &mut self.tx);
        self.apply(out);
        result
    }
    fn wait(&mut self, ms: u64) {
        for _ in 0..ms / 10 {
            self.now += 10;
            let now = self.now;
            self.run(|s, _, out, tx| s.tick(now, out, tx)).unwrap();
        }
    }
    fn scan(&mut self) {
        self.run(|s, now, out, tx| s.scan(now, out, tx)).unwrap();
        self.wait(2500);
        assert_eq!(self.station.state(), State::Off);
    }
}

fn wpa2() -> Ap { Ap::new(Some(scan::own_rsne().to_vec())) }

fn joined() -> Air {
    let mut air = Air::new(wpa2());
    air.scan();
    let pmk = keys::pmk(PASSPHRASE, SSID).map(Secret::Pmk);
    air.run(|s, now, out, tx| s.join(SSID, pmk, now, out, tx)).unwrap();
    assert_eq!(air.station.state(), State::Joined, "{:?}", air.station.reason());
    air
}

#[test]
fn scan_finds_the_network_and_probes_only_where_allowed() {
    let mut air = Air::new(wpa2());
    air.scan();
    let found: Vec<_> = air.station.networks().iter().collect();
    assert_eq!(found.len(), 1);
    let bss = found[0];
    assert_eq!((bss.ssid(), bss.bssid, bss.channel, bss.weakness, bss.security), (SSID, BSSID, 6, 48, Security::Wpa2));
    // Probe requests on channels 1 to 11 only, at 1 Mb/s; 12 and 13 are listened to.
    assert_eq!(air.sent.len(), 11);
    assert!(air.sent.iter().all(|(f, rate)| f[0] == frame::PROBE_REQUEST << 4 && *rate == 2));
    // A second scan while one runs is refused.
    air.run(|s, now, out, tx| s.scan(now, out, tx)).unwrap();
    assert_eq!(air.run(|s, now, out, tx| s.scan(now, out, tx)), Err(Error::Busy));
}

#[test]
fn joins_a_wpa2_network_and_carries_data_both_ways() {
    let mut air = joined();
    assert!(air.ap.confirmed);
    // From the station: an Ethernet frame goes out under CCMP and arrives whole.
    let mut ethernet = vec![0x02, 0xAA, 0xAA, 0xAA, 0xAA, 0x01];
    ethernet.extend_from_slice(&OWN);
    ethernet.extend_from_slice(&[0x08, 0x06]);
    ethernet.extend_from_slice(b"an ARP request, say");
    air.run(|s, _, out, tx| s.send(&ethernet, out, tx)).unwrap();
    let (last, rate) = air.sent.last().unwrap();
    assert!(last[1] & PROTECTED != 0 && *rate == 48); // 24 Mb/s to start
    // The station offered 802.11b's and 802.11g's rates, DSSS and CCK basic.
    assert_eq!(air.ap.offered, [0x82, 0x84, 0x8B, 0x96, 0x0C, 0x12, 0x18, 0x24, 0x30, 0x48, 0x60, 0x6C]);
    assert_eq!(air.ap.received, vec![(0x0806, b"an ARP request, say".to_vec())]);
    // A frame claiming another source is not sent (no bridging).
    let mut other = ethernet.clone();
    other[11] = 9;
    assert_eq!(air.run(|s, _, out, tx| s.send(&other, out, tx)), Err(Error::Invalid));

    // To the station: unicast under the TK, then broadcast under the GTK, each delivered as Ethernet.
    let source = [0x02, 0x33, 0, 0, 0, 7];
    let unicast = air.ap.data(&OWN, &source, 0x0800, b"an IPv4 packet", true);
    air.hear(&unicast).unwrap();
    let mut expected = OWN.to_vec();
    expected.extend_from_slice(&source);
    expected.extend_from_slice(&[0x08, 0x00]);
    expected.extend_from_slice(b"an IPv4 packet");
    assert_eq!(air.delivered, vec![expected]);
    let broadcast = air.ap.data(&BROADCAST, &source, 0x0806, b"who has", true);
    air.hear(&broadcast).unwrap();
    assert_eq!(air.delivered.len(), 2);
    assert_eq!(air.delivered[1][..6], BROADCAST);

    // A replayed frame, a changed one, one in the clear, and the station's own broadcast coming back are dropped.
    assert_eq!(air.hear(&unicast), Err(Error::Replay));
    let mut changed = air.ap.data(&OWN, &source, 0x0800, b"another", true);
    let at = changed.len() - 12;
    changed[at] ^= 1;
    assert_eq!(air.hear(&changed), Err(Error::Integrity));
    let clear = air.ap.data(&OWN, &source, 0x0800, b"in the clear", false);
    air.hear(&clear).unwrap();
    let echo = air.ap.data(&BROADCAST, &OWN, 0x0806, b"mine", true);
    air.hear(&echo).unwrap();
    assert_eq!(air.delivered.len(), 2);
}

#[test]
fn group_rekey_and_rate_control() {
    let mut air = joined();
    // A new GTK through the group key handshake; frames under it are delivered.
    let new = Gtk { id: 2, key: [0x88; 16] };
    let mut m = [0u8; EAPOL_MAX];
    let len = air.ap.authenticator.as_mut().unwrap().group(&new, [0; 8], &mut m).unwrap();
    air.ap.expect = Expect::Group2;
    let reply = m[..len].to_vec();
    let message = air.ap.eapol(&reply);
    air.hear(&message).unwrap();
    air.ap.gtk = new;
    air.ap.group_pn = 0;
    let broadcast = air.ap.data(&BROADCAST, &BSSID, 0x0806, b"after the rekey", true);
    air.hear(&broadcast).unwrap();
    assert_eq!(air.delivered.len(), 1);

    // Two unacknowledged frames step the rate down, ten acknowledged ones up again.
    assert_eq!(air.station.data_rate(), 48);
    air.station.sent(false);
    air.station.sent(false);
    assert_eq!(air.station.data_rate(), 24); // the next rate both support: 12 Mb/s
    for _ in 0..10 { air.station.sent(true); }
    assert_eq!(air.station.data_rate(), 48);
}

#[test]
fn a_radio_without_ofdm_joins_as_an_802_11b_station() {
    let mut ap = wpa2();
    ap.rates = vec![1, 8, 0x82, 0x84, 0x8B, 0x96, 0x0C, 0x12, 0x18, 0x24, 50, 4, 0x30, 0x48, 0x60, 0x6C];
    let mut air = Air::with(ap, false);
    air.scan();
    // Probe requests offer 1, 2, 5.5 and 11 Mb/s alone.
    for (f, _) in &air.sent {
        assert_eq!(frame::Elements::find(&f[24..], frame::RATES), Some(&[0x82, 0x84, 0x8B, 0x96][..]));
        assert_eq!(frame::Elements::find(&f[24..], frame::EXTENDED_RATES), None);
    }
    let pmk = keys::pmk(PASSPHRASE, SSID).map(Secret::Pmk);
    air.run(|s, now, out, tx| s.join(SSID, pmk, now, out, tx)).unwrap();
    assert_eq!(air.station.state(), State::Joined, "{:?}", air.station.reason());
    assert_eq!(air.ap.offered, [0x82, 0x84, 0x8B, 0x96]);
    // The association request claims no short slot time, which is 802.11g's.
    let request = air.sent.iter().find(|(f, _)| f[0] == frame::ASSOCIATION_REQUEST << 4).unwrap();
    assert_eq!(u16::from_le_bytes([request.0[24], request.0[25]]) & frame::CAPABILITY_SHORT_SLOT, 0);
    // Data goes at 11 Mb/s, steps down to 5.5 and back, and never reaches an OFDM rate.
    assert_eq!(air.station.data_rate(), 22);
    air.station.sent(false);
    air.station.sent(false);
    assert_eq!(air.station.data_rate(), 11);
    for _ in 0..30 { air.station.sent(true); }
    assert_eq!(air.station.data_rate(), 22);
    let mut ethernet = BSSID.to_vec();
    ethernet.extend_from_slice(&OWN);
    ethernet.extend_from_slice(&[0x08, 0x00]);
    ethernet.extend_from_slice(b"at 11 Mb/s");
    air.run(|s, _, out, tx| s.send(&ethernet, out, tx)).unwrap();
    assert!(air.sent.iter().all(|(_, rate)| *rate <= 22));
    assert_eq!(air.ap.received.last().map(|(t, p)| (*t, p.as_slice())), Some((0x0800, &b"at 11 Mb/s"[..])));
}

#[test]
fn an_access_point_that_wants_ofdm_refuses_an_802_11b_station() {
    let mut ap = wpa2();
    ap.status = frame::STATUS_BASIC_RATES as u8;
    let mut air = Air::with(ap, false);
    air.scan();
    let pmk = keys::pmk(PASSPHRASE, SSID).map(Secret::Pmk);
    air.run(|s, now, out, tx| s.join(SSID, pmk, now, out, tx)).unwrap();
    assert_eq!((air.station.state(), air.station.reason()), (State::Failed, Reason::Unsupported));
}

#[test]
fn a_wrong_passphrase_fails_the_handshake() {
    let mut air = Air::new(wpa2());
    air.scan();
    let wrong = keys::pmk(b"incorrect horse", SSID).map(Secret::Pmk);
    air.run(|s, now, out, tx| s.join(SSID, wrong, now, out, tx)).unwrap();
    assert_eq!(air.station.state(), State::Joining);
    air.wait(HANDSHAKE_MS + 20);
    assert_eq!((air.station.state(), air.station.reason()), (State::Failed, Reason::WrongPassphrase));
    // It left with a deauthentication.
    assert_eq!(air.sent.last().unwrap().0[0], frame::DEAUTHENTICATION << 4);
    // Without a key a WPA2 network is not tried.
    assert_eq!(air.run(|s, now, out, tx| s.join(SSID, None, now, out, tx)), Err(Error::Invalid));
}

#[test]
fn silence_refusal_and_a_downgrade_fail_the_join() {
    let pmk = keys::pmk(PASSPHRASE, SSID).map(Secret::Pmk);
    let mut air = Air::new(wpa2());
    air.scan();
    air.ap.silent = true;
    air.run(|s, now, out, tx| s.join(SSID, pmk, now, out, tx)).unwrap();
    air.wait(1000);
    assert_eq!((air.station.state(), air.station.reason()), (State::Failed, Reason::NoAnswer));
    assert_eq!(air.sent.iter().filter(|(f, _)| f[0] == frame::AUTHENTICATION << 4).count(), 3);

    air.ap.silent = false;
    air.ap.refuse = true;
    air.run(|s, now, out, tx| s.join(SSID, pmk, now, out, tx)).unwrap();
    assert_eq!((air.station.state(), air.station.reason()), (State::Failed, Reason::Refused));

    // The beacon says CCMP; message 3 carries a TKIP element: the station leaves.
    let mut air = Air::new(wpa2());
    let mut weaker = scan::own_rsne().to_vec();
    weaker[13] = 2;
    air.ap.handshake_rsne = Some(weaker);
    air.scan();
    air.run(|s, now, out, tx| s.join(SSID, pmk, now, out, tx)).unwrap();
    assert_eq!((air.station.state(), air.station.reason()), (State::Failed, Reason::Refused));
    assert_eq!(air.sent.last().unwrap().0[0], frame::DEAUTHENTICATION << 4);
    assert!(!air.ap.confirmed);
}

#[test]
fn an_open_network_passes_frames_in_the_clear() {
    let mut air = Air::new(Ap::new(None));
    air.scan();
    assert_eq!(air.station.networks().iter().next().unwrap().security, Security::Open);
    air.run(|s, now, out, tx| s.join(SSID, None, now, out, tx)).unwrap();
    assert_eq!(air.station.state(), State::Joined);
    let f = air.ap.data(&OWN, &BSSID, 0x0800, b"open", false);
    air.hear(&f).unwrap();
    assert_eq!(air.delivered.len(), 1);
}

#[test]
fn a_lost_network_is_joined_again() {
    let mut air = joined();
    // A deauthentication from the access point.
    let mut deauth = air.ap.header(frame::DEAUTHENTICATION, &OWN);
    deauth.extend_from_slice(&[7, 0]);
    air.ap.tk = None;
    air.ap.confirmed = false;
    air.hear(&deauth).unwrap();
    assert_eq!((air.station.state(), air.station.reason()), (State::Joining, Reason::Lost));
    air.wait(REJOIN_MS + 20);
    assert_eq!(air.station.state(), State::Joined);
    assert!(air.ap.confirmed);
    // Silence for SILENCE_MS is a loss too; beacons keep the link.
    for _ in 0..3 { air.wait(SILENCE_MS / 2); let b = air.ap.beacon(); air.hear(&b).unwrap(); }
    assert_eq!(air.station.state(), State::Joined);
    air.wait(SILENCE_MS + 20);
    assert_eq!(air.station.reason(), Reason::Lost);
}

#[test]
fn a_kept_network_takes_its_keys_from_the_key_service() {
    let mut air = Air::new(wpa2());
    air.scan();
    air.run(|s, now, out, tx| s.join(SSID, Some(Secret::Kept), now, out, tx)).unwrap();
    assert_eq!(air.station.state(), State::Joined, "{:?}", air.station.reason());
    assert_eq!(air.service.asked, 1);
    // Data under the keys it was given.
    let echo = air.ap.data(&OWN, &BSSID, 0x0800, b"kept", true);
    air.hear(&echo).unwrap();
    assert_eq!(air.delivered.len(), 1);

    // A handshake frame in the clear once the keys are installed is a forgery: dropped, the service not asked.
    let mut m = [0u8; EAPOL_MAX];
    let len = Authenticator::new(air.ap.pmk, BSSID, OWN, &scan::own_rsne(), [0x5A; 32]).unwrap().message_1(&mut m).unwrap();
    let forged = air.ap.data(&OWN, &BSSID, ETHERTYPE_EAPOL, &m[..len], false);
    assert_eq!(air.hear(&forged), Err(Error::Unexpected));
    assert_eq!((air.station.state(), air.service.asked), (State::Joined, 1));

    // Joining again after a loss asks again; a service that gives no keys ends the join.
    let mut deauth = air.ap.header(frame::DEAUTHENTICATION, &OWN);
    deauth.extend_from_slice(&[7, 0]);
    air.ap.tk = None;
    air.ap.confirmed = false;
    air.hear(&deauth).unwrap();
    air.service.refuse = true;
    air.wait(REJOIN_MS + 20);
    assert_eq!((air.station.state(), air.station.reason(), air.service.asked), (State::Failed, Reason::Refused, 2));
    assert_eq!(air.sent.last().unwrap().0[0], frame::DEAUTHENTICATION << 4);
    assert!(!air.ap.confirmed);
}

#[test]
fn networks_it_does_not_join() {
    let mut air = Air::new(Ap::new(Some(vec![0x30, 20, 1, 0, 0x00, 0x0F, 0xAC, 4, 1, 0, 0x00, 0x0F, 0xAC, 4, 1, 0, 0x00, 0x0F, 0xAC, 8, 0x80, 0])));
    air.scan();
    assert_eq!(air.station.networks().iter().next().unwrap().security, Security::Wpa3);
    assert_eq!(air.run(|s, now, out, tx| s.join(SSID, None, now, out, tx)), Err(Error::Unsupported));
    assert_eq!(air.run(|s, now, out, tx| s.join(b"elsewhere", None, now, out, tx)), Err(Error::NotFound));
    assert_eq!(air.station.reason(), Reason::NotFound);
    // The security of a few elements.
    assert_eq!(scan::security(&[0x30, 20, 1, 0, 0x00, 0x0F, 0xAC, 2, 1, 0, 0x00, 0x0F, 0xAC, 4, 1, 0, 0x00, 0x0F, 0xAC, 2, 0, 0]), Security::Other);
    assert_eq!(scan::security(&[0x30, 20, 1, 0, 0x00, 0x0F, 0xAC, 4, 1, 0, 0x00, 0x0F, 0xAC, 4, 1, 0, 0x00, 0x0F, 0xAC, 1, 0, 0]), Security::Enterprise);
    assert_eq!(scan::security(&[0x30, 20, 1, 0, 0x00, 0x0F, 0xAC, 4, 1, 0, 0x00, 0x0F, 0xAC, 4, 1, 0, 0x00, 0x0F, 0xAC, 2, 0x40, 0]), Security::Other);
    assert_eq!(scan::security(&[0x30, 3, 1, 0, 0]), Security::Other);
}

#[test]
fn frames_are_read_carefully() {
    assert_eq!(frame::header(&[0x80, 0]), Err(Error::Malformed));
    assert_eq!(frame::header(&[0xD4, 0, 0, 0, 1, 2, 3, 4, 5, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]), Err(Error::Malformed)); // control
    let mut qos = vec![0x88, 0x03];
    qos.extend_from_slice(&[0; 28]);
    assert_eq!(frame::header(&qos), Err(Error::Malformed)); // four addresses and QoS control need 32 bytes
    qos.extend_from_slice(&[5, 0]);
    let h = frame::header(&qos).unwrap();
    assert_eq!((h.len, h.tid, h.a4.is_some()), (32, Some(5), true));
    let elements = [0, 3, b'a', b'b', b'c', 1, 9, 1];
    assert_eq!(frame::Elements::new(&elements).count(), 1);
    assert_eq!(frame::Elements::find_whole(&elements, 1), None);
}
