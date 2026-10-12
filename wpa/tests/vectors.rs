//! Host tests of the WPA2-PSK library (550-NET-0016): IEEE 802.11 J.4 (PMK) and J.3 (PRF) vectors, RFC 3394 key wrap,
//! the M.6.4 CCMP vector, the PTK (cross-checked with Python's hashlib), and both handshakes against an authenticator
//! written here from 12.7.6 and 12.7.7.
use aes_kw::KekAes128;
use wpa::ccmp::{self, Replay};
use wpa::eapol::{self, Gtk, ACK, ENCRYPTED, INSTALL, MIC, PAIRWISE, SECURE, VERSION_SHA1_AES};
use wpa::handshake::{Supplicant, REPLY_MAX};
use wpa::keys;
use wpa::Error;

fn hex(text: &str) -> Vec<u8> {
    let text: String = text.split_whitespace().collect();
    (0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap()).collect()
}

#[test]
fn pmk_of_a_passphrase() {
    // IEEE 802.11 J.4.2.
    for (passphrase, ssid, expected) in [
        ("password", "IEEE", "f42c6fc52df0ebef9ebb4b90b38a5f902e83fe1b135a70e23aed762e9710a12e"),
        ("ThisIsAPassword", "ThisIsASSID", "0dc0d6eb90555ed6419756b9a15ec3e3209b63df707dd508d14581f8982721af"),
        ("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ", "becb93866bb8c3832cb777c2f559807c8c59afcb6eae734885001300a981cc62"),
    ] {
        assert_eq!(keys::pmk(passphrase.as_bytes(), ssid.as_bytes()).unwrap().to_vec(), hex(expected), "{passphrase}");
    }
    assert!(keys::pmk(b"short", b"IEEE").is_none());
    assert!(keys::pmk(&[b'a'; 64], b"IEEE").is_none());
    assert!(keys::pmk(b"pass\x01word", b"IEEE").is_none());
    assert!(keys::pmk(b"password", &[b'Z'; 33]).is_none());
    assert_eq!(keys::psk_hex(b"0dc0d6eb90555ed6419756b9a15ec3e3209b63df707dd508d14581f8982721AF").unwrap().to_vec(),
               hex("0dc0d6eb90555ed6419756b9a15ec3e3209b63df707dd508d14581f8982721af"));
    assert!(keys::psk_hex(b"0dc0").is_none());
}

#[test]
fn prf_sha1() {
    // IEEE 802.11 J.3.2 (PRF-512, label "prefix").
    let cases: [(Vec<u8>, Vec<u8>, &str); 3] = [
        (vec![0x0b; 20], b"Hi There".to_vec(),
         "bcd4c650b30b9684951829e0d75f9d54b862175ed9f00606e17d8da35402ffee75df78c3d31e0f889f012120c0862beb67753e7439ae242edb8373698356cf5a"),
        (b"Jefe".to_vec(), b"what do ya want for nothing?".to_vec(),
         "51f4de5b33f249adf81aeb713a3c20f4fe631446fabdfa58244759ae58ef9009a99abf4eac2ca5fa87e692c440eb40023e7babb206d61de7b92f41529092b8fc"),
        (vec![0xaa; 20], vec![0xdd; 50],
         "e1ac546ec4cb636f9976487be5c86be17a0252ca5d8d8df12cfb0473525249ce9dd8d177ead710bc9b590547239107aef7b4abd43d87f0a68f1cbd9e2b6f7607"),
    ];
    for (key, data, expected) in cases {
        let mut out = [0u8; 64];
        keys::prf(&key, b"prefix", &data, &mut out);
        assert_eq!(out.to_vec(), hex(expected));
    }
}

const AA: [u8; 6] = [0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5];
const SPA: [u8; 6] = [0xb0, 0xb1, 0xb2, 0xb3, 0xb4, 0xb5];

fn anonce() -> [u8; 32] { core::array::from_fn(|i| 0xe0 + i as u8) }
fn snonce() -> [u8; 32] { core::array::from_fn(|i| 0xc0 + i as u8) }

#[test]
fn pairwise_key_expansion() {
    // Expected values from Python's hashlib and hmac over the same inputs (an independent implementation).
    let pmk = keys::pmk(b"ThisIsAPassword", b"ThisIsASSID").unwrap();
    let ptk = keys::ptk(&pmk, &AA, &SPA, &anonce(), &snonce());
    assert_eq!(ptk.kck.to_vec(), hex("e677aae82af1fb9a25e93e22ad77022e"));
    assert_eq!(ptk.kek.to_vec(), hex("d90489e672adcc6ae148d0afafb92b69"));
    assert_eq!(ptk.tk.to_vec(), hex("36606222ae74d4d2d1db3ac91f6c2809"));
    // The order of the addresses and nonces does not matter.
    assert!(keys::ptk(&pmk, &SPA, &AA, &snonce(), &anonce()) == ptk);
}

#[test]
fn key_unwrap_rfc_3394() {
    // RFC 3394 4.1: 128 bits of key data under a 128-bit KEK.
    let kek: [u8; 16] = hex("000102030405060708090A0B0C0D0E0F").try_into().unwrap();
    let mut out = [0u8; eapol::KEY_DATA_MAX];
    let plain = eapol::unwrap(&kek, &hex("1FA68B0A8112B447 AEF34BD8FB5A7B82 9D3E862371D2CFE5"), &mut out).unwrap();
    assert_eq!(plain.to_vec(), hex("00112233445566778899AABBCCDDEEFF"));
    let mut bad = hex("1FA68B0A8112B447 AEF34BD8FB5A7B82 9D3E862371D2CFE5");
    bad[3] ^= 1;
    assert_eq!(eapol::unwrap(&kek, &bad, &mut out).unwrap_err(), Error::KeyData);
    assert_eq!(eapol::unwrap(&kek, &bad[..20], &mut out).unwrap_err(), Error::KeyData);
}

#[test]
fn ccmp_vector() {
    // IEEE 802.11 M.6.4; the encrypted MPDU also matches OpenSSL's AES-CCM over the nonce and AAD of 12.5.3.3.
    let tk: [u8; 16] = hex("c97c1f67ce371185514a8a19f2bdd52f").try_into().unwrap();
    let pn = 0xB5039776E70C;
    let plain = hex("0848c32c0fd2e128a57c5030f1844408abaea5b8fcba8033 f8ba1a55d02f85ae967bb62fb6cda8eb7e78a050");
    let expected = hex("0848c32c0fd2e128a57c5030f1844408abaea5b8fcba8033 0ce70020769703b5
                        f3d0a2fe9a3dbf2342a643e43246e80c3c04d0197845ce0b16f97623");
    let mut out = [0u8; 128];
    let len = ccmp::encrypt(&tk, 0, pn, &plain, &mut out).unwrap();
    // The vector's header has the Protected bit (0x40 in byte 1) set already.
    assert_eq!(out[..len].to_vec(), expected);
    assert_eq!(ccmp::peek(&expected).unwrap(), (0, pn));
    let mut back = [0u8; 128];
    let (len, got) = ccmp::decrypt(&tk, &expected, &mut back).unwrap();
    assert_eq!(got, pn);
    assert_eq!(back[24..len].to_vec(), plain[24..].to_vec());
    assert_eq!(back[1], plain[1] & !0x40);
    // A changed byte of the body, of the masked-in header, or of the MIC is refused.
    for at in [30, 4, expected.len() - 1] {
        let mut bad = expected.clone();
        bad[at] ^= 0x01;
        assert_eq!(ccmp::decrypt(&tk, &bad, &mut back).unwrap_err(), Error::Mic, "byte {at}");
    }
    // The Retry bit is masked out of the AAD: a retransmission still decrypts.
    let mut retry = expected.clone();
    retry[1] |= 0x08;
    assert!(ccmp::decrypt(&tk, &retry, &mut back).is_ok());
}

#[test]
fn ccmp_qos_round_trip_and_replay() {
    let tk = [7u8; 16];
    // A QoS data frame, ToDS, TID 5.
    let mut frame = hex("8801 3a01 000102030405 101112131415 202122232425 3000 0500");
    frame.extend_from_slice(b"\xaa\xaa\x03\x00\x00\x00\x08\x00payload");
    let mut out = [0u8; 128];
    let len = ccmp::encrypt(&tk, 1, 42, &frame, &mut out).unwrap();
    assert_eq!(ccmp::peek(&out[..len]).unwrap(), (1, 42));
    assert_eq!(ccmp::tid(&out[..len]).unwrap(), 5);
    let mut back = [0u8; 128];
    let (n, pn) = ccmp::decrypt(&tk, &out[..len], &mut back).unwrap();
    assert_eq!((back[..n].to_vec(), pn), (frame.clone(), 42));
    // Another TID in the QoS control field changes the nonce: refused.
    let mut moved = out[..len].to_vec();
    moved[24] = 6;
    assert_eq!(ccmp::decrypt(&tk, &moved, &mut back).unwrap_err(), Error::Mic);
    let mut replay = Replay::starting(40);
    assert_eq!(replay.accept(5, 40), Err(Error::Replay));
    assert_eq!(replay.accept(5, 42), Ok(()));
    assert_eq!(replay.accept(5, 42), Err(Error::Replay));
    assert_eq!(replay.accept(6, 41), Ok(()));
    assert!(ccmp::encrypt(&tk, 0, 0, &frame, &mut out).is_err());
    assert!(ccmp::encrypt(&tk, 0, 1 << 48, &frame, &mut out).is_err());
}

// The test authenticator: the RSN element of a WPA2-PSK CCMP network, and the frames of 12.7.6 and 12.7.7.
const RSNE: [u8; 22] = [0x30, 20, 1, 0, 0x00, 0x0f, 0xac, 4, 1, 0, 0x00, 0x0f, 0xac, 4, 1, 0, 0x00, 0x0f, 0xac, 2, 0x0c, 0];

struct Authenticator { pmk: [u8; 32], replay: u64, ptk: Option<keys::Ptk> }

impl Authenticator {
    fn message_1(&mut self) -> Vec<u8> {
        self.replay += 1;
        let mut out = [0u8; 256];
        let len = eapol::write(&mut out, 2, VERSION_SHA1_AES | PAIRWISE | ACK, self.replay, &anonce(), &[]).unwrap();
        out[7..9].copy_from_slice(&16u16.to_be_bytes()); // key length: CCMP's 16
        out[..len].to_vec()
    }
    // Checks message 2 as the authenticator does: its MIC under the PTK of its SNonce, and the station's RSN element.
    fn take_message_2(&mut self, frame: &[u8]) {
        let key = eapol::parse(frame).unwrap();
        assert_eq!(key.info, VERSION_SHA1_AES | PAIRWISE | MIC);
        assert_eq!(key.replay, self.replay);
        assert_eq!(key.data, &RSNE[..]);
        let ptk = keys::ptk(&self.pmk, &AA, &SPA, &anonce(), &key.nonce);
        assert!(eapol::verify(&ptk.kck, frame, &key));
        self.ptk = Some(ptk);
    }
    fn key_data(&self, rsne: &[u8], gtk: &Gtk) -> Vec<u8> {
        let mut plain = rsne.to_vec();
        plain.extend_from_slice(&[0xdd, 22, 0x00, 0x0f, 0xac, 1, gtk.id, 0]);
        plain.extend_from_slice(&gtk.key);
        plain.push(0xdd);
        while plain.len() % 8 != 0 { plain.push(0); }
        let mut wrapped = vec![0u8; plain.len() + 8];
        KekAes128::from(self.ptk.as_ref().unwrap().kek).wrap(&plain, &mut wrapped).unwrap();
        wrapped
    }
    fn signed(&mut self, info: u16, nonce: &[u8; 32], data: &[u8], rsc: [u8; 8]) -> Vec<u8> {
        self.replay += 1;
        let mut out = [0u8; 512];
        let len = eapol::write(&mut out, 2, info, self.replay, nonce, data).unwrap();
        out[eapol::HEADER + 61..eapol::HEADER + 69].copy_from_slice(&rsc);
        eapol::sign(&self.ptk.as_ref().unwrap().kck, &mut out[..len]).unwrap();
        out[..len].to_vec()
    }
    fn message_3(&mut self, rsne: &[u8], gtk: &Gtk) -> Vec<u8> {
        let data = self.key_data(rsne, gtk);
        self.signed(VERSION_SHA1_AES | PAIRWISE | ACK | MIC | INSTALL | SECURE | ENCRYPTED, &anonce(), &data, [9, 0, 0, 0, 0, 0, 0, 0])
    }
    fn group(&mut self, gtk: &Gtk) -> Vec<u8> {
        let data = self.key_data(&[], gtk);
        self.signed(VERSION_SHA1_AES | ACK | MIC | SECURE | ENCRYPTED, &[0; 32], &data, [3, 1, 0, 0, 0, 0, 0, 0])
    }
}

fn station() -> (Supplicant, Authenticator) {
    let pmk = keys::pmk(b"ThisIsAPassword", b"ThisIsASSID").unwrap();
    (Supplicant::new(pmk, SPA, AA, &RSNE, &RSNE).unwrap(), Authenticator { pmk, replay: 0, ptk: None })
}

#[test]
fn four_way_and_group_handshakes() {
    let (mut supplicant, mut ap) = station();
    let mut out = [0u8; REPLY_MAX];
    let gtk = Gtk { id: 1, key: [0x5a; 16] };

    let step = supplicant.receive(&ap.message_1(), &snonce(), &mut out).unwrap();
    assert!(step.pairwise.is_none() && step.group.is_none());
    ap.take_message_2(&out[..step.reply]);
    let tk = ap.ptk.as_ref().unwrap().tk;

    let message_3 = ap.message_3(&RSNE, &gtk);
    let step = supplicant.receive(&message_3, &[0; 32], &mut out).unwrap();
    assert_eq!(step.pairwise, Some(tk));
    assert_eq!(step.group, Some((gtk, [9, 0, 0, 0, 0, 0, 0, 0])));
    let message_4 = eapol::parse(&out[..step.reply]).unwrap();
    assert_eq!(message_4.info, VERSION_SHA1_AES | PAIRWISE | MIC | SECURE);
    assert_eq!(message_4.replay, ap.replay);
    assert!(message_4.data.is_empty() && message_4.nonce == [0; 32]);
    assert!(eapol::verify(&ap.ptk.as_ref().unwrap().kck, &out[..step.reply], &message_4));

    // The same message 3 again: a replay, dropped.
    assert_eq!(supplicant.receive(&message_3, &[0; 32], &mut out).unwrap_err(), Error::Replay);
    // A retransmitted message 3 (a higher counter) is answered, but its keys are not handed out again.
    let step = supplicant.receive(&ap.message_3(&RSNE, &gtk), &[0; 32], &mut out).unwrap();
    assert!(step.reply > 0 && step.pairwise.is_none() && step.group.is_none());

    // A new GTK through the group key handshake, once.
    let new = Gtk { id: 2, key: [0xa5; 16] };
    let group = ap.group(&new);
    let step = supplicant.receive(&group, &[0; 32], &mut out).unwrap();
    assert_eq!(step.group, Some((new, [3, 1, 0, 0, 0, 0, 0, 0])));
    let reply = eapol::parse(&out[..step.reply]).unwrap();
    assert_eq!(reply.info, VERSION_SHA1_AES | MIC | SECURE);
    assert!(eapol::verify(&ap.ptk.as_ref().unwrap().kck, &out[..step.reply], &reply));
    assert_eq!(supplicant.receive(&group, &[0; 32], &mut out).unwrap_err(), Error::Replay);
    let step = supplicant.receive(&ap.group(&new), &[0; 32], &mut out).unwrap();
    assert!(step.reply > 0 && step.group.is_none());
}

#[test]
fn handshake_refusals() {
    let mut out = [0u8; REPLY_MAX];
    let gtk = Gtk { id: 1, key: [0x5a; 16] };

    // Message 3 before message 1, and a group message before the 4-way handshake.
    let (mut supplicant, mut ap) = station();
    let message_1 = ap.message_1();
    let mut other = [0u8; REPLY_MAX];
    let step = supplicant.receive(&message_1, &snonce(), &mut other).unwrap();
    ap.take_message_2(&other[..step.reply]);
    let (mut fresh, _) = station();
    assert_eq!(fresh.receive(&ap.message_3(&RSNE, &gtk), &[0; 32], &mut out).unwrap_err(), Error::Unexpected);
    assert_eq!(fresh.receive(&ap.group(&gtk), &[0; 32], &mut out).unwrap_err(), Error::Unexpected);

    // A wrong MIC, then an RSN element other than the advertised one (a downgrade).
    let mut bad = ap.message_3(&RSNE, &gtk);
    bad[eapol::HEADER + 80] ^= 1;
    assert_eq!(supplicant.receive(&bad, &[0; 32], &mut out).unwrap_err(), Error::Mic);
    let mut weaker = RSNE;
    weaker[13] = 2; // TKIP pairwise
    assert_eq!(supplicant.receive(&ap.message_3(&weaker, &gtk), &[0; 32], &mut out).unwrap_err(), Error::Downgrade);
    assert!(supplicant.ptk().is_none());

    // Message 3 with another ANonce than message 1's.
    let data = ap.key_data(&RSNE, &gtk);
    let other_nonce = ap.signed(VERSION_SHA1_AES | PAIRWISE | ACK | MIC | INSTALL | SECURE | ENCRYPTED, &[1; 32], &data, [0; 8]);
    assert_eq!(supplicant.receive(&other_nonce, &[0; 32], &mut out).unwrap_err(), Error::Unexpected);

    // Without the encrypted key data bit, and a frame from the station's side (no Ack).
    let plain = ap.signed(VERSION_SHA1_AES | PAIRWISE | ACK | MIC | INSTALL | SECURE, &anonce(), &data, [0; 8]);
    assert_eq!(supplicant.receive(&plain, &[0; 32], &mut out).unwrap_err(), Error::Malformed);
    let mut no_ack = message_1.clone();
    no_ack[6] &= !(ACK as u8);
    assert_eq!(supplicant.receive(&no_ack, &[0; 32], &mut out).unwrap_err(), Error::Unexpected);

    // Truncated frames and a key data length beyond the body.
    for len in [0, 4, 50, eapol::FIXED - 1] {
        assert_eq!(supplicant.receive(&message_1[..len.min(message_1.len())], &[0; 32], &mut out).unwrap_err(), Error::Malformed);
    }
    let mut long = message_1.clone();
    long[eapol::FIXED - 1] = 8;
    assert_eq!(supplicant.receive(&long, &[0; 32], &mut out).unwrap_err(), Error::Malformed);

    // The good message 3 still completes the handshake after all that.
    let step = supplicant.receive(&ap.message_3(&RSNE, &gtk), &[0; 32], &mut out).unwrap();
    assert!(step.pairwise.is_some());
}

#[test]
fn key_data_elements() {
    let mut data = RSNE.to_vec();
    data.extend_from_slice(&[0xdd, 22, 0x00, 0x0f, 0xac, 1, 0x02, 0]);
    data.extend_from_slice(&[0x11; 16]);
    data.extend_from_slice(&[0xdd, 0, 0, 0]);
    let found = eapol::elements(&data).unwrap();
    assert_eq!(found.rsne, Some(&RSNE[..]));
    assert_eq!(found.gtk, Some(Gtk { id: 2, key: [0x11; 16] }));
    assert_eq!(eapol::elements(&[0x30, 20, 1]).unwrap_err(), Error::KeyData);
    assert_eq!(eapol::elements(&[0xdd, 10, 0x00, 0x0f, 0xac, 1, 0, 0, 1, 2, 3, 4]).unwrap_err(), Error::KeyData);
    assert!(eapol::elements(&[]).unwrap().gtk.is_none());
}
