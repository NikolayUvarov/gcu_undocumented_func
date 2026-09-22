// Interpolate within an RTC second using uptime; resynchronize at every RTC
// change. A stale/unavailable RTC freezes the last second, never inventing a
// new 100-second bucket that disagrees with the white corner indicators.
pub struct Cycle {
    second: Option<usize>,
    since: usize,
}

impl Cycle {
    pub const fn new() -> Self {
        Self {
            second: None,
            since: 0,
        }
    }

    pub fn observe(&mut self, seconds: usize, now: usize) {
        if seconds < 86400 && self.second != Some(seconds) {
            self.second = Some(seconds);
            self.since = now;
        }
    }

    pub fn phase(&self, now: usize) -> Option<usize> {
        self.second
            .map(|s| (s % 100) * 1000 + now.wrapping_sub(self.since).min(999))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrbitMode {
    Off,
    Simple,
    Ticks,
}

impl OrbitMode {
    pub fn toggle(self, requested: Self) -> Self {
        if self == requested {
            Self::Off
        } else {
            requested
        }
    }
}

// One clockwise turn per 100,000 ms, starting toward the first hour corner.
// Corners are 45 + 90*index degrees clockwise from twelve o'clock. Fixed
// point quarter-sine samples (scale 10,000), interpolated between samples,
// avoid a libm dependency in this no_std application.
pub fn point_at(phase: usize, radius: usize) -> (isize, isize) {
    const SIN: [isize; 26] = [
        0, 628, 1253, 1874, 2487, 3090, 3681, 4258, 4818, 5358, 5878, 6374, 6845, 7290, 7705, 8090,
        8443, 8763, 9048, 9298, 9511, 9686, 9823, 9921, 9980, 10000,
    ];
    let origin = 12_500 + crate::face::FIRST_CORNER * 25_000;
    let phase = (phase % 100_000 + origin) % 100_000;
    let part = phase % 25_000;
    let index = part / 1000;
    let fraction = (part % 1000) as isize;
    let sine = SIN[index] + (SIN[index + 1] - SIN[index]) * fraction / 1000;
    let cosine = SIN[25 - index] + (SIN[24 - index] - SIN[25 - index]) * fraction / 1000;
    let (x, y) = match phase / 25_000 {
        0 => (sine, -cosine),
        1 => (cosine, sine),
        2 => (-sine, cosine),
        _ => (-cosine, -sine),
    };
    let radius = radius as isize;
    (x * radius / 10000, y * radius / 10000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rtc_sync_interpolation_boundary_and_midnight() {
        let mut cycle = Cycle::new();
        assert_eq!(cycle.phase(50), None);
        cycle.observe(usize::MAX, 50);
        assert_eq!(cycle.phase(50), None);
        cycle.observe(3599, 1000);
        cycle.observe(3599, 1100); // Same second must not restart interpolation.
        assert_eq!(cycle.phase(1150), Some(99150));
        assert_eq!(cycle.phase(5000), Some(99999)); // Stale RTC stays in its bucket.
        cycle.observe(3600, 5000);
        assert_eq!(cycle.phase(5000), Some(0));
        cycle.observe(86399, 6000);
        assert_eq!(cycle.phase(6600), Some(99600));
        cycle.observe(0, 7000);
        assert_eq!(cycle.phase(7000), Some(0));
        cycle.observe(123, usize::MAX - 50);
        assert_eq!(cycle.phase(49), Some(23100)); // Uptime overflow.
    }

    #[test]
    fn quarter_turns_and_wrap_are_clockwise() {
        assert_eq!(point_at(0, 120), (84, 84));
        assert_eq!(point_at(25000, 120), (-84, 84));
        assert_eq!(point_at(50000, 120), (-84, -84));
        assert_eq!(point_at(75000, 120), (84, -84));
        assert_eq!(point_at(100000, 120), point_at(0, 120));
        for phase in (0..100000).step_by(100) {
            let a = point_at(phase, 120);
            let b = point_at(phase + 100, 120);
            assert!(a.0 * b.1 - a.1 * b.0 >= 0);
            assert!((a.0 - b.0).abs() <= 2 && (a.1 - b.1).abs() <= 2);
        }
    }

    #[test]
    fn full_orbit_clears_center_and_corner_indicators() {
        for half in [48, 96, 120, 153, 160] {
            let indicator = half as isize / 3;
            let corner = half as isize;
            for phase in (0..100000).step_by(100) {
                let radius = half * 3 / 4;
                let length = (half / 24).max(1);
                for (x, y) in [
                    point_at(phase, radius - length),
                    point_at(phase, radius + length),
                ] {
                    // The tick endpoints enclose the orbit and smaller moving dot.
                    assert!(x * x + y * y > (indicator + 3).pow(2));
                    for (cx, cy) in [
                        (corner, corner),
                        (-corner, corner),
                        (corner, -corner),
                        (-corner, -corner),
                    ] {
                        assert!((x - cx).pow(2) + (y - cy).pow(2) > (indicator + 3).pow(2));
                    }
                }
            }
        }
    }

    #[test]
    fn keys_select_modes_and_repeat_turns_them_off() {
        assert_eq!(OrbitMode::Off.toggle(OrbitMode::Simple), OrbitMode::Simple);
        assert_eq!(OrbitMode::Off.toggle(OrbitMode::Ticks), OrbitMode::Ticks);
        assert_eq!(OrbitMode::Simple.toggle(OrbitMode::Ticks), OrbitMode::Ticks);
        assert_eq!(
            OrbitMode::Ticks.toggle(OrbitMode::Simple),
            OrbitMode::Simple
        );
        assert_eq!(OrbitMode::Simple.toggle(OrbitMode::Simple), OrbitMode::Off);
        assert_eq!(OrbitMode::Ticks.toggle(OrbitMode::Ticks), OrbitMode::Off);
    }
}
