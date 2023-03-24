//! Q565 reference implementation.
//!
//! # Format
//!
//! ## Header
//!
//! - 4-byte magic: `q565`
//! - u16le width: 1..65535
//! - u16le height: 1..65535
//!
//! ## Stream format
//!
//! ```plain
//! .- QOI_OP_INDEX ----------.
//! |         Byte[0]         |
//! |  7  6  5  4  3  2  1  0 |
//! |-------+-----------------|
//! |  0  0 |     index       |
//! `-------------------------`
//! ```
//!
//! - 2-bit tag b00
//! - 6-bit index into the color index array: 0..63
//! - A valid encoder must not issue 2 or more consecutive QOI_OP_INDEX chunks to the same index.
//!   QOI_OP_RUN should be used instead.
//!
//!
//! ```plain
//! .- QOI_OP_DIFF -----------.
//! |         Byte[0]         |
//! |  7  6  5  4  3  2  1  0 |
//! |-------+-----+-----+-----|
//! |  0  1 |  dr |  dg |  db |
//! `-------------------------`
//! ```
//! - 2-bit tag b01
//! - 2-bit   red channel difference from the previous pixel between -2..1
//! - 2-bit green channel difference from the previous pixel between -2..1
//! - 2-bit  blue channel difference from the previous pixel between -2..1
//!
//!
//! ```plain
//! .- QOI_OP_LUMA -------------------------------------.
//! |         Byte[0]         |         Byte[1]         |
//! |  7  6  5  4  3  2  1  0 |  7  6  5  4  3  2  1  0 |
//! |----------+--------------+-------------+-----------|
//! |  1  0  0 |  green diff  |   dr - dg   |  db - dg  |
//! `---------------------------------------------------`
//! ```
//! - 3-bit tag b100
//! - 5-bit green channel difference from the previous pixel -16..15
//! - 4-bit   red channel difference minus green channel difference -8..7
//! - 4-bit  blue channel difference minus green channel difference -8..7
//!
//!
//! ```plain
//! .- QOI_OP_DIFF_INDEXED -----------------------------.
//! |         Byte[0]         |         Byte[1]         |
//! |  7  6  5  4  3  2  1  0 |  7  6  5  4  3  2  1  0 |
//! |----------+--------------+------+------------------|
//! |  1  0  1 | dg    |  dr  |  db  |            index |
//! `---------------------------------------------------`
//! ```
//! - 3-bit tag b101
//! - 3-bit green channel difference from the indexed array pixel between -4..3
//! - 2-bit   red channel difference from the indexed array pixel between -2..1
//! - 2-bit  blue channel difference from the indexed array pixel between -2..1
//! - 6-bit index into the color index array: 0..63
//!
//!
//! ```plain
//! .- QOI_OP_RUN ------------.
//! |         Byte[0]         |
//! |  7  6  5  4  3  2  1  0 |
//! |-------+-----------------|
//! |  1  1 |       run       |
//! `-------------------------`
//! ```
//! - 2-bit tag b11
//! - 6-bit run-length repeating the previous pixel: 1..62
//! - The run-length is stored with a bias of -1. Note that the run-lengths 63 and 64 (b111110 and
//!   b111111) are illegal as they are occupied by the QOI_OP_RGB565 and QOI_OP_END tag.
//!
//!
//! ```plain
//! .- QOI_OP_RGB565 -----------------------------.
//! |         Byte[0]         | Byte[1] | Byte[2] |
//! |  7  6  5  4  3  2  1  0 | 7 .. 0  | 7 .. 0  |
//! |-------------------------+---------+---------|
//! |  1  1  1  1  1  1  1  0 | rgb565le          |
//! `---------------------------------------------`
//! ```
//! - 8-bit tag b11111110
//! - 5-bit   red channel value
//! - 6-bit green channel value
//! - 5-bit  blue channel value
//!
//! ```plain
//! .- QOI_OP_END -------------
//! |         Byte[0]         |
//! |  7  6  5  4  3  2  1  0 |
//! |-------------------------+
//! |  1  1  1  1  1  1  1  1 |
//! `--------------------------
//! ```
//!
//! End of stream marker

#![cfg_attr(not(test), no_std)]

use utils::{apply_diff, hash, unlikely, ByteOrder};

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
pub mod alloc_api;
pub mod streaming_no_header;
pub mod utils;

#[repr(C)]
pub struct Q565Context {
    pub prev: u16,
    pub arr: [u16; 64],
}

impl Q565Context {
    pub const fn new() -> Self {
        Self {
            arr: [0; 64],
            prev: 0,
        }
    }
}

impl Default for Q565Context {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub enum DecodeToSliceUncheckedError {
    OutputSliceTooSmall,
}

/// Decodes a Q565 image into a buffer.
///
/// Returns the number of pixels written to the output buffer, if successful.
///
/// # Safety
/// This function does not do *any* bounds checks except checking that the output slice is big
/// enough to hold the image based on the header size.
///
/// The caller needs to ensure that the input is a valid Q565 image. Any failure to do so results
/// in undefined behavior.
pub unsafe fn decode_to_slice_unchecked<T: ByteOrder>(
    state: &mut Q565Context,
    data: &[u8],
    output: &mut [u16],
) -> Result<usize, DecodeToSliceUncheckedError> {
    unsafe fn set_pixel<T: ByteOrder>(
        state: &mut Q565Context,
        pixel: u16,
        output: &mut [u16],
        output_idx: &mut usize,
    ) {
        state.prev = pixel;
        *output.get_unchecked_mut(*output_idx) = T::to_wire(pixel);
        *output_idx += 1;
    }

    *state = Q565Context::new();

    let width = u16::from_le_bytes([*data.get_unchecked(4), *data.get_unchecked(5)]);
    let height = u16::from_le_bytes([*data.get_unchecked(6), *data.get_unchecked(7)]);
    let data = data.get_unchecked(8..);

    if output.len() < width as usize * height as usize {
        return Err(DecodeToSliceUncheckedError::OutputSliceTooSmall);
    }

    let mut output_idx = 0;
    let mut input_idx = 0;
    let mut next = || {
        let b = *data.get_unchecked(input_idx);
        input_idx += 1;
        b
    };

    loop {
        let byte = next();
        let op = byte >> 6;

        let pixel = if op == 0b00 {
            let pixel = *state.arr.get_unchecked(byte as usize);
            set_pixel::<T>(state, pixel, output, &mut output_idx);

            continue;
        } else if unlikely(op == 0b11) {
            if byte == 0xFE {
                let pixel = [next(), next()];
                u16::from_le_bytes(pixel)
            } else if byte != 0xFF {
                let count = (byte & 0b0011_1111) + 1;
                let count = count as usize;

                output
                    .get_unchecked_mut(output_idx..)
                    .get_unchecked_mut(..count)
                    .fill(T::to_wire(state.prev));
                output_idx += count;

                continue;
            } else {
                break;
            }
        } else if op == 0b01 {
            let (r_diff, g_diff, b_diff) = (
                ((byte >> 4) & 0b11) as i8 - 2,
                ((byte >> 2) & 0b11) as i8 - 2,
                (byte & 0b11) as i8 - 2,
            );

            let pixel = apply_diff(state.prev, r_diff, g_diff, b_diff);
            set_pixel::<T>(state, pixel, output, &mut output_idx);

            continue;
        } else if op == 0b10 {
            if byte & 0b0010_0000 == 0 {
                let g_diff = (byte & 0b0001_1111) as i8 - 16;
                let rg_bg_diffs = next();
                let (rg_diff, bg_diff) = (
                    (rg_bg_diffs >> 4) as i8 - 8,
                    (rg_bg_diffs & 0b1111) as i8 - 8,
                );
                let (r_diff, b_diff) = (rg_diff + g_diff, bg_diff + g_diff);

                apply_diff(state.prev, r_diff, g_diff, b_diff)
            } else {
                let g_diff = ((byte & 0b0001_1100) >> 2) as i8 - 4;
                let r_diff = (byte & 0b0000_0011) as i8 - 2;
                let second_byte = next();
                let b_diff = (second_byte >> 6) as i8 - 2;
                let index = (second_byte & 0b0011_1111) as usize;

                apply_diff(state.arr[index], r_diff, g_diff, b_diff)
            }
        } else {
            unsafe { core::hint::unreachable_unchecked() }
        };

        let index = hash(pixel);
        *state.arr.get_unchecked_mut(index as usize) = pixel;
        set_pixel::<T>(state, pixel, output, &mut output_idx);
    }

    Ok(output_idx)
}
