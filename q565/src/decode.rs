use crate::utils::{apply_diff, hash, unlikely, ByteOrder};
use snafu::Snafu;

pub mod streaming_no_header;

#[cfg(feature = "alloc")]
mod alloc_api;
#[cfg(feature = "alloc")]
pub use alloc_api::*;

#[repr(C)]
pub struct Q565DecodeContext {
    pub prev: u16,
    pub arr: [u16; 64],
}

impl Q565DecodeContext {
    pub const fn new() -> Self {
        Self {
            arr: [0; 64],
            prev: 0,
        }
    }
}

impl Default for Q565DecodeContext {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Snafu)]
pub enum DecodeToSliceUncheckedError {
    OutputSliceTooSmall,
}

impl Q565DecodeContext {
    /// Decodes a Q565 image into a buffer.
    ///
    /// Returns the number of pixels written to the output buffer, if successful.
    ///
    /// # Safety
    ///
    /// This function does not do *any* bounds checks except checking that the output slice is big
    /// enough to hold the image based on the header size.
    ///
    /// The caller needs to ensure that the input is a valid Q565 image. Any failure to do so
    /// results in undefined behavior.
    pub unsafe fn decode_to_slice_unchecked<T: ByteOrder>(
        data: &[u8],
        output: &mut [u16],
    ) -> Result<usize, DecodeToSliceUncheckedError> {
        let mut state = Q565DecodeContext::new();
        state.decode_to_slice_unchecked_with_state::<T>(data, output)
    }

    /// Decodes a Q565 image into a buffer, with the given state as starting state.
    ///
    /// Returns the number of pixels written to the output buffer, if successful.
    ///
    /// # Safety
    ///
    /// This function does not do *any* bounds checks except checking that the output slice is big
    /// enough to hold the image based on the header size.
    ///
    /// The caller needs to ensure that the input is a valid Q565 image. Any failure to do so
    /// results in undefined behavior.
    pub unsafe fn decode_to_slice_unchecked_with_state<T: ByteOrder>(
        &mut self,
        data: &[u8],
        output: &mut [u16],
    ) -> Result<usize, DecodeToSliceUncheckedError> {
        unsafe fn set_pixel<T: ByteOrder>(
            state: &mut Q565DecodeContext,
            pixel: u16,
            output: &mut [u16],
            output_idx: &mut usize,
        ) {
            state.prev = pixel;
            *output.get_unchecked_mut(*output_idx) = T::to_wire(pixel);
            *output_idx += 1;
        }

        let width = u16::from_le_bytes([*data.get_unchecked(4), *data.get_unchecked(5)]);
        let height = u16::from_le_bytes([*data.get_unchecked(6), *data.get_unchecked(7)]);
        let data = data.get_unchecked(8..);

        if output.len() < usize::from(width) * usize::from(height) {
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
                let pixel = *self.arr.get_unchecked(usize::from(byte));
                set_pixel::<T>(self, pixel, output, &mut output_idx);

                continue;
            } else if unlikely(op == 0b11) {
                if byte == 0xFE {
                    let pixel = [next(), next()];
                    u16::from_le_bytes(pixel)
                } else if byte != 0xFF {
                    let count = (byte & 0b0011_1111) + 1;
                    let count = usize::from(count);

                    output
                        .get_unchecked_mut(output_idx..)
                        .get_unchecked_mut(..count)
                        .fill(T::to_wire(self.prev));
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

                let pixel = apply_diff(self.prev, r_diff, g_diff, b_diff);
                set_pixel::<T>(self, pixel, output, &mut output_idx);

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

                    apply_diff(self.prev, r_diff, g_diff, b_diff)
                } else {
                    let g_diff = ((byte & 0b0001_1100) >> 2) as i8 - 4;
                    let r_diff = (byte & 0b0000_0011) as i8 - 2;
                    let second_byte = next();
                    let b_diff = (second_byte >> 6) as i8 - 2;
                    let index = usize::from(second_byte & 0b0011_1111);

                    apply_diff(self.arr[index], r_diff, g_diff, b_diff)
                }
            } else {
                unsafe { core::hint::unreachable_unchecked() }
            };

            let index = hash(pixel);
            *self.arr.get_unchecked_mut(usize::from(index)) = pixel;
            set_pixel::<T>(self, pixel, output, &mut output_idx);
        }

        Ok(output_idx)
    }
}
