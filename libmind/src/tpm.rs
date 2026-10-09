//! TPM 2.0 commands for the TPM service (351-DRV-0015): what sealing a secret to the TPM takes, marshalled by hand
//! (TCG TPM 2.0 Library, Part 2 and 3). A sealed object is a keyed-hash object holding the secret, under a primary
//! storage key the TPM derives again from its owner seed on every boot (ECC P-256, the same template), so nothing but
//! the sealed blob need be kept. Authorization is the empty password of the owner hierarchy and of the objects; a policy
//! on the boot state (PCRs) is a later step. No system calls: tests/tpm_host.rs builds it.

/// A client with this badge may seal and unseal (the key service's); any client may ask what the TPM is.
pub const BADGE_SEAL: u16 = 1;

/// The largest command or response handled.
pub const BUFFER: usize = 2048;
/// The largest secret sealed (TPM2B_SENSITIVE_DATA holds 128 bytes).
pub const SECRET_MAX: usize = 128;

pub const RC_SUCCESS: u32 = 0;
/// TPM2_Startup after the firmware already started the TPM.
pub const RC_INITIALIZE: u32 = 0x100;

const ST_NO_SESSIONS: u16 = 0x8001;
const ST_SESSIONS: u16 = 0x8002;
const CC_CREATE_PRIMARY: u32 = 0x131;
const CC_STARTUP: u32 = 0x144;
const CC_CREATE: u32 = 0x153;
const CC_LOAD: u32 = 0x157;
const CC_UNSEAL: u32 = 0x15E;
const CC_FLUSH_CONTEXT: u32 = 0x165;
const CC_GET_CAPABILITY: u32 = 0x17A;
const RH_OWNER: u32 = 0x4000_0001;
const RS_PW: u32 = 0x4000_0009;
const ALG_AES: u16 = 0x0006;
const ALG_KEYEDHASH: u16 = 0x0008;
const ALG_SHA256: u16 = 0x000B;
const ALG_NULL: u16 = 0x0010;
const ALG_ECC: u16 = 0x0023;
const ALG_CFB: u16 = 0x0043;
const ECC_NIST_P256: u16 = 0x0003;
const CAP_TPM_PROPERTIES: u32 = 6;
const PT_MANUFACTURER: u32 = 0x105;
// Object attributes: fixedTPM, fixedParent, sensitiveDataOrigin, userWithAuth, noDA, restricted, decrypt.
const FIXED_TPM: u32 = 1 << 1;
const FIXED_PARENT: u32 = 1 << 4;
const SENSITIVE_DATA_ORIGIN: u32 = 1 << 5;
const USER_WITH_AUTH: u32 = 1 << 6;
const NO_DA: u32 = 1 << 10;
const RESTRICTED: u32 = 1 << 16;
const DECRYPT: u32 = 1 << 17;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The command would not fit in `BUFFER`, or a secret is longer than `SECRET_MAX`.
    TooLarge,
    /// A response shorter than it says, or not of the form the command expects.
    Malformed,
    /// The TPM answered with this response code.
    Tpm(u32),
}

/// A command being marshalled: big-endian, its size patched in at the end.
pub struct Command { bytes: [u8; BUFFER], len: usize }

impl Command {
    fn new(tag: u16, code: u32) -> Self {
        let mut command = Command { bytes: [0; BUFFER], len: 0 };
        command.u16(tag); command.u32(0); command.u32(code);
        command
    }
    fn put(&mut self, data: &[u8]) {
        // Commands are built only from bounded parts: overflow marks the command too large.
        let end = self.len.saturating_add(data.len());
        if end > BUFFER { self.len = usize::MAX; return; }
        self.bytes[self.len..end].copy_from_slice(data);
        self.len = end;
    }
    fn u8(&mut self, v: u8) { self.put(&[v]) }
    fn u16(&mut self, v: u16) { self.put(&v.to_be_bytes()) }
    fn u32(&mut self, v: u32) { self.put(&v.to_be_bytes()) }
    fn sized(&mut self, data: &[u8]) { self.u16(data.len() as u16); self.put(data) }
    // A password session with the empty password: TPMS_AUTH_COMMAND in an authorization area.
    fn password(&mut self) { self.u32(9); self.u32(RS_PW); self.u16(0); self.u8(0); self.u16(0); }
    // Writes the size into the header; the command's bytes.
    fn finish(mut self) -> Result<CommandBytes, Error> {
        if self.len > BUFFER { return Err(Error::TooLarge); }
        let size = (self.len as u32).to_be_bytes();
        self.bytes[2..6].copy_from_slice(&size);
        Ok(CommandBytes { bytes: self.bytes, len: self.len })
    }
}

/// A marshalled command.
pub struct CommandBytes { bytes: [u8; BUFFER], len: usize }
impl CommandBytes { pub fn as_bytes(&self) -> &[u8] { &self.bytes[..self.len] } }

/// TPM2_Startup(CLEAR).
pub fn startup() -> Result<CommandBytes, Error> { let mut c = Command::new(ST_NO_SESSIONS, CC_STARTUP); c.u16(0); c.finish() }

/// TPM2_GetCapability for the manufacturer property.
pub fn manufacturer() -> Result<CommandBytes, Error> {
    let mut c = Command::new(ST_NO_SESSIONS, CC_GET_CAPABILITY);
    c.u32(CAP_TPM_PROPERTIES); c.u32(PT_MANUFACTURER); c.u32(1);
    c.finish()
}

// The storage primary key's public template: ECC P-256, AES-128-CFB for its children, restricted decryption.
fn primary_template(c: &mut Command) {
    let mut t = Command { bytes: [0; BUFFER], len: 0 };
    t.u16(ALG_ECC); t.u16(ALG_SHA256);
    t.u32(FIXED_TPM | FIXED_PARENT | SENSITIVE_DATA_ORIGIN | USER_WITH_AUTH | NO_DA | RESTRICTED | DECRYPT);
    t.u16(0); // no policy
    t.u16(ALG_AES); t.u16(128); t.u16(ALG_CFB); // symmetric
    t.u16(ALG_NULL); t.u16(ECC_NIST_P256); t.u16(ALG_NULL); // scheme, curve, KDF
    t.u16(0); t.u16(0); // unique: empty point
    c.sized(&t.bytes[..t.len]);
}

/// TPM2_CreatePrimary under the owner hierarchy: the storage key, the same on every boot of this TPM.
pub fn create_primary() -> Result<CommandBytes, Error> {
    let mut c = Command::new(ST_SESSIONS, CC_CREATE_PRIMARY);
    c.u32(RH_OWNER); c.password();
    c.u16(4); c.u16(0); c.u16(0); // inSensitive: no auth, no data
    primary_template(&mut c);
    c.u16(0); c.u32(0); // outsideInfo, creationPCR
    c.finish()
}

/// TPM2_Create of a sealed data object holding `secret` under `parent`.
pub fn create_sealed(parent: u32, secret: &[u8]) -> Result<CommandBytes, Error> {
    if secret.is_empty() || secret.len() > SECRET_MAX { return Err(Error::TooLarge); }
    let mut c = Command::new(ST_SESSIONS, CC_CREATE);
    c.u32(parent); c.password();
    c.u16(4 + secret.len() as u16); c.u16(0); c.sized(secret); // inSensitive: no auth, the secret
    let mut t = Command { bytes: [0; BUFFER], len: 0 };
    t.u16(ALG_KEYEDHASH); t.u16(ALG_SHA256); t.u32(FIXED_TPM | FIXED_PARENT | USER_WITH_AUTH | NO_DA);
    t.u16(0); t.u16(ALG_NULL); t.u16(0); // no policy, no scheme, unique empty
    c.sized(&t.bytes[..t.len]);
    c.u16(0); c.u32(0);
    c.finish()
}

/// TPM2_Load of a sealed object (its TPM2B_PRIVATE and TPM2B_PUBLIC, sizes included) under `parent`.
pub fn load(parent: u32, private: &[u8], public: &[u8]) -> Result<CommandBytes, Error> {
    let mut c = Command::new(ST_SESSIONS, CC_LOAD);
    c.u32(parent); c.password(); c.put(private); c.put(public);
    c.finish()
}

/// TPM2_Unseal of a loaded object.
pub fn unseal(item: u32) -> Result<CommandBytes, Error> { let mut c = Command::new(ST_SESSIONS, CC_UNSEAL); c.u32(item); c.password(); c.finish() }

/// TPM2_FlushContext of a transient object.
pub fn flush(handle: u32) -> Result<CommandBytes, Error> { let mut c = Command::new(ST_NO_SESSIONS, CC_FLUSH_CONTEXT); c.u32(handle); c.finish() }

/// A response, read from the front.
pub struct Response<'a> { bytes: &'a [u8], at: usize }

impl<'a> Response<'a> {
    /// Checks the header: the size it gives, and a response code of success (or `Error::Tpm`).
    pub fn parse(bytes: &'a [u8]) -> Result<Self, Error> {
        if bytes.len() < 10 { return Err(Error::Malformed); }
        let size = u32::from_be_bytes(bytes[2..6].try_into().unwrap_or([0; 4])) as usize;
        if size < 10 || size > bytes.len() { return Err(Error::Malformed); }
        let code = u32::from_be_bytes(bytes[6..10].try_into().unwrap_or([0; 4]));
        if code != RC_SUCCESS { return Err(Error::Tpm(code)); }
        Ok(Response { bytes: &bytes[..size], at: 10 })
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self.at.checked_add(n).filter(|&e| e <= self.bytes.len()).ok_or(Error::Malformed)?;
        let part = &self.bytes[self.at..end];
        self.at = end;
        Ok(part)
    }
    fn u32(&mut self) -> Result<u32, Error> { Ok(u32::from_be_bytes(self.take(4)?.try_into().map_err(|_| Error::Malformed)?)) }
    fn u16(&mut self) -> Result<u16, Error> { Ok(u16::from_be_bytes(self.take(2)?.try_into().map_err(|_| Error::Malformed)?)) }
    // A TPM2B with its size, as the TPM marshals it.
    fn sized_whole(&mut self) -> Result<&'a [u8], Error> {
        let start = self.at;
        let n = self.u16()? as usize;
        self.take(n)?;
        Ok(&self.bytes[start..self.at])
    }
}

/// The handle TPM2_CreatePrimary or TPM2_Load returned.
pub fn handle(response: &[u8]) -> Result<u32, Error> { Response::parse(response)?.u32() }

/// The manufacturer of a TPM2_GetCapability(TPM_PROPERTIES, MANUFACTURER) response: four ASCII letters as a u32.
pub fn parse_manufacturer(response: &[u8]) -> Result<u32, Error> {
    let mut r = Response::parse(response)?;
    r.take(1)?; // moreData
    if r.u32()? != CAP_TPM_PROPERTIES || r.u32()? < 1 || r.u32()? != PT_MANUFACTURER { return Err(Error::Malformed); }
    r.u32()
}

/// The sealed blob of a TPM2_Create response: its TPM2B_PRIVATE then its TPM2B_PUBLIC, sizes included.
pub fn parse_created(response: &[u8], blob: &mut [u8]) -> Result<usize, Error> {
    let mut r = Response::parse(response)?;
    r.u32()?; // parameterSize
    let private = r.sized_whole()?;
    let public = r.sized_whole()?;
    let len = private.len() + public.len();
    if len > blob.len() { return Err(Error::TooLarge); }
    blob[..private.len()].copy_from_slice(private);
    blob[private.len()..len].copy_from_slice(public);
    Ok(len)
}

/// A sealed blob's two parts, each with its size.
pub fn split_blob(blob: &[u8]) -> Result<(&[u8], &[u8]), Error> {
    let part = |b: &[u8]| -> Result<usize, Error> { let n = 2 + u16::from_be_bytes([*b.first().ok_or(Error::Malformed)?, *b.get(1).ok_or(Error::Malformed)?]) as usize; (n <= b.len()).then_some(n).ok_or(Error::Malformed) };
    let first = part(blob)?;
    let second = part(&blob[first..])?;
    if first + second != blob.len() { return Err(Error::Malformed); }
    Ok((&blob[..first], &blob[first..]))
}

/// The secret of a TPM2_Unseal response.
pub fn parse_unsealed(response: &[u8], secret: &mut [u8]) -> Result<usize, Error> {
    let mut r = Response::parse(response)?;
    r.u32()?; // parameterSize
    let n = r.u16()? as usize;
    let data = r.take(n)?;
    if n > secret.len() { return Err(Error::TooLarge); }
    secret[..n].copy_from_slice(data);
    Ok(n)
}

/// The size a response header gives (for a transport that reads the header first).
pub fn response_size(header: &[u8]) -> Option<usize> {
    (header.len() >= 6).then(|| u32::from_be_bytes([header[2], header[3], header[4], header[5]]) as usize).filter(|&n| (10..=BUFFER).contains(&n))
}
