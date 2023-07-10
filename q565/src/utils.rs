#[inline]
pub(crate) const fn hash(pixel: u16) -> u8 {
    // Sicne the bytes are just added together, native endianness is fine here.
    let [a, b] = pixel.to_ne_bytes();
    a.wrapping_add(b) & 0b111111 // % 64
}

/// Computes the signed difference between two numbers. (N-bit numbers)
#[cfg(feature = "alloc")]
pub const fn diff_n<const N: u8>(a: u8, b: u8) -> i8 {
    (a.wrapping_sub(b) as i8) << (8 - N) >> (8 - N)
}

/// Applies an signed difference to a number. (N-bit numbers)
#[inline]
pub const fn sum_n<const N: u8>(a: u8, d: i8) -> u8 {
    (((a as i8).wrapping_add(d)) << (8 - N)) as u8 >> (8 - N)
}

/// Splits a RGB565 pixel into its components.
#[inline]
pub const fn decode_565(pixel: u16) -> [u8; 3] {
    let r = (pixel & 0b1111_1000_0000_0000) >> 11;
    let g = (pixel & 0b0000_0111_1110_0000) >> 5;
    let b = pixel & 0b0000_0000_0001_1111;

    [r as u8, g as u8, b as u8]
}

/// Compose the 5-bit R, 6-bit G, and 5-bit B values into a RGB565 u16 pixel. Does not mask off
/// higher bits if they are set.
#[inline]
pub const fn encode_rgb565_unchecked([r, g, b]: [u8; 3]) -> u16 {
    ((r as u16) << 11) | ((g as u16) << 5) | (b as u16)
}

/// Converts an RGB888 pixel into an RGB565 pixel.
#[inline]
pub const fn rgb888_to_rgb565([r, g, b]: [u8; 3]) -> [u8; 3] {
    // https://stackoverflow.com/questions/2442576/how-does-one-convert-16-bit-rgb565-to-24-bit-rgb888
    let r = (r as u32 * 249 + 1014) >> 11;
    let g = (g as u32 * 253 + 505) >> 10;
    let b = (b as u32 * 249 + 1014) >> 11;

    [r as u8, g as u8, b as u8]
}

/// Converts an RGB565 pixel into an RGB888 pixel.
#[inline]
pub const fn rgb565_to_rgb888([r, g, b]: [u8; 3]) -> [u8; 3] {
    // https://stackoverflow.com/questions/2442576/how-does-one-convert-16-bit-rgb565-to-24-bit-rgb888

    let r = (r as u32 * 527 + 23) >> 6;
    let g = (g as u32 * 259 + 33) >> 6;
    let b = (b as u32 * 527 + 23) >> 6;

    [r as u8, g as u8, b as u8]
}
