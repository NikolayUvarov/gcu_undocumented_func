// timed's NTS (111-NET-0075): AES-SIV against RFC 5297's examples, NTS-KE's records, the NTP request's extension
// fields as a server reads them, the answer's checks (replayed, forged, a NAK), and the parsers on random and mutated
// input. Evidence of these executions, not a proof (MC-12.2).
#[cfg(test)]
#[path = "../libmind/src/nts.rs"]
#[allow(dead_code)]
mod nts;
#[cfg(test)]
#[path = "../timed/src/siv.rs"]
mod siv;

#[cfg(test)]
mod tests {
    use super::nts::*;
    use super::siv;

    fn hex(text: &str) -> Vec<u8> {
        let text: String = text.split_whitespace().collect();
        (0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap()).collect()
    }

    fn key(text: &str) -> [u8; 32] { hex(text).try_into().unwrap() }

    #[test]
    fn aes_siv_rfc_5297_examples() {
        // A.1, deterministic: one header.
        let k = key("fffefdfc fbfaf9f8 f7f6f5f4 f3f2f1f0 f0f1f2f3 f4f5f6f7 f8f9fafb fcfdfeff");
        let ad = hex("10111213 14151617 18191a1b 1c1d1e1f 20212223 24252627");
        let sealed = siv::seal(&k, &[&ad], &hex("11223344 55667788 99aabbcc ddee"));
        assert_eq!(sealed, hex("85632d07 c6e8f37f 950acd32 0a2ecc93 40c02b96 90c4dc04 daef7f6a fe5c"));
        assert_eq!(siv::open(&k, &[&ad], &sealed), Some(hex("11223344 55667788 99aabbcc ddee")));
        // A.2, nonce-based: two associated data and the nonce last, as NTS passes them.
        let k = key("7f7e7d7c 7b7a7978 77767574 73727170 40414243 44454647 48494a4b 4c4d4e4f");
        let ad1 = hex("00112233 44556677 8899aabb ccddeeff deaddada deaddada ffeeddcc bbaa9988 77665544 33221100");
        let ad2 = hex("10203040 50607080 90a0");
        let nonce = hex("09f91102 9d74e35b d84156c5 635688c0");
        let plain = hex("74686973 20697320 736f6d65 20706c61 696e7465 78742074 6f20656e 63727970 74207573 696e6720 5349562d 414553");
        let sealed = siv::seal(&k, &[&ad1, &ad2, &nonce], &plain);
        assert_eq!(sealed, hex("7bdb6e3b 432667eb 06f4d14b ff2fbd0f cb900f2f ddbe4043 26601965 c889bf17 dba77ceb 094fa663 b7a3f748 ba8af829 ea64ad54 4a272e9c 485b62a3 fd5c0d"));
        let mut forged = sealed.clone();
        forged[20] ^= 1;
        assert_eq!(siv::open(&k, &[&ad1, &ad2, &nonce], &forged), None);
        assert_eq!(siv::open(&k, &[&ad1, &nonce], &sealed), None);
    }

    fn records(list: &[(u16, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        for (kind, body) in list {
            out.extend_from_slice(&kind.to_be_bytes());
            out.extend_from_slice(&(body.len() as u16).to_be_bytes());
            out.extend_from_slice(body);
        }
        out
    }

    const C: u16 = 0x8000;

    #[test]
    fn the_ke_request_and_context() {
        assert_eq!(ke_request(), hex("8001 0002 0000 8004 0002 000f 8000 0000"));
        assert_eq!(context(false), [0, 0, 0, 15, 0]);
        assert_eq!(context(true), [0, 0, 0, 15, 1]);
    }

    #[test]
    fn ke_responses_taken_and_refused() {
        let good = |extra: &[(u16, &[u8])]| {
            let mut list: Vec<(u16, &[u8])> = vec![(C | 1, &[0, 0]), (C | 4, &[0, 15])];
            list.extend_from_slice(extra);
            list.push((C, &[]));
            records(&list)
        };
        let ke = ke_response(&good(&[(5, b"cookie-one"), (5, b"cookie-two"), (6, b"10.0.2.2"), (C | 7, &[0x2b, 0x67]), (0x4321, b"ignored")])).unwrap();
        assert_eq!(ke.cookies, vec![b"cookie-one".to_vec(), b"cookie-two".to_vec()]);
        assert_eq!((ke.server.as_deref(), ke.port), (Some("10.0.2.2"), Some(11111)));
        // At most COOKIES taken; bytes after the end not looked at.
        let many: Vec<(u16, &[u8])> = (0..12).map(|_| (5u16, &b"c"[..])).collect();
        let mut data = good(&many);
        data.extend_from_slice(b"\xff\xff\xff");
        assert_eq!(ke_response(&data).unwrap().cookies.len(), COOKIES);
        // Not yet whole.
        let whole = good(&[(5, b"c")]);
        for cut in 0..whole.len() { assert_eq!(ke_response(&whole[..cut]), Err(KeRefused::Incomplete), "{cut}"); }
        assert_eq!(ke_response(&records(&[(C | 2, &[0, 1]), (C, &[])])), Err(KeRefused::Error(1)));
        assert_eq!(ke_response(&good(&[(C | 3, &[0, 9])])), Err(KeRefused::Warning(9)));
        assert_eq!(ke_response(&good(&[(5, b"c"), (C | 1, &[0, 0])])), Err(KeRefused::Protocol));
        assert_eq!(ke_response(&records(&[(C | 1, &[0, 0, 0, 1]), (C | 4, &[0, 15]), (5, b"c"), (C, &[])])), Err(KeRefused::Protocol));
        assert_eq!(ke_response(&records(&[(C | 1, &[]), (C | 4, &[0, 15]), (5, b"c"), (C, &[])])), Err(KeRefused::Protocol));
        assert_eq!(ke_response(&records(&[(C | 4, &[0, 15]), (5, b"c"), (C, &[])])), Err(KeRefused::Protocol));
        assert_eq!(ke_response(&records(&[(C | 1, &[0, 0]), (C | 4, &[0, 16]), (5, b"c"), (C, &[])])), Err(KeRefused::Aead));
        assert_eq!(ke_response(&records(&[(C | 1, &[0, 0]), (5, b"c"), (C, &[])])), Err(KeRefused::Aead));
        assert_eq!(ke_response(&good(&[(5, b"c"), (C | 0x4321, b"x")])), Err(KeRefused::Critical(0x4321)));
        assert_eq!(ke_response(&good(&[])), Err(KeRefused::Cookie));
        assert_eq!(ke_response(&good(&[(5, b"")])), Err(KeRefused::Cookie));
        assert_eq!(ke_response(&good(&[(5, &[1; COOKIE_MAX + 1])])), Err(KeRefused::Cookie));
        assert_eq!(ke_response(&good(&[(5, b"c"), (7, &[1])])), Err(KeRefused::Malformed));
        assert_eq!(ke_response(&good(&[(5, b"c"), (6, b"")])), Err(KeRefused::Malformed));
        assert_eq!(ke_response(&good(&[(5, b"c"), (6, b"a b")])), Err(KeRefused::Malformed));
        assert_eq!(ke_response(&vec![0x40; RESPONSE_MAX + 8]), Err(KeRefused::TooLong));
    }

    const C2S: [u8; 32] = [0x11; 32];
    const S2C: [u8; 32] = [0x22; 32];

    // What a server does with the request: its fields, the client's tag checked; the answer it seals for `uid`.
    fn serve(request: &[u8], cookies: usize, flip: Option<usize>, other_uid: bool) -> Vec<u8> {
        let list = fields(request, 48, 16).expect("the request's fields parse");
        let kinds: Vec<u16> = list.iter().map(|f| f.0).collect();
        assert_eq!(kinds[..2], [FIELD_UID, FIELD_COOKIE]);
        assert_eq!(*kinds.last().unwrap(), FIELD_AUTHENTICATOR);
        let (_, auth, start) = *list.last().unwrap();
        assert_eq!((u16::from_be_bytes([auth[0], auth[1]]), u16::from_be_bytes([auth[2], auth[3]])), (NONCE as u16, TAG as u16));
        assert_eq!(auth.len(), 4 + NONCE + TAG, "no additional padding with a 16-byte nonce");
        assert_eq!(siv::open(&C2S, &[&request[..start], &auth[4..4 + NONCE]], &auth[4 + NONCE..]), Some(vec![]));
        let uid = if other_uid { vec![0x99; UID] } else { list[0].1.to_vec() };
        let mut answer = request[..48].to_vec();
        answer[0] = 0x24;
        answer.extend_from_slice(&FIELD_UID.to_be_bytes());
        answer.extend_from_slice(&36u16.to_be_bytes());
        answer.extend_from_slice(&uid);
        let mut plain = Vec::new();
        for n in 0..cookies {
            plain.extend_from_slice(&FIELD_COOKIE.to_be_bytes());
            plain.extend_from_slice(&12u16.to_be_bytes());
            plain.extend_from_slice(&[n as u8; 8]);
        }
        let nonce = [7u8; 12];
        let sealed = siv::seal(&S2C, &[&answer, &nonce], &plain);
        let value_length = 4 + 12 + ((sealed.len() + 3) & !3);
        answer.extend_from_slice(&FIELD_AUTHENTICATOR.to_be_bytes());
        answer.extend_from_slice(&((4 + value_length) as u16).to_be_bytes());
        answer.extend_from_slice(&12u16.to_be_bytes());
        answer.extend_from_slice(&(sealed.len() as u16).to_be_bytes());
        answer.extend_from_slice(&nonce);
        answer.extend_from_slice(&sealed);
        answer.resize(answer.len() + ((sealed.len() + 3) & !3) - sealed.len(), 0);
        if let Some(at) = flip { answer[at] ^= 0x40; }
        answer
    }

    fn open(ad: &[u8], nonce: &[u8], sealed: &[u8]) -> Option<Vec<u8>> { siv::open(&S2C, &[ad, nonce], sealed) }

    fn request_for(uid: &[u8; UID], cookie: &[u8], placeholders: usize) -> Vec<u8> {
        let header = [0x23u8; 48];
        request(&header, uid, cookie, placeholders, &[5; NONCE], |ad, nonce| siv::seal(&C2S, &[ad, nonce], &[]).try_into().unwrap())
    }

    #[test]
    fn a_request_and_its_answer() {
        let uid = [3u8; UID];
        // A cookie of 10 bytes is padded to 12; each placeholder as long.
        let request = request_for(&uid, b"0123456789", placeholders(3));
        assert_eq!(placeholders(3), 4);
        assert_eq!(placeholders(0), 7);
        assert_eq!(placeholders(9), 0);
        let list = fields(&request, 48, 16).unwrap();
        assert_eq!(list.iter().filter(|f| f.0 == FIELD_PLACEHOLDER).count(), 4);
        assert!(list.iter().filter(|f| f.0 == FIELD_PLACEHOLDER || f.0 == FIELD_COOKIE).all(|f| f.1.len() == 12));
        let answer = serve(&request, 5, None, false);
        assert_eq!(super::nts::answer(&answer, &uid, open).unwrap(), (0..5).map(|n| vec![n as u8; 8]).collect::<Vec<_>>());
    }

    #[test]
    fn replayed_forged_and_refused_answers() {
        let uid = [3u8; UID];
        let request = request_for(&uid, b"cookie!!", 0);
        let good = serve(&request, 1, None, false);
        // Another request's answer: a replay.
        assert_eq!(super::nts::answer(&good, &[4u8; UID], open), Err(Refused::NotOurs));
        assert_eq!(super::nts::answer(&serve(&request, 1, None, true), &uid, open), Err(Refused::NotOurs));
        // Any byte changed in the header, the identifier or the sealed part: not authentic, or not ours.
        for at in [0, 20, 47, 48 + 4, 48 + 35, good.len() - 1] {
            let mut forged = good.clone();
            forged[at] ^= 0x40;
            let got = super::nts::answer(&forged, &uid, open);
            assert!(matches!(got, Err(Refused::NotAuthentic) | Err(Refused::NotOurs) | Err(Refused::Fields)), "{at}: {got:?}");
        }
        let mut forged = good.clone();
        forged[30] ^= 1;
        assert_eq!(super::nts::answer(&forged, &uid, open), Err(Refused::NotAuthentic));
        // Sealing no cookie.
        assert_eq!(super::nts::answer(&serve(&request, 0, None, false), &uid, open), Err(Refused::NoCookie));
        // No authenticator: an unprotected answer.
        assert_eq!(super::nts::answer(&good[..48 + 36], &uid, open), Err(Refused::NoAuthenticator));
        assert_eq!(super::nts::answer(&good[..48], &uid, open), Err(Refused::NoAuthenticator));
        // The identifier only after the authenticator, where nothing is authenticated.
        let mut moved = good[..48].to_vec();
        moved.extend_from_slice(&good[48 + 36..]);
        moved.extend_from_slice(&good[48..48 + 36]);
        assert_eq!(super::nts::answer(&moved, &uid, open), Err(Refused::NotOurs));
        // A field whose length is not a multiple of 4, or runs past the end.
        let mut bad = good.clone();
        bad[48 + 3] = 37;
        assert_eq!(super::nts::answer(&bad, &uid, open), Err(Refused::Fields));
        assert_eq!(super::nts::answer(&good[..good.len() - 4], &uid, open), Err(Refused::Fields));
        // An NTS NAK for this request, and one for another.
        let mut nak = good[..48 + 36].to_vec();
        nak[1] = 0;
        nak[12..16].copy_from_slice(b"NTSN");
        assert_eq!(super::nts::answer(&nak, &uid, open), Err(Refused::Nak));
        assert_eq!(super::nts::answer(&nak, &[9u8; UID], open), Err(Refused::NotOurs));
    }

    // xorshift64*: the same inputs on every run of a seed.
    struct Random(u64);
    impl Random {
        fn next(&mut self) -> u64 { self.0 ^= self.0 >> 12; self.0 ^= self.0 << 25; self.0 ^= self.0 >> 27; self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) }
        fn below(&mut self, n: usize) -> usize { (self.next() % n as u64) as usize }
    }

    #[test]
    fn the_parsers_on_random_and_mutated_input() {
        let seed = std::env::var("MIND_FUZZ_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(0x4e54_5321u64);
        let rounds = std::env::var("MIND_FUZZ_ITERATIONS").ok().and_then(|s| s.parse().ok()).unwrap_or(3000usize);
        let mut random = Random(seed | 1);
        let uid = [3u8; UID];
        let request = request_for(&uid, b"cookie!!", 2);
        let answer = serve(&request, 3, None, false);
        let ke = records(&[(C | 1, &[0, 0]), (C | 4, &[0, 15]), (5, b"cookie"), (6, b"10.0.2.2"), (7, &[0, 123]), (C, &[])]);
        let (mut taken, mut answers) = (0, 0);
        for round in 0..rounds {
            let base: &[u8] = if round % 2 == 0 { &answer } else { &ke };
            let mut data = if round % 5 == 0 {
                (0..random.below(600)).map(|_| random.next() as u8).collect::<Vec<u8>>()
            } else {
                let mut data = base.to_vec();
                for _ in 0..1 + random.below(6) {
                    match random.below(4) {
                        0 if !data.is_empty() => { let at = random.below(data.len()); data[at] ^= 1 << random.below(8); }
                        1 if !data.is_empty() => { let at = random.below(data.len()); data[at] = random.next() as u8; }
                        2 => { let at = random.below(data.len() + 1); data.truncate(at); }
                        _ => { let at = random.below(data.len() + 1); data.insert(at, random.next() as u8); }
                    }
                }
                data
            };
            if ke_response(&data).is_ok() { taken += 1; }
            if let Ok(cookies) = super::nts::answer(&data, &uid, open) {
                answers += 1;
                assert!(!cookies.is_empty() && cookies.len() <= COOKIES);
            }
            let _ = fields(&data, 0, 4);
            data.clear();
        }
        assert!(taken > 0, "some mutated responses still parse");
        println!("{rounds} inputs: {taken} NTS-KE responses taken, {answers} answers authentic");
    }
}
