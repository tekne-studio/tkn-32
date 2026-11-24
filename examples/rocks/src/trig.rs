//! Angles in 1/256 of a turn, 0 up and growing clockwise, and their sine
//! and cosine in 1/1024: integers, so every runner turns the ship alike.

/// The sine of a quarter turn, in 65 steps.
#[rustfmt::skip]
const QUARTER: [i16; 65] = [
    0, 25, 50, 75, 100, 125, 150, 175, 200, 224, 249, 273, 297, 321, 345, 369,
    392, 415, 438, 460, 483, 505, 526, 548, 569, 590, 610, 630, 650, 669, 688, 706,
    724, 742, 759, 775, 792, 807, 822, 837, 851, 865, 878, 891, 903, 915, 926, 936,
    946, 955, 964, 972, 980, 987, 993, 999, 1004, 1009, 1013, 1016, 1019, 1021, 1023, 1024,
    1024,
];

/// The scale of [`sin`] and [`cos`]: their value at a quarter turn.
pub const ONE: i32 = 1024;

pub fn sin(angle: u8) -> i32 {
    let i = (angle & 63) as usize;
    i32::from(match angle >> 6 {
        0 => QUARTER[i],
        1 => QUARTER[64 - i],
        2 => -QUARTER[i],
        _ => -QUARTER[64 - i],
    })
}

pub fn cos(angle: u8) -> i32 {
    sin(angle.wrapping_add(64))
}

/// `(x, y)` turned clockwise by `angle`, in the same unit.
pub fn turn(x: i32, y: i32, angle: u8) -> (i32, i32) {
    let (s, c) = (sin(angle), cos(angle));
    ((x * c - y * s) / ONE, (x * s + y * c) / ONE)
}

/// `length` along `angle`: up is `(0, -length)`.
pub fn along(angle: u8, length: i32) -> (i32, i32) {
    (sin(angle) * length / ONE, -cos(angle) * length / ONE)
}

/// The angle that points along `(dx, dy)` most nearly: of the steps
/// ahead of it, the one it lies least to the side of. Up when both are 0.
pub fn heading(dx: i32, dy: i32) -> u8 {
    let (dx, dy) = (i64::from(dx), i64::from(dy));
    let off = |angle: u8| {
        let (s, c) = (i64::from(sin(angle)), i64::from(cos(angle)));
        (s * dx - c * dy <= 0, (s * dy + c * dx).abs())
    };
    (0..=u8::MAX).min_by_key(|&angle| off(angle)).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quarter_turn_is_a_quarter() {
        assert_eq!((sin(0), cos(0)), (0, ONE));
        assert_eq!((sin(64), cos(64)), (ONE, 0));
        assert_eq!((sin(128), cos(128)), (0, -ONE));
        assert_eq!((sin(192), cos(192)), (-ONE, 0));
        assert_eq!(along(64, 100), (100, 0));
        assert_eq!(turn(0, -10, 64), (10, 0));
    }

    #[test]
    fn a_heading_points_where_along_goes() {
        assert_eq!(heading(0, 0), 0);
        for angle in 0..=u8::MAX {
            let (dx, dy) = along(angle, 1000);
            assert_eq!(heading(dx, dy), angle, "{angle}");
        }
    }
}
