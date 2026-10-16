use crate::{io, model};

// Device replies follow actual register triggers; this model does not establish RF power or calibration quality.
#[derive(Default)]
pub(crate) struct ChannelModel {
    pub commands: Vec<(u32, u64)>, pub shots: Vec<(usize, u32, u32, u32, u64)>,
    pub stuck_command: Option<usize>, pub stuck_tssi: bool, pub bad_dpk: bool,
    pub command_ready: Option<u64>, pub report_ready: Option<(u64, usize, u16)>,
}
impl ChannelModel {
    pub fn write(&mut self, inner: &mut model::Device, rf: &mut [[u32; 256]; 2], at: usize, previous: u32, value: u32) {
        if at == 0x10370 {
            let path = ((value >> 28) & 3) as usize; let reg = (value >> 20) & 255;
            if path == 0 && reg == 0x18 { rf[0][0xc5] |= 1 << 15; }
        }
        if at == 0x18000 && value & 1 != 0 {
            self.commands.push((value, inner.now)); self.command_ready = Some(inner.now + 10);
            let old = inner.get(0x18008, 4); inner.put(0x18008, 4, old & !(1 << 26));
            if value & 0xff00 == 0x300 {
                let path = usize::from(value & 0x20 != 0);
                rf[path][0x58] = (12 << 15) | (13 << 10); rf[path][0xa] = (20 << 14) | (21 << 4);
            }
        }
        if [0x15820, 0x17820].contains(&at) && value >> 31 != 0 && previous >> 31 == 0 {
            inner.put(0x11c18 + usize::from(at == 0x17820) * 0x2000, 4, 0);
        }
        if at == 0x109c0 && value & 1 != 0 && previous & 1 == 0
            && inner.get(0x10980, 4) & 1 != 0 && inner.get(0x109c4, 4) & 16 != 0 {
            let path = match inner.get(0x1458c, 4) >> 28 & 15 { 1 => 0, 2 => 1, value => panic!("invalid PMAC path {value}") };
            let power = inner.get(0x14594, 4) >> 22 & 511;
            assert_ne!(inner.get(0x109c4, 4) & 16, 0); assert_eq!(inner.get(0x10980, 4) & 0x10001, 0x10001);
            let report = match (path, power) { (0, 48) => 160, (0, 20) => 100, (1, 48) => 200, (1, 20) => 142, _ => panic!("unexpected raw PMAC power") };
            self.shots.push((path, power, inner.get(0x109c8, 4), inner.get(0x109c4, 4) >> 8, inner.now));
            self.report_ready = Some((inner.now + 40, path, report));
        }
        if at == 0x109c4 && value & 16 == 0 { self.report_ready = None; }
    }
    pub fn read(&self, inner: &model::Device, at: usize) -> Option<io::Result<u32>> {
        if at == 0x1bff8 { return Some(Ok(u32::from(self.command_ready.is_some_and(|t| inner.now >= t)
            && self.stuck_command != Some(self.commands.len())) * 0x55)); }
        if at == 0x180fc && self.commands.last().is_some_and(|&(v, _)| v >> 8 >= 0x10) {
            return Some(Ok(match inner.get(0x180d4, 4) >> 16 & 0x3f {
                3 => 0x8000,
                0 => (0x600 << 16) | (if self.bad_dpk { 100 } else { 200 } << 8) | 7,
                9 => (1 << 16) | 0xffe,
                6 => if inner.get(0x180bc, 4) & (1 << 14) != 0 { 3 << 4 } else { 100 << 16 | 100 },
                _ => 0,
            }));
        }
        if let Some((time, path, report)) = self.report_ready {
            if !self.stuck_tssi && inner.now >= time && at == 0x11c18 + path * 0x2000 { return Some(Ok(1 << 16 | u32::from(report))); }
        }
        None
    }
}
