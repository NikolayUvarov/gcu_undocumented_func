// The TLS service's exporter (tls/src/exporter.rs, tls.wit 1.4, 111-NET-0073) against OpenSSL 3.0.13's: each vector is
// a connection's EXPORTER_SECRET from `openssl s_client -keylogfile` and the keying material both `s_client` and
// `s_server` printed for `-keymatexport EXPORTER-network-time-security -keymatexportlen 32` (no context, which TLS 1.3
// treats as an empty one), with the suite's hash.
#[cfg(test)]
#[path = "../tls/src/exporter.rs"]
mod exporter;

#[cfg(test)]
mod tests {
    use super::exporter::{export, forget, Hash, Secret, LENGTH_MAX};

    fn hex(text: &str) -> Vec<u8> { (0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap()).collect() }

    const LABEL: &[u8] = b"EXPORTER-network-time-security";

    #[test]
    fn openssl_vectors() {
        let vectors = [
            (Hash::Sha256, "d0506a40f9d65309fa7715d9c0cb9f605b60e3008273701f6fc46fd73add102e", "0A0E682AAE53C086DE04202E1842F243C86C388FF195B4E749E8859EC7BC2B69"),
            (Hash::Sha384, "8d9102fcf173ac08138846625188201192f4cd6a60d8fde2735996abea02bbd4136305c021e8bb2af9ac63a28dfa2959",
             "BAB70B44FAE704E70E302F367D8967791D341B3EE21DD6AE6C6A1B1FF385002B"),
        ];
        for (hash, secret, expected) in vectors {
            let mut out = [0u8; 32];
            export(hash, &hex(secret), LABEL, b"", &mut out).unwrap();
            assert_eq!(out.to_vec(), hex(expected), "{:?}", hash);
        }
    }

    #[test]
    fn context_and_length_change_the_bytes() {
        let secret = hex("d0506a40f9d65309fa7715d9c0cb9f605b60e3008273701f6fc46fd73add102e");
        let (mut empty, mut c2s, mut s2c, mut long) = ([0u8; 32], [0u8; 32], [0u8; 32], [0u8; 64]);
        export(Hash::Sha256, &secret, LABEL, b"", &mut empty).unwrap();
        export(Hash::Sha256, &secret, LABEL, &[0, 0, 0, 15, 0], &mut c2s).unwrap();
        export(Hash::Sha256, &secret, LABEL, &[0, 0, 0, 15, 1], &mut s2c).unwrap();
        export(Hash::Sha256, &secret, LABEL, b"", &mut long).unwrap();
        assert!(empty != c2s && c2s != s2c && empty != s2c);
        // The length is in HKDF's info: 64 bytes do not begin with the 32.
        assert_ne!(long[..32], empty);
    }

    #[test]
    fn refused_lengths_and_labels() {
        let secret = [7u8; 32];
        assert!(export(Hash::Sha256, &secret, LABEL, b"", &mut []).is_none());
        assert!(export(Hash::Sha256, &secret, LABEL, b"", &mut [0u8; LENGTH_MAX + 1]).is_none());
        assert!(export(Hash::Sha256, &secret, &[b'x'; 250], b"", &mut [0u8; 16]).is_none());
        assert!(export(Hash::Sha256, &secret, &[b'x'; 249], b"", &mut [0u8; 16]).is_some());
    }

    #[test]
    fn the_key_log_keeps_the_exporter_secret_alone() {
        let secret = Secret::new();
        assert_eq!(secret.get(), None);
        for label in ["CLIENT_HANDSHAKE_TRAFFIC_SECRET", "SERVER_HANDSHAKE_TRAFFIC_SECRET", "CLIENT_TRAFFIC_SECRET_0", "SERVER_TRAFFIC_SECRET_0"] {
            secret.log(label, &[1; 32]);
        }
        assert_eq!(secret.get(), None);
        secret.log("EXPORTER_SECRET", &[9; 49]);
        assert_eq!(secret.get(), None);
        secret.log("EXPORTER_SECRET", &[9; 48]);
        let copy = secret.get();
        assert_eq!(copy, Some(vec![9; 48]));
        forget(copy.unwrap());
    }
}
