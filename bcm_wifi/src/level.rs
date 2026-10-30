// The levels a frame's receive header gives, corrected for the external LNA held off (550-DRV-0045). Plain logic, no
// hardware, so a host test drives it.

// What a level read with the LNA held off lacks: the PHY still counts the LNA's gain. The network's beacons read 18 dB
// lower at once when it was held and 18 dB higher at once when it was given back, at both switches of 550-DRV-0039's
// comparison (the MacBook Pro's BCM4331, 2.4 GHz, 2026-10-29). Measured on that machine only.
pub const LNA_GAIN: i8 = 18;

// A core's power byte in dBm with `gain` added. 0 and -128 are no level and stay as they are; a level stays below 0.
pub fn level(raw: u8, gain: i8) -> i8 {
    let v = raw as i8;
    if v < 0 && v > -128 { (v as i16 + gain as i16).clamp(-127, -1) as i8 } else { v }
}

// The three cores' levels of a receive header's power bytes.
pub fn levels(raw: [u8; 3], gain: i8) -> [i8; 3] { raw.map(|p| level(p, gain)) }
