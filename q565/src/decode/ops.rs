use crate::utils::{decode_565, encode_rgb565_unchecked, sum_n};

// OP: 0x101
#[inline(always)]
pub(crate) const fn indexed_diff(color_array: &[u16; 64], byte: u8, second_byte: u8) -> u16 {
    let g_diff = ((byte & 0b0001_1100) >> 2) as i8 - 4;
    let r_diff = (byte & 0b0000_0011) as i8 - 2;
    let b_diff = (second_byte >> 6) as i8 - 2;
    let index = (second_byte & 0b0011_1111) as usize;

    apply_diff(color_array[index], r_diff, g_diff, b_diff)
}

// OP: 0x01
#[inline(always)]
pub(crate) const fn direct_small_diff(prev: u16, byte: u8) -> u16 {
    let (r_diff, g_diff, b_diff) = (
        ((byte >> 4) & 0b11) as i8 - 2,
        ((byte >> 2) & 0b11) as i8 - 2,
        (byte & 0b11) as i8 - 2,
    );

    apply_diff(prev, r_diff, g_diff, b_diff)
}

// OP: 0x100
#[inline(always)]
pub(crate) const fn direct_bigger_diff(prev: u16, byte: u8, rg_bg_diffs: u8) -> u16 {
    let g_diff = (byte & 0b0001_1111) as i8 - 16;
    let (rg_diff, bg_diff) = (
        (rg_bg_diffs >> 4) as i8 - 8,
        (rg_bg_diffs & 0b1111) as i8 - 8,
    );
    let (r_diff, b_diff) = (rg_diff + g_diff, bg_diff + g_diff);

    apply_diff(prev, r_diff, g_diff, b_diff)
}

#[inline]
pub(crate) const fn apply_diff(prev: u16, r_diff: i8, g_diff: i8, b_diff: i8) -> u16 {
    let [r, g, b] = decode_565(prev);
    encode_rgb565_unchecked([
        sum_n::<5>(r, r_diff),
        sum_n::<6>(g, g_diff),
        sum_n::<5>(b, b_diff),
    ])
}
