// The levels' arithmetic (audio.wit 1.2, 000-DRV-0057), without the devices: a level is a setting of an amplifier, as
// HD Audio describes one (`steps` settings, each `step_qdb` quarter dB above the one before, setting `zero` at 0 dB),
// shown to clients in per cent of its range. Where a device has no amplifier the gateway scales the samples.

/// An amplifier's settings: how many, the quarter dB between two, and the one at 0 dB.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Amp { pub steps: u8, pub step_qdb: u8, pub zero: u8 }

/// Scaled samples for the output: as AC'97's 5-bit master, 0 dB at the top and 1.5 dB a step down to -46.5 dB.
pub const SOFT_OUTPUT: Amp = Amp { steps: 32, step_qdb: 6, zero: 31 };
/// Scaled samples for the capture: as AC'97's record gain, 0 dB at the bottom and 1.5 dB a step up to +22.5 dB.
pub const SOFT_CAPTURE: Amp = Amp { steps: 16, step_qdb: 6, zero: 0 };
/// AC'97's microphone boost, in quarter dB (+20 dB, register 0x0E bit 6).
pub const AC97_BOOST_QDB: u16 = 80;

/// The setting nearest to `percent` of an amplifier's range; 0 % the lowest, 100 % the highest.
pub fn setting(percent: u8, amp: Amp) -> u8 {
    let top = amp.steps.saturating_sub(1) as u32;
    ((percent.min(100) as u32 * top + 50) / 100) as u8
}

/// The per cent a setting stands for, the inverse of `setting` for every setting it gives.
pub fn percent(setting: u8, amp: Amp) -> u8 {
    let top = amp.steps.saturating_sub(1) as u32;
    if top == 0 { return 100; }
    ((setting.min(top as u8) as u32 * 100 + top / 2) / top) as u8
}

/// A setting's level in quarter dB: negative below the 0 dB setting.
pub fn qdb(setting: u8, amp: Amp) -> i32 { (setting as i32 - amp.zero as i32) * amp.step_qdb as i32 }

/// An HD Audio amplifier from its capabilities (parameter 0x12 or 0xD): offset in bits 6:0, steps - 1 in 14:8, the
/// step's size - 1 in quarter dB in 22:16, mute in 31. None for a fixed amplifier (no step to choose).
pub fn hda_amp(caps: u32) -> Option<(Amp, bool)> {
    let last = ((caps >> 8) & 0x7F) as u8;
    if last == 0 { return None; }
    let step_qdb = (((caps >> 16) & 0x7F) + 1).min(255) as u8;
    Some((Amp { steps: last + 1, step_qdb, zero: ((caps & 0x7F) as u8).min(last) }, caps >> 31 != 0))
}

/// The Set Amplifier Gain/Mute payload (verb 0x3) for both channels: the output amplifier, or the input amplifier of
/// connection `index`.
pub fn hda_gain(output: bool, index: u8, setting: u8, mute: bool) -> u32 {
    let side = if output { 0xB000 } else { 0x7000 | (index as u32 & 0xF) << 8 };
    side | (mute as u32) << 7 | (setting as u32 & 0x7F)
}

/// AC'97's master volume (register 0x02): attenuation 0 at the top setting, both channels, mute in bit 15.
pub fn ac97_master(setting: u8, amp: Amp, mute: bool) -> u16 {
    let down = (amp.steps.saturating_sub(1).saturating_sub(setting) & 0x3F) as u16;
    (mute as u16) << 15 | down << 8 | down
}

/// AC'97's record gain (register 0x1C): 0 dB at setting 0, both channels, 1.5 dB a step up to +22.5 dB.
pub fn ac97_record(setting: u8) -> u16 { let g = (setting & 0xF) as u16; g << 8 | g }

/// AC'97's microphone (register 0x0E): unmuted, its own gain at 0 dB (0x08), the +20 dB boost in bit 6.
pub fn ac97_microphone(boost: bool) -> u16 { 0x0008 | (boost as u16) << 6 }

/// A factor in Q15 (32768 = 1) for a level of `qdb` quarter dB, from -46.5 to +22.5 dB in 1.5 dB steps (the scaled
/// ranges); 0 for mute. Each 1.5 dB step multiplies by 10^(±1.5/20), kept in Q30 so that 31 steps stay exact to 0.1 %.
pub fn factor_q15(qdb: i32, mute: bool) -> u32 {
    if mute { return 0; }
    let steps = qdb / 6;
    let (per, count): (u64, i32) = if steps < 0 { (903_441_154, -steps) } else { (1_276_144_550, steps) }; // 10^(-0.075) or 10^(+0.075), in Q30
    let mut f: u64 = 1 << 30;
    for _ in 0..count.min(64) { f = f * per >> 30; }
    (f >> 15) as u32
}

/// One sample scaled by a Q15 factor, clipped to 16 bits.
pub fn scale(sample: i16, factor: u32) -> i16 { (sample as i64 * factor as i64 >> 15).clamp(i16::MIN as i64, i16::MAX as i64) as i16 }

/// Interleaved 16-bit little-endian samples scaled in place; nothing to do at 1.
pub fn scale_bytes(pcm: &mut [u8], factor: u32) {
    if factor == 1 << 15 { return; }
    for s in pcm.chunks_exact_mut(2) { let v = scale(i16::from_le_bytes([s[0], s[1]]), factor); s.copy_from_slice(&v.to_le_bytes()); }
}
