// 213-DRV-0087: PPDU status reports captured on the Ryzen, whole RX buffers as the bench print showed them
// (2026-11-21 16:29-16:33 UTC): the PCIe word, the descriptor, the report.
#![allow(dead_code)]

// CoolNet2's 5 GHz beacon on channel 100 (A0:36:BC:11:F8:8C, OFDM 6 Mb/s): an OFDM_BRK report, paths A 91 and B 95.
pub const FIVE: &str = "54c0cb0440008001120004000000000001002000010060111f0001001800000000000000020000000b2c00008305005d5b5f00000158303000bacffb18c05e080000000000000000007fc05f0000000000000000";
// Its 2.4 GHz beacon on channel 8 heard from 7 (CCK 1 Mb/s): a CCK report, RPL 90 above the average 88.
pub const CCK: &str = "6cc0b400580080014000000000000000010020000100001000000200180000000000000000000000a0f30a04b00bacc7000000008b07005858580000805a210000df6ffe5a5db00b20410030011e0730000d800400c01f0800000000000000000017c05f0000000000000000";
// The same beacon earlier, an OFDM_BRK report at CCK: paths 92 and 89.
pub const BREAK: &str = "5cc06100480080013000000000000000010020000100001000000200180000000000000000000000a0f30a04b00bacc7000000008305005a5c5900000100073000fb144e00c00b080000000000000000001bc05f0000000000000000";
// A data frame from the access point to this radio (OFDM, rate 9): a legacy OFDM report with the frequency-domain RPL 97.
pub const DATA: &str = "54c0c71240008001320009000000000001002000010066111f000100030000000000000002000000cd2c00008c05006a686c00000161083000e0bffd16c05d0872feffff0000000000ff284c0000000000000000";
// Another station's CCK frame: RPL 69 above the average 63.
pub const OTHER: &str = "6cc0c812580080010000000000000000000020000100000000000200180000000000000000000000a0f30a04e006becf000000008b07003f413c000080459f020031f0035445e006206300300124083000ea8eec0000000872feffff00000000007fc05f0000000000000000";

pub fn bytes(hex: &str) -> Vec<u8> { (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap()).collect() }

/// A Wi-Fi frame's whole RX buffer (FCS included in `mpdu`): a short descriptor whose word 1 carries the PPDU `count`
/// and the data `rate`, which its report repeats.
pub fn frame(mpdu: &[u8], count: u8, rate: u16) -> Vec<u8> {
    let length = 20 + mpdu.len(); let mut b = vec![0; length];
    b[..4].copy_from_slice(&(length as u32 | 0xc000).to_le_bytes());
    b[4..8].copy_from_slice(&(mpdu.len() as u32).to_le_bytes());
    b[8..12].copy_from_slice(&(u32::from(count) << 4 | u32::from(rate) << 16).to_le_bytes());
    b[20..].copy_from_slice(mpdu); b
}
