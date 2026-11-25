//! A host client of `hub-api 3`: signs a call over the last challenge the hub gave, as a device, an agent or an admin
//! does, and speaks TLS 1.3 to a hub known by its key's SPKI SHA-256 (4.1), or by a chain to a CA bundle's roots,
//! directly or through an HTTP proxy's CONNECT (100003-APP-0160). The tests, the conformance suite and
//! `mindhub admin-code` use it; `improve` on a host (stage 5) is built on it.

use ed25519_dalek::SigningKey;
use improve_proto::json::{self, Value};
use improve_proto::sign;
use pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::{ClientConfig, DigitallySignedStruct, RootCertStore, SignatureScheme};
use std::io::{self, Read, Write};
use std::net::{IpAddr, TcpStream};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Reply { pub status: u16, pub headers: Vec<(String, String)>, pub body: Vec<u8> }

impl Reply {
    pub fn header(&self, name: &str) -> Option<&str> { self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str()) }
    pub fn text(&self) -> String { String::from_utf8_lossy(&self.body).into_owned() }
    /// The body as JSON; a body that is not is `Null`.
    pub fn json(&self) -> Value { std::str::from_utf8(&self.body).ok().and_then(|t| json::parse(t).ok()).unwrap_or(Value::Null) }
    /// The refusal's limit, when the answer is one.
    pub fn error(&self) -> Option<String> { self.json().get("error").and_then(Value::as_str).map(String::from) }
}

/// The hub's TLS key, pinned: a certificate whose SPKI SHA-256 is one of the pins is the hub, whoever signed it.
#[derive(Debug)]
struct Pinned { pins: Vec<[u8; 32]> }

const SCHEMES: [SignatureScheme; 2] = [SignatureScheme::ED25519, SignatureScheme::ECDSA_NISTP256_SHA256];

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(&self, end_entity: &CertificateDer<'_>, _: &[CertificateDer<'_>], _: &ServerName<'_>, _: &[u8], _: UnixTime) -> Result<ServerCertVerified, rustls::Error> {
        let spki = httpd::tls::spki_sha256(end_entity).ok_or_else(|| rustls::Error::General("the hub's certificate cannot be read".into()))?;
        if self.pins.contains(&spki) { Ok(ServerCertVerified::assertion()) }
        else { Err(rustls::Error::General(format!("the hub's key {} is not pinned", httpd::tls::hex(&spki)))) }
    }
    fn verify_tls12_signature(&self, _: &[u8], _: &CertificateDer<'_>, _: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        Err(rustls::Error::General("TLS 1.3 only".into()))
    }
    fn verify_tls13_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        let bad = || rustls::Error::General("the hub's handshake signature does not verify".into());
        let ee = webpki::EndEntityCert::try_from(cert).map_err(|_| bad())?;
        let spki = ee.subject_public_key_info();
        let spki = spki.as_ref();
        match dss.scheme {
            SignatureScheme::ED25519 => {
                // The SubjectPublicKeyInfo of an Ed25519 key: its algorithm, then the 32 bytes of the key.
                const PREFIX: [u8; 12] = [0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00];
                let inner = spki.strip_prefix(&PREFIX[2..]).or_else(|| spki.strip_prefix(&PREFIX[..])).ok_or_else(bad)?;
                let key: [u8; 32] = inner.try_into().map_err(|_| bad())?;
                let key = ed25519_dalek::VerifyingKey::from_bytes(&key).map_err(|_| bad())?;
                let sig = ed25519_dalek::Signature::from_slice(dss.signature()).map_err(|_| bad())?;
                key.verify_strict(message, &sig).map_err(|_| bad())?;
            }
            SignatureScheme::ECDSA_NISTP256_SHA256 => {
                use p256::ecdsa::signature::Verifier;
                use p256::pkcs8::DecodePublicKey;
                let mut der = spki.to_vec();
                if der.first() != Some(&0x30) || der.len() < 2 || der[1] as usize + 2 != der.len() {
                    // webpki gives the SPKI's contents; the DER wraps them in their SEQUENCE.
                    let mut wrapped = vec![0x30, der.len() as u8];
                    wrapped.extend_from_slice(&der);
                    der = wrapped;
                }
                let key = p256::ecdsa::VerifyingKey::from_public_key_der(&der).map_err(|_| bad())?;
                let sig = p256::ecdsa::Signature::from_der(dss.signature()).map_err(|_| bad())?;
                key.verify(message, &sig).map_err(|_| bad())?;
            }
            _ => return Err(rustls::Error::General("an Ed25519 or P-256 key only".into())),
        }
        Ok(HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> { SCHEMES.to_vec() }
}

enum Transport { Plain, Tls(Arc<ClientConfig>, ServerName<'static>) }

/// How a client knows the hub over TLS: by its key's SPKI SHA-256, as a device does; or by a chain to one of these
/// roots, its name the URL's host, as an agent does behind a proxy that ends TLS with a CA of its own (100003-APP-0160).
/// Either way every call is signed over the hub's challenge, so whoever ends TLS on the way cannot forge one.
#[derive(Clone)]
pub enum Trust { Pins(Vec<[u8; 32]>), Roots(Arc<RootCertStore>) }

/// The roots of a PEM bundle; one that holds no certificate it can use is refused.
pub fn roots(path: &Path) -> Result<Arc<RootCertStore>, String> {
    use pki_types::pem::PemObject;
    let certs: Vec<CertificateDer<'static>> = CertificateDer::pem_file_iter(path).map_err(|e| format!("{}: {e}", path.display()))?
        .collect::<Result<_, _>>().map_err(|e| format!("{}: {e}", path.display()))?;
    let mut store = RootCertStore::empty();
    let (added, _) = store.add_parsable_certificates(certs);
    if added == 0 { return Err(format!("{}: no certificate of a CA", path.display())); }
    Ok(Arc::new(store))
}

/// The proxy `https://HOST` is reached through: `HTTPS_PROXY` (or `https_proxy`), as `host:port`, unless `NO_PROXY`
/// names the host or the host is a loopback one; `Err` for a proxy this client cannot use.
pub fn proxy_from_env(host: &str) -> Result<Option<String>, String> {
    let var = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
    proxy_for(host, var("HTTPS_PROXY").or_else(|| var("https_proxy")).as_deref(), var("NO_PROXY").or_else(|| var("no_proxy")).as_deref())
}

/// `proxy_from_env` with the variables' values given.
pub fn proxy_for(host: &str, https_proxy: Option<&str>, no_proxy: Option<&str>) -> Result<Option<String>, String> {
    let Some(url) = https_proxy.filter(|u| !u.is_empty()) else { return Ok(None) };
    if loopback(host) || no_proxy.unwrap_or("").split(',').map(str::trim).any(|e| excepted(host, e)) { return Ok(None); }
    proxy(url).map(Some)
}

/// A proxy's URL as `host:port`: `http://HOST:PORT` or `HOST:PORT`, with no user and no TLS to the proxy itself.
pub fn proxy(url: &str) -> Result<String, String> {
    if url.starts_with("https://") { return Err("a proxy spoken to over TLS is not supported: http://HOST:PORT".into()); }
    let rest = url.strip_prefix("http://").unwrap_or(url).trim_end_matches('/');
    if rest.contains('@') { return Err("a proxy with a user and a password is not supported".into()); }
    if rest.rsplit_once(':').is_none_or(|(h, p)| h.is_empty() || p.parse::<u16>().is_err()) { return Err(format!("the proxy {rest}: HOST:PORT")); }
    Ok(rest.to_string())
}

// `host:port`, the scheme's port added when the URL names none (`hub.example`, `[::1]`).
fn with_port(addr: &str, port: u16) -> String {
    let named = match addr.rfind(']') { Some(end) => addr[end..].contains(':'), None => addr.matches(':').count() == 1 };
    if named { addr.to_string() } else { format!("{addr}:{port}") }
}

fn loopback(host: &str) -> bool { host == "localhost" || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback()) }

// Whether a NO_PROXY entry names the host: `*`, the host, a domain it is in, or an IPv4 network that holds it.
fn excepted(host: &str, entry: &str) -> bool {
    let entry = entry.trim_start_matches('.');
    if entry.is_empty() { return false; }
    if entry == "*" || host == entry || host.ends_with(&format!(".{entry}")) { return true; }
    let (Some((net, bits)), Ok(std::net::IpAddr::V4(ip))) = (entry.split_once('/'), host.parse::<IpAddr>()) else { return false };
    match (net.parse::<std::net::Ipv4Addr>(), bits.parse::<u32>()) {
        (Ok(net), Ok(bits)) if bits <= 32 => { let mask = if bits == 0 { 0 } else { u32::MAX << (32 - bits) }; u32::from(ip) & mask == u32::from(net) & mask }
        _ => false,
    }
}

// A TCP connection to `addr` through an HTTP proxy's CONNECT; its head read to its end and no further, so that TLS
// starts at the next byte; a refusal named with the proxy's status line.
fn tunnel(proxy: &str, addr: &str, timeout: Duration) -> io::Result<TcpStream> {
    let mut s = TcpStream::connect(proxy)?;
    s.set_read_timeout(Some(timeout))?;
    s.set_write_timeout(Some(timeout))?;
    s.write_all(format!("CONNECT {addr} HTTP/1.1\r\nHost: {addr}\r\n\r\n").as_bytes())?;
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if head.len() > 16 * 1024 { return Err(io::Error::other(format!("the proxy {proxy} answered CONNECT with a head over 16 KiB"))); }
        if s.read(&mut byte)? == 0 { return Err(io::Error::other(format!("the proxy {proxy} closed the connection to CONNECT {addr}"))); }
        head.push(byte[0]);
    }
    let text = String::from_utf8_lossy(&head);
    let line = text.lines().next().unwrap_or("");
    if !line.split(' ').nth(1).is_some_and(|status| status.starts_with('2')) {
        return Err(io::Error::other(format!("the proxy {proxy} refused CONNECT {addr}: {line}")));
    }
    Ok(s)
}

/// A client of one hub, with the key it signs with.
pub struct Client { addr: String, transport: Transport, proxy: Option<String>, key: SigningKey, challenge: Option<[u8; 32]>, pub timeout: Duration }

impl Client {
    /// A client of `http://HOST:PORT` (a hub on loopback) or `https://HOST:PORT`, known by one of `pins`, reached directly.
    pub fn new(url: &str, pins: &[[u8; 32]], key: SigningKey) -> Result<Client, String> { Client::connect(url, Trust::Pins(pins.to_vec()), None, key) }

    /// A client of `url` known by `trust`, reached through `proxy` (`host:port`, CONNECT) when one is given.
    pub fn connect(url: &str, trust: Trust, proxy: Option<String>, key: SigningKey) -> Result<Client, String> {
        let (tls, rest) = if let Some(r) = url.strip_prefix("https://") { (true, r) } else if let Some(r) = url.strip_prefix("http://") { (false, r) }
            else { return Err(format!("{url}: http:// or https://")) };
        let addr = with_port(rest.trim_end_matches('/'), if tls { 443 } else { 80 });
        let host = addr.rsplit_once(':').map(|(h, _)| h).unwrap_or(&addr).trim_start_matches('[').trim_end_matches(']').to_string();
        let transport = if tls {
            let builder = |provider| ClientConfig::builder_with_provider(provider).with_protocol_versions(&[&rustls::version::TLS13]).map_err(|e| e.to_string());
            let mut config = match trust {
                Trust::Pins(pins) if pins.is_empty() => return Err("a hub over TLS is known by its key's SPKI SHA-256 (--pin) or by a CA bundle (--ca)".into()),
                Trust::Pins(pins) => builder(httpd::tls::provider(false))?.dangerous().with_custom_certificate_verifier(Arc::new(Pinned { pins })).with_no_client_auth(),
                Trust::Roots(roots) => builder(crate::verify::provider())?.with_root_certificates(roots).with_no_client_auth(),
            };
            config.alpn_protocols = vec![b"http/1.1".to_vec()];
            let name = ServerName::try_from(host).map_err(|e| format!("{url}: {e}"))?;
            Transport::Tls(Arc::new(config), name)
        } else {
            if proxy.is_some() { return Err(format!("{url}: a hub reached through a proxy is reached over TLS, https://")); }
            Transport::Plain
        };
        Ok(Client { addr, transport, proxy, key, challenge: None, timeout: Duration::from_secs(30) })
    }

    pub fn key(&self) -> &SigningKey { &self.key }
    pub fn public_key(&self) -> [u8; 32] { self.key.verifying_key().to_bytes() }
    pub fn challenge(&self) -> Option<[u8; 32]> { self.challenge }

    fn exchange(&mut self, method: &str, target: &str, headers: &[(&str, String)], body: &[u8]) -> io::Result<Reply> {
        let tcp = match &self.proxy { Some(proxy) => tunnel(proxy, &self.addr, self.timeout)?, None => TcpStream::connect(&self.addr)? };
        tcp.set_read_timeout(Some(self.timeout))?;
        tcp.set_write_timeout(Some(self.timeout))?;
        let mut head = format!("{method} {target} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nContent-Length: {}\r\n", self.addr, body.len());
        for (k, v) in headers { head.push_str(&format!("{k}: {v}\r\n")); }
        head.push_str("\r\n");
        let mut raw = Vec::new();
        match &self.transport {
            Transport::Plain => {
                let mut s = tcp;
                s.write_all(head.as_bytes())?;
                s.write_all(body)?;
                s.read_to_end(&mut raw)?;
            }
            Transport::Tls(config, name) => {
                let conn = rustls::ClientConnection::new(config.clone(), name.clone()).map_err(io::Error::other)?;
                let mut s = rustls::StreamOwned::new(conn, tcp);
                s.write_all(head.as_bytes())?;
                s.write_all(body)?;
                s.flush()?;
                match s.read_to_end(&mut raw) {
                    Ok(_) => {}
                    // A server that closes without close_notify after a whole answer; a proxy that resets the
                    // connection after one (the cloud's does, 100003-APP-0160).
                    Err(e) if e.kind() == io::ErrorKind::UnexpectedEof && !raw.is_empty() => {}
                    Err(e) if e.kind() == io::ErrorKind::ConnectionReset && whole(&raw) => {}
                    Err(e) => return Err(e),
                }
            }
        }
        let reply = parse(&raw).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "not an HTTP answer"))?;
        if let Some(c) = reply.header("Hub-Challenge").and_then(improve_proto::parse_hex32) { self.challenge = Some(c); }
        Ok(reply)
    }

    /// An unsigned call: the board, a challenge, a page.
    pub fn plain(&mut self, method: &str, target: &str, headers: &[(&str, String)], body: &[u8]) -> io::Result<Reply> { self.exchange(method, target, headers, body) }

    /// A call signed over the last challenge, which the hub gave in its last answer; one stale challenge is taken again.
    pub fn call(&mut self, method: &str, target: &str, body: &[u8]) -> io::Result<Reply> {
        for attempt in 0..2 {
            if self.challenge.is_none() { self.exchange("GET", "/hub/v1/challenge", &[], &[])?; }
            let challenge = self.challenge.take().ok_or_else(|| io::Error::other("the hub gave no challenge"))?;
            let header = sign::header(&self.key, method, target, body, &challenge).map_err(|e| io::Error::other(format!("{e:?}")))?;
            let reply = self.exchange(method, target, &[(sign::HEADER, header)], body)?;
            if reply.status == 401 && reply.error().as_deref() == Some("challenge") && attempt == 0 { continue; }
            return Ok(reply);
        }
        unreachable!()
    }
}

// Whether `raw` holds an answer's head and all of the body its Content-Length names.
fn whole(raw: &[u8]) -> bool {
    let Some(end) = raw.windows(4).position(|w| w == b"\r\n\r\n") else { return false };
    let head = String::from_utf8_lossy(&raw[..end]);
    head.lines().find_map(|l| l.split_once(':').filter(|(k, _)| k.trim().eq_ignore_ascii_case("content-length")).and_then(|(_, v)| v.trim().parse::<usize>().ok()))
        .is_some_and(|n| raw.len() - end - 4 >= n)
}

fn parse(raw: &[u8]) -> Option<Reply> {
    let end = raw.windows(4).position(|w| w == b"\r\n\r\n")?;
    let head = std::str::from_utf8(&raw[..end]).ok()?;
    let mut lines = head.split("\r\n");
    let status = lines.next()?.split(' ').nth(1)?.parse().ok()?;
    let headers: Vec<(String, String)> = lines.filter_map(|l| l.split_once(':')).map(|(k, v)| (k.to_string(), v.trim().to_string())).collect();
    let body = raw[end + 4..].to_vec();
    let length = headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("content-length")).and_then(|(_, v)| v.parse::<usize>().ok());
    let body = match length { Some(n) if n <= body.len() => body[..n].to_vec(), _ => body };
    Some(Reply { status, headers, body })
}

/// A key file: the Ed25519 secret's 32 bytes in hex on one line, readable by its owner alone.
pub fn read_key(path: &std::path::Path) -> Result<SigningKey, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let seed = improve_proto::parse_hex32(text.trim()).ok_or_else(|| format!("{}: not 32 bytes in lower-case hex", path.display()))?;
    Ok(SigningKey::from_bytes(&seed))
}

pub fn write_key(path: &std::path::Path) -> Result<SigningKey, String> {
    use std::os::unix::fs::OpenOptionsExt;
    let key = SigningKey::from_bytes(&crate::util::random::<32>());
    let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    writeln!(f, "{}", improve_proto::hex(key.as_bytes())).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(key)
}
