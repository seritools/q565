use crate::{
    utils::{apply_diff, decode_565, diff_n, hash, ByteOrder},
    Q565Context,
};
use alloc::vec::Vec;

pub fn encode_to_vec(width: u16, height: u16, pixels: &[u16], w: &mut Vec<u8>) -> bool {
    if usize::from(width) * usize::from(height) != pixels.len() {
        return false;
    }

    w.extend_from_slice(b"q565");
    w.extend_from_slice(&width.to_le_bytes());
    w.extend_from_slice(&height.to_le_bytes());

    let mut state = Q565Context::new();
    let mut pixels = pixels.iter();

    loop {
        let Some(&pixel) = pixels.next() else {
            break;
        };

        if pixel == state.prev {
            let slice = pixels.as_slice();
            let repeats = slice.iter().take_while(|&&p| p == state.prev).count();
            pixels = slice[repeats..].iter();

            // initial pixel
            let count = repeats + 1;

            let max_count_count = count / 62;
            let rest_count = count % 62;
            for _ in 0..max_count_count {
                w.push(0b1100_0000 | (62 - 1));
            }
            if rest_count > 0 {
                w.push(0b1100_0000 | (rest_count - 1) as u8);
            }

            // already same as prev, no need to update
            // already same as prev, already in arr
            continue;
        }

        let index = hash(pixel);
        if state.arr[usize::from(index)] == pixel {
            w.push(index);
            state.prev = pixel;
            // already in arr
            continue;
        }

        let (r, g, b) = decode_565(pixel);
        let (r_prev, g_prev, b_prev) = decode_565(state.prev);
        let (r_diff, g_diff, b_diff) = (
            diff_n::<5>(r, r_prev),
            diff_n::<6>(g, g_prev),
            diff_n::<5>(b, b_prev),
        );

        let rg_diff = r_diff - g_diff;
        let bg_diff = b_diff - g_diff;

        if matches!((r_diff, g_diff, b_diff), (-2..=1, -2..=1, -2..=1)) {
            let b = 0b0100_0000 | ((r_diff + 2) << 4) | ((g_diff + 2) << 2) | (b_diff + 2);
            w.push(b as u8);
            state.prev = pixel;

            // don't add to arr, too similar to prev
            continue;
        } else if matches!((rg_diff, g_diff, bg_diff), (-8..=7, -16..=15, -8..=7)) {
            let bytes = [
                (0b1000_0000u8 | ((g_diff + 16) as u8)),
                (((rg_diff + 8) as u8) << 4 | (bg_diff + 8) as u8),
            ];
            w.extend_from_slice(&bytes);
        } else if let Some(bytes) = state.arr.iter().enumerate().find_map(|(i, &p)| {
            let (r_arr, g_arr, b_arr) = decode_565(p);
            let (r_diff, g_diff, b_diff) = (
                diff_n::<5>(r, r_arr),
                diff_n::<6>(g, g_arr),
                diff_n::<5>(b, b_arr),
            );

            if matches!((r_diff, g_diff, b_diff), (-2..=1, -4..=3, -2..=1)) {
                let bytes = [
                    (0b1010_0000u8 | ((g_diff + 4) as u8) << 2 | ((r_diff + 2) as u8)),
                    (((b_diff + 2) as u8) << 6 | i as u8),
                ];
                Some(bytes)
            } else {
                None
            }
        }) {
            w.extend_from_slice(&bytes);
        } else {
            let [a, b] = pixel.to_le_bytes();
            w.extend_from_slice(&[0b11111110, a, b]);
        }

        state.arr[usize::from(index)] = pixel;
        state.prev = pixel;
    }

    w.push(0b11111111);

    true
}

#[derive(Debug)]
pub enum DecodeToVecError {
    UnexpectedEof,
    InvalidMagic,
}

pub struct Header {
    pub width: u16,
    pub height: u16,
}

pub fn decode_to_vec<T: ByteOrder>(
    data: &[u8],
    w: &mut Vec<u16>,
) -> Result<Header, DecodeToVecError> {
    let mut state = Q565Context::new();

    // Header size plus 1 byte for the end marker
    if data.len() < 9 {
        return Err(DecodeToVecError::UnexpectedEof);
    }

    let (header, data) = data.split_at(8);
    let magic = &header[0..4];
    if magic != b"q565" {
        return Err(DecodeToVecError::InvalidMagic);
    }

    let width = u16::from_le_bytes([header[4], header[5]]);
    let height = u16::from_le_bytes([header[6], header[7]]);
    let header = Header { width, height };

    let mut data = data.iter().copied();
    let mut next = || data.next().ok_or(DecodeToVecError::UnexpectedEof);

    loop {
        let byte = next()?;
        let op = byte >> 6;

        let pixel = match op {
            0b11 => match byte {
                0xFF => break,
                0xFE => {
                    let pixel = [next()?, next()?];
                    u16::from_le_bytes(pixel)
                }
                _ => {
                    let count = (byte & 0b0011_1111) + 1;
                    w.extend(core::iter::repeat(T::to_wire(state.prev)).take(usize::from(count)));
                    continue;
                }
            },
            0b00 => {
                let pixel = state.arr[usize::from(byte)];
                state.prev = pixel;
                w.push(T::to_wire(pixel));
                continue;
            }
            0b10 => {
                if byte & 0b0010_0000 == 0 {
                    let g_diff = (byte & 0b0001_1111) as i8 - 16;
                    let rg_bg_diffs = next()?;
                    let (rg_diff, bg_diff) = (
                        (rg_bg_diffs >> 4) as i8 - 8,
                        (rg_bg_diffs & 0b1111) as i8 - 8,
                    );
                    let (r_diff, b_diff) = (rg_diff + g_diff, bg_diff + g_diff);

                    apply_diff(state.prev, r_diff, g_diff, b_diff)
                } else {
                    let g_diff = ((byte & 0b0001_1100) >> 2) as i8 - 4;
                    let r_diff = (byte & 0b0000_0011) as i8 - 2;
                    let second_byte = next()?;
                    let b_diff = (second_byte >> 6) as i8 - 2;
                    let index = usize::from(second_byte & 0b0011_1111);

                    apply_diff(state.arr[index], r_diff, g_diff, b_diff)
                }
            }
            0b01 => {
                let (r_diff, g_diff, b_diff) = (
                    ((byte >> 4) & 0b11) as i8 - 2,
                    ((byte >> 2) & 0b11) as i8 - 2,
                    (byte & 0b11) as i8 - 2,
                );

                let pixel = apply_diff(state.prev, r_diff, g_diff, b_diff);
                state.prev = pixel;
                w.push(T::to_wire(pixel));
                continue;
            }
            _ => unreachable!(),
        };

        let index = usize::from(hash(pixel));
        state.arr[index] = pixel;
        state.prev = pixel;
        w.push(T::to_wire(pixel));
    }

    Ok(header)
}
