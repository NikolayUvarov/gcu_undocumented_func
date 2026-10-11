// The TPM 2.0's registers from the ACPI TPM2 table (351-KRN-0052, facts of the TCG ACPI specification): the address
// of a CRB's control area at 40, the start method at 48. No allocation and no system state: tests/aml_host.rs.

/// The page of locality 0's registers the TPM2 table names: a CRB's control area page (start method 7, or 8 with an
/// ACPI start), a FIFO's address (6) or, with `pc`, the PC Client profile's fixed FIFO page. None for start methods the
/// TPM service does not drive, or a FIFO elsewhere without an address here (aarch64 names it in the DSDT).
pub fn registers(table: &[u8], pc: bool) -> Option<u64> {
    if table.len() < 52 || &table[..4] != b"TPM2" { return None; }
    let control = u64::from_le_bytes(table[40..48].try_into().ok()?);
    let method = u32::from_le_bytes(table[48..52].try_into().ok()?);
    match method {
        6..=8 if control != 0 => Some(control & !0xFFF),
        6 if pc => Some(0xFED4_0000),
        _ => None,
    }
}
