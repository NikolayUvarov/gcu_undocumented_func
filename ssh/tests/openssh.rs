//! Host tests of the SSH client (351-NET-0026) against OpenSSH's server: sshd is started on 127.0.0.1 for the test,
//! with its own host key, configuration and authorized key in a temporary directory, and stopped after it. Needs
//! OpenSSH's server (`openssh-server`, /usr/sbin/sshd) and ssh-keygen.
use ssh::{Error, Io, Keys};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::Duration;

const SSHD: &str = "/usr/sbin/sshd";
const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64(data: &[u8]) -> String {
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, &b)| n | (b as u32) << (16 - 8 * i));
        for i in 0..4 { out.push(if i <= chunk.len() { B64[(n >> (18 - 6 * i)) as usize & 63] as char } else { '=' }); }
    }
    out
}

fn unbase64(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let (mut n, mut bits) = (0u32, 0);
    for c in text.bytes().take_while(|&c| c != b'=') {
        n = n << 6 | B64.iter().position(|&b| b == c).expect("base64") as u32;
        bits += 6;
        if bits >= 8 { bits -= 8; out.push((n >> bits) as u8); }
    }
    out
}

fn sha256(data: &[u8]) -> [u8; 32] {
    use sha2::Digest;
    sha2::Sha256::digest(data).into()
}

fn random(out: &mut [u8]) -> bool { std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(out)).is_ok() }

struct Tcp(TcpStream);
impl Io for Tcp {
    fn send(&mut self, data: &[u8]) -> Result<(), Error> { self.0.write_all(data).map_err(|_| Error::Io) }
    fn receive(&mut self, buffer: &mut [u8]) -> Result<usize, Error> { self.0.read(buffer).map_err(|_| Error::Io) }
}

struct Key(ed25519_dalek::SigningKey, usize);
impl Keys for Key {
    fn public(&self) -> [u8; 32] { self.0.verifying_key().to_bytes() }
    fn sign(&mut self, data: &[u8]) -> Option<[u8; 64]> {
        use ed25519_dalek::Signer;
        // The data OpenSSH's server takes is what the key service checks before it signs (351-NET-0027).
        assert!(ssh::login::fits(data, &self.public()), "a login's data that the key service would refuse");
        self.1 += 1;
        Some(self.0.sign(data).to_bytes())
    }
}

fn blob(public: &[u8; 32]) -> Vec<u8> {
    let mut b = Vec::new();
    for part in [&b"ssh-ed25519"[..], &public[..]] { b.extend_from_slice(&(part.len() as u32).to_be_bytes()); b.extend_from_slice(part); }
    b
}

/// sshd for one test: its directory, port and host key pin; stopped when dropped.
struct Server { child: Child, dir: PathBuf, port: u16, pin: [u8; 32] }

impl Drop for Server {
    fn drop(&mut self) { let _ = self.child.kill(); let _ = self.child.wait(); let _ = std::fs::remove_dir_all(&self.dir); }
}

fn server(name: &str, client: &[u8; 32]) -> Server {
    assert!(Path::new(SSHD).exists(), "OpenSSH's server is needed for these tests: install openssh-server ({} is missing)", SSHD);
    let dir = std::env::temp_dir().join(format!("mind-ssh-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let host = dir.join("host_key");
    let made = Command::new("ssh-keygen").args(["-q", "-t", "ed25519", "-N", "", "-f"]).arg(&host).status().expect("ssh-keygen");
    assert!(made.success());
    let public = std::fs::read_to_string(dir.join("host_key.pub")).unwrap();
    let pin = sha256(&unbase64(public.split_whitespace().nth(1).unwrap()));
    std::fs::write(dir.join("authorized_keys"), format!("ssh-ed25519 {} mind-test\n", base64(&blob(client)))).unwrap();
    let mut pattern: Vec<u8> = (0..100_000u32).map(|i| (i * 7 + i / 251) as u8).collect();
    pattern.extend_from_slice(b"the end\n");
    std::fs::write(dir.join("release.bin"), &pattern).unwrap();
    let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let config = dir.join("sshd_config");
    std::fs::write(&config, format!(
        "Port {port}\nListenAddress 127.0.0.1\nHostKey {}\nPidFile {}\nAuthorizedKeysFile {}\nPasswordAuthentication no\n\
         KbdInteractiveAuthentication no\nPubkeyAuthentication yes\nUsePAM no\nStrictModes no\nSubsystem sftp internal-sftp\nLogLevel ERROR\n",
        host.display(), dir.join("sshd.pid").display(), dir.join("authorized_keys").display())).unwrap();
    // Run as root, sshd wants its privilege separation directory.
    if std::env::var("USER").as_deref() == Ok("root") || unsafe_is_root() { let _ = std::fs::create_dir_all("/run/sshd"); }
    let child = Command::new(SSHD).args(["-D", "-e", "-f"]).arg(&config).spawn().expect("sshd");
    for _ in 0..100 {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() { break; }
        std::thread::sleep(Duration::from_millis(50));
    }
    Server { child, dir, port, pin }
}

fn unsafe_is_root() -> bool { std::fs::metadata("/proc/self").map(|m| { use std::os::unix::fs::MetadataExt; m.uid() == 0 }).unwrap_or(false) }

fn user() -> String {
    let out = Command::new("id").arg("-un").output().expect("id");
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

fn tcp(port: u16) -> Tcp {
    let stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    Tcp(stream)
}

#[test]
fn reads_a_file_over_sftp() {
    let mut key = Key(ed25519_dalek::SigningKey::from_bytes(&[7; 32]), 0);
    let s = server("read", &key.public());
    let mut session = ssh::connect(tcp(s.port), &user(), &s.pin, &mut key, random).expect("login");
    assert_eq!(session.host_key(), s.pin);
    assert_eq!(key.1, 1, "one signature for one login");
    let path = s.dir.join("release.bin");
    let expected = std::fs::read(&path).unwrap();
    let file = session.open(path.to_str().unwrap()).expect("open");
    assert_eq!(session.size(&file).unwrap(), Some(expected.len() as u64));
    let mut got = Vec::new();
    let mut buffer = vec![0u8; 32 * 1024];
    loop {
        let n = session.read(&file, got.len() as u64, &mut buffer).expect("read");
        if n == 0 { break; }
        got.extend_from_slice(&buffer[..n]);
    }
    assert_eq!(got, expected);
    // A read from the middle, and one past the end.
    let n = session.read(&file, 99_990, &mut buffer[..100]).unwrap();
    assert_eq!(&buffer[..n], &expected[99_990..]);
    assert_eq!(session.read(&file, expected.len() as u64 + 10, &mut buffer).unwrap(), 0);
    session.close(file).unwrap();
    // A missing file: SFTP's "no such file".
    assert_eq!(session.open(s.dir.join("missing").to_str().unwrap()).err(), Some(Error::Status(2)));
}

#[test]
fn refuses_another_host_key_and_another_login_key() {
    let mut key = Key(ed25519_dalek::SigningKey::from_bytes(&[7; 32]), 0);
    let s = server("refuse", &key.public());
    let mut wrong = s.pin;
    wrong[0] ^= 1;
    assert_eq!(ssh::connect(tcp(s.port), &user(), &wrong, &mut key, random).err(), Some(Error::HostKey));
    assert_eq!(key.1, 0, "nothing signed for a server that is not the pinned one");
    let mut stranger = Key(ed25519_dalek::SigningKey::from_bytes(&[9; 32]), 0);
    assert_eq!(ssh::connect(tcp(s.port), &user(), &s.pin, &mut stranger, random).err(), Some(Error::Refused));
}

struct Silent;
impl Keys for Silent {
    fn public(&self) -> [u8; 32] { [0; 32] }
    fn sign(&mut self, _: &[u8]) -> Option<[u8; 64]> { None }
}

#[test]
fn a_signer_that_gives_nothing_ends_the_login() {
    let key = Key(ed25519_dalek::SigningKey::from_bytes(&[7; 32]), 0);
    let s = server("nokey", &key.public());
    assert_eq!(ssh::connect(tcp(s.port), &user(), &s.pin, &mut Silent, random).err(), Some(Error::NoKey));
}
