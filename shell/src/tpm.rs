//! `tpm [seal <text>]` (351-APP-0018): the TPM the TPM service drives (idl/tpm.wit), through the shell's client, which has
//! no seal badge. `tpm seal` asks to seal anyway, as a check of the service's rights: it is refused and logged.
use core::fmt::Write;
use mind::abi::SLOT_TPM;
use mind::idl::tpm;
use mind::ipc::Endpoint;

const TPM: Endpoint = Endpoint(SLOT_TPM);

pub fn command(out: &mut impl Write, args: &[u8]) {
    let text = core::str::from_utf8(args).unwrap_or("").trim();
    if let Some(secret) = text.strip_prefix("seal ") {
        let mut blob = [0u8; 1024];
        match tpm::seal(TPM, secret.as_bytes(), &mut blob) {
            Ok(Ok(n)) => { let _ = writeln!(out, "TPM: SEALED, {} BYTES", n); }
            Ok(Err(error)) => { let _ = writeln!(out, "TPM: {:?}", error); }
            Err(_) => { let _ = writeln!(out, "TPM: NO TPM SERVICE"); }
        }
        return;
    }
    if !text.is_empty() { let _ = writeln!(out, "TPM [SEAL <TEXT>]"); return; }
    match tpm::info(TPM) {
        Ok(Ok(info)) => {
            let name = info.manufacturer.to_be_bytes();
            let _ = writeln!(out, "TPM 2.0 BY {}, {} INTERFACE", core::str::from_utf8(&name).unwrap_or("?").trim_matches(|c: char| c == '\0' || c == ' '), if info.crb { "CRB" } else { "FIFO" });
        }
        Ok(Err(tpm::Error::NoTpm)) => { let _ = writeln!(out, "TPM: NONE"); }
        Ok(Err(error)) => { let _ = writeln!(out, "TPM: {:?}", error); }
        Err(_) => { let _ = writeln!(out, "TPM: NO TPM SERVICE"); }
    }
}
