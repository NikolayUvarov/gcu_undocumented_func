// Corner indices run clockwise: top right, bottom right, bottom left, top left.
// Both the first hour range and the orbit's zero-second reference start here.
pub const FIRST_CORNER: usize = 1;
pub const COLORS: [u32; 6] = [0xFF0000, 0xFFFF00, 0x00FF00, 0x00FFFF, 0x0000FF, 0xFF00FF];
pub const WHITE: u32 = 0x606060;
pub const OFF: u32 = 0x080C12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Face {
    pub corners: [u32; 4],
    pub center: u32,
}

impl Face {
    pub const DARK: Self = Self {
        corners: [OFF; 4],
        center: OFF,
    };

    pub fn at(seconds: usize) -> Option<Self> {
        if seconds >= 86400 {
            return None;
        }
        let hour = seconds / 3600;
        let hour_corner = (FIRST_CORNER + hour / 6) % 4;
        let step = (seconds % 600) / 100;
        let mut face = Self {
            corners: [OFF; 4],
            center: COLORS[(seconds / 600) % 6],
        };
        face.corners[hour_corner] = COLORS[hour % 6];
        for free in 0..3 {
            let white = if step < 3 {
                free == step
            } else {
                free != step - 3
            };
            face.corners[(hour_corner + 1 + free) % 4] = if white { WHITE } else { OFF };
        }
        Some(face)
    }
}

pub fn time_text(seconds: usize) -> [u8; 8] {
    let h = seconds / 3600;
    let m = seconds / 60 % 60;
    let s = seconds % 60;
    [
        b'0' + (h / 10) as u8,
        b'0' + (h % 10) as u8,
        b':',
        b'0' + (m / 10) as u8,
        b'0' + (m % 10) as u8,
        b':',
        b'0' + (s / 10) as u8,
        b'0' + (s % 10) as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hour_and_ten_minute_colors_cover_the_day() {
        for hour in 0..24 {
            for tens in 0..6 {
                let face = Face::at(hour * 3600 + tens * 600).unwrap();
                let corner = [1, 2, 3, 0][hour / 6];
                assert_eq!(face.corners[corner], COLORS[hour % 6]);
                assert_eq!(face.center, COLORS[tens]);
                assert_eq!(
                    face.corners.iter().filter(|c| COLORS.contains(c)).count(),
                    1
                );
            }
        }
        assert_eq!(Face::at(86400), None);
        assert_eq!(Face::at(usize::MAX), None);
    }

    #[test]
    fn clockwise_white_patterns_change_at_exact_100_second_boundaries() {
        let patterns = [
            [WHITE, OFF, OFF],
            [OFF, WHITE, OFF],
            [OFF, OFF, WHITE],
            [OFF, WHITE, WHITE],
            [WHITE, OFF, WHITE],
            [WHITE, WHITE, OFF],
        ];
        for (segment, corner) in [1, 2, 3, 0].into_iter().enumerate() {
            for (step, expected) in patterns.iter().enumerate() {
                for offset in [0, 99] {
                    let face = Face::at(segment * 21600 + 1200 + step * 100 + offset).unwrap();
                    assert_eq!(face.center, COLORS[2]);
                    for free in 0..3 {
                        assert_eq!(face.corners[(corner + 1 + free) % 4], expected[free]);
                    }
                }
            }
        }
    }

    #[test]
    fn every_second_decodes_to_its_100_second_bucket() {
        for seconds in 0..86400 {
            let face = Face::at(seconds).unwrap();
            let corner = face
                .corners
                .iter()
                .position(|c| COLORS.contains(c))
                .unwrap();
            let hour = [3, 0, 1, 2][corner] * 6
                + COLORS
                    .iter()
                    .position(|c| *c == face.corners[corner])
                    .unwrap();
            let tens = COLORS.iter().position(|c| *c == face.center).unwrap();
            let whites = face.corners.iter().filter(|&&c| c == WHITE).count();
            let marker = if whites == 1 { WHITE } else { OFF };
            assert!(whites == 1 || whites == 2);
            let free = (0..3)
                .find(|i| face.corners[(corner + 1 + i) % 4] == marker)
                .unwrap();
            let decoded = hour * 3600 + tens * 600 + (free + if whites == 2 { 3 } else { 0 }) * 100;
            assert_eq!(decoded, seconds / 100 * 100);
        }
    }

    #[test]
    fn digital_time_formats_midnight_and_last_second() {
        assert_eq!(&time_text(0), b"00:00:00");
        assert_eq!(&time_text(6 * 3600 + 9 * 60 + 8), b"06:09:08");
        assert_eq!(&time_text(86399), b"23:59:59");
    }
}
