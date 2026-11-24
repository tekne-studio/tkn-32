//! Dots packed tight: each an index into a palette of [`COLOURS`] in
//! [`BITS`] bits, one after another from the top left, each dot's highest
//! bit first, so eight dots take five bytes. A cover's picture is packed
//! so, and so is any picture a tool keeps of a game's.

use crate::COLOURS;

/// The bits a dot takes: enough for [`COLOURS`].
pub const BITS: usize = COLOURS.trailing_zeros() as usize;

/// The bytes `dots` dots take.
pub const fn bytes(dots: usize) -> usize {
    (dots * BITS).div_ceil(8)
}

/// The dot at `i` of `packed`, or `None` past its end.
pub const fn dot(packed: &[u8], i: usize) -> Option<u8> {
    let bit = i * BITS;
    if bit + BITS > packed.len() * 8 {
        return None;
    }
    // The dot lies within the two bytes from its first, read as one
    // number, the first byte high.
    let at = bit / 8;
    let next = if at + 1 < packed.len() {
        packed[at + 1]
    } else {
        0
    };
    let two = (packed[at] as u16) << 8 | next as u16;
    Some((two >> (16 - BITS - bit % 8)) as u8 & (COLOURS as u8 - 1))
}

/// Packs `dots` into `out`, which is [`bytes`] long: a dot's bits past
/// [`BITS`] are dropped.
pub fn pack(dots: &[u8], out: &mut [u8]) {
    out.fill(0);
    for (i, &dot) in dots.iter().enumerate() {
        let bit = i * BITS;
        let at = bit / 8;
        let two = (u16::from(dot) & (COLOURS as u16 - 1)) << (16 - BITS - bit % 8);
        out[at] |= (two >> 8) as u8;
        if let Some(next) = out.get_mut(at + 1) {
            *next |= two as u8;
        }
    }
}

/// Unpacks `packed` into `dots` eight at a time, what a game's meter
/// charges least: whole groups of eight dots and five bytes, a tail of
/// fewer left as it was.
pub fn unpack(packed: &[u8], dots: &mut [u8]) {
    for (eight, five) in dots.chunks_exact_mut(8).zip(packed.chunks_exact(5)) {
        let forty = u64::from_be_bytes([0, 0, 0, five[0], five[1], five[2], five[3], five[4]]);
        eight.copy_from_slice(&spread(forty).to_le_bytes());
    }
}

/// The eight dots of five bits in `forty`, the first highest, into the
/// eight bytes of a `u64`, the first lowest: halved three times, each half
/// moved to its own lane, so eight dots take a dozen instructions.
const fn spread(forty: u64) -> u64 {
    // Four dots of 20 bits to each 32-bit lane, the first four low.
    let lanes = forty >> 20 | (forty & 0xf_ffff) << 32;
    // Two of 10 bits to each 16-bit lane.
    let lanes = (lanes >> 10 & 0x0000_03ff_0000_03ff) | (lanes & 0x0000_03ff_0000_03ff) << 16;
    // One of 5 bits to each byte.
    (lanes >> 5 & 0x001f_001f_001f_001f) | (lanes & 0x001f_001f_001f_001f) << 8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dots_packed_come_back_whole_by_one_and_by_eight() {
        // Every index, in an order that is no pattern.
        let dots: [u8; 160] = core::array::from_fn(|i| {
            ((i as u32).wrapping_mul(2_654_435_761) >> 24) as u8 & (COLOURS as u8 - 1)
        });
        let mut packed = [0; bytes(160)];
        pack(&dots, &mut packed);
        let mut back = [0; 160];
        unpack(&packed, &mut back);
        assert_eq!(back, dots);
        for (i, &dot) in dots.iter().enumerate() {
            assert_eq!(super::dot(&packed, i), Some(dot), "dot {i}");
        }
        assert_eq!(super::dot(&packed, dots.len()), None);
    }

    #[test]
    fn five_bits_are_eight_dots_in_five_bytes_the_first_high() {
        let mut packed = [0; 5];
        pack(&[31, 0, 0, 0, 0, 0, 0, 1], &mut packed);
        assert_eq!(packed, [0b1111_1000, 0, 0, 0, 0b0000_0001]);
        // A dot's bits past five are dropped.
        pack(&[0xff, 0, 0, 0, 0, 0, 0, 0], &mut packed);
        assert_eq!(packed[0], 0b1111_1000);
        assert_eq!((BITS, bytes(160 * 120)), (5, 12_000));
    }
}
