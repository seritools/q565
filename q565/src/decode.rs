use crate::{
    utils::{apply_diff, hash},
    ColorFormat, HeaderInfo,
};
use byteorder::ByteOrder;
use snafu::{ensure, Snafu};

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
#[snafu(module)]
pub enum DecodeUncheckedError {
    OutputTooSmall,
}

#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum DecodeError {
    OutputTooSmall,
    UnexpectedEof,
    InvalidMagic,
}

impl Q565DecodeContext {
    pub fn decode<B>(
        data: &[u8],
        output: impl InfallibleDecodeOutput,
    ) -> Result<(usize, HeaderInfo), DecodeError>
    where
        B: ByteOrder,
    {
        let mut state = Q565DecodeContext::new();
        state.decode_with_state::<B>(data, output)
    }

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
    pub unsafe fn decode_unchecked<B>(
        data: &[u8],
        output: impl InfallibleDecodeOutput,
    ) -> Result<usize, DecodeUncheckedError>
    where
        B: ByteOrder,
    {
        let mut state = Q565DecodeContext::new();
        state.decode_unchecked_with_state::<B>(data, output)
    }

    pub fn decode_with_state<B>(
        &mut self,
        data: &[u8],
        mut output: impl InfallibleDecodeOutput,
    ) -> Result<(usize, HeaderInfo), DecodeError>
    where
        B: ByteOrder,
    {
        #[inline(always)]
        fn set_pixel<B: ByteOrder>(
            state: &mut Q565DecodeContext,
            pixel: u16,
            output: &mut impl InfallibleDecodeOutput,
        ) {
            state.prev = pixel;
            output.write_pixel::<B>(pixel);
        }

        // Header size plus 1 byte for the end marker
        ensure!(data.len() >= 9, decode_error::UnexpectedEofSnafu);

        let (header, data) = data.split_at(8);
        let magic = &header[0..4];
        ensure!(magic == b"q565", decode_error::InvalidMagicSnafu);

        let width = u16::from_le_bytes([header[4], header[5]]);
        let height = u16::from_le_bytes([header[6], header[7]]);
        let header = HeaderInfo { width, height };

        ensure!(
            output
                .max_len()
                .map(|max_len| max_len >= (width as usize) * (height as usize))
                .unwrap_or(true),
            decode_error::OutputTooSmallSnafu
        );

        let mut data = data.iter().copied();
        let mut next = || data.next().ok_or(DecodeError::UnexpectedEof);

        loop {
            let byte = next()?;
            let op = byte >> 6;

            let pixel = match op {
                0b00 => {
                    let pixel = unsafe { *self.arr.get_unchecked(usize::from(byte)) };
                    set_pixel::<B>(self, pixel, &mut output);
                    continue;
                }
                0b01 => {
                    let pixel = self.direct_small_diff(byte);
                    set_pixel::<B>(self, pixel, &mut output);
                    continue;
                }
                0b10 => {
                    if byte & 0b0010_0000 == 0 {
                        self.direct_bigger_diff(byte, next()?)
                    } else {
                        self.indexed_diff(byte, next()?)
                    }
                }
                0b11 => {
                    if byte == 0xFE {
                        let pixel = [next()?, next()?];
                        u16::from_le_bytes(pixel)
                    } else if byte != 0xFF {
                        let count = (byte & 0b0011_1111) + 1;
                        let count = usize::from(count);

                        output.write_many_pixels::<B>(self.prev, count);
                        continue;
                    } else {
                        break;
                    }
                }
                _ => unsafe { core::hint::unreachable_unchecked() },
            };

            let index = hash(pixel);
            unsafe {
                *self.arr.get_unchecked_mut(usize::from(index)) = pixel;
            }
            set_pixel::<B>(self, pixel, &mut output);
        }

        Ok((output.current_output_position(), header))
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
    pub unsafe fn decode_unchecked_with_state<B>(
        &mut self,
        data: &[u8],
        mut output: impl InfallibleDecodeOutput,
    ) -> Result<usize, DecodeUncheckedError>
    where
        B: ByteOrder,
    {
        #[inline(always)]
        fn set_pixel<B: ByteOrder>(
            state: &mut Q565DecodeContext,
            pixel: u16,
            output: &mut impl InfallibleDecodeOutput,
        ) {
            state.prev = pixel;
            output.write_pixel::<B>(pixel);
        }

        let width = u16::from_le_bytes([*data.get_unchecked(4), *data.get_unchecked(5)]);
        let height = u16::from_le_bytes([*data.get_unchecked(6), *data.get_unchecked(7)]);
        let data = data.get_unchecked(8..);

        if output
            .max_len()
            .map(|max_len| max_len < (width as usize) * (height as usize))
            .unwrap_or(false)
        {
            return Err(DecodeUncheckedError::OutputTooSmall);
        }

        let mut input_idx = 0;
        let mut next = || {
            let b = *data.get_unchecked(input_idx);
            input_idx += 1;
            b
        };

        loop {
            let byte = next();
            let op = byte >> 6;

            let pixel = match op {
                0b00 => {
                    let pixel = *self.arr.get_unchecked(usize::from(byte));
                    set_pixel::<B>(self, pixel, &mut output);
                    continue;
                }
                0b01 => {
                    let pixel = self.direct_small_diff(byte);
                    set_pixel::<B>(self, pixel, &mut output);
                    continue;
                }
                0b10 => {
                    if byte & 0b0010_0000 == 0 {
                        self.direct_bigger_diff(byte, next())
                    } else {
                        self.indexed_diff(byte, next())
                    }
                }
                0b11 => {
                    if byte == 0xFE {
                        let pixel = [next(), next()];
                        u16::from_le_bytes(pixel)
                    } else if byte != 0xFF {
                        let count = (byte & 0b0011_1111) + 1;
                        let count = usize::from(count);

                        output.write_many_pixels::<B>(self.prev, count);
                        continue;
                    } else {
                        break;
                    }
                }
                _ => unsafe { core::hint::unreachable_unchecked() },
            };

            let index = hash(pixel);
            *self.arr.get_unchecked_mut(usize::from(index)) = pixel;
            set_pixel::<B>(self, pixel, &mut output);
        }

        Ok(output.current_output_position())
    }

    // OP: 0x101
    #[inline(always)]
    fn indexed_diff(&mut self, byte: u8, second_byte: u8) -> u16 {
        let g_diff = ((byte & 0b0001_1100) >> 2) as i8 - 4;
        let r_diff = (byte & 0b0000_0011) as i8 - 2;
        let b_diff = (second_byte >> 6) as i8 - 2;
        let index = usize::from(second_byte & 0b0011_1111);

        apply_diff(self.arr[index], r_diff, g_diff, b_diff)
    }

    // OP: 0x01
    #[inline(always)]
    fn direct_small_diff(&mut self, byte: u8) -> u16 {
        let (r_diff, g_diff, b_diff) = (
            ((byte >> 4) & 0b11) as i8 - 2,
            ((byte >> 2) & 0b11) as i8 - 2,
            (byte & 0b11) as i8 - 2,
        );

        apply_diff(self.prev, r_diff, g_diff, b_diff)
    }

    // OP: 0x100
    #[inline(always)]
    fn direct_bigger_diff(&mut self, byte: u8, rg_bg_diffs: u8) -> u16 {
        let g_diff = (byte & 0b0001_1111) as i8 - 16;
        let (rg_diff, bg_diff) = (
            (rg_bg_diffs >> 4) as i8 - 8,
            (rg_bg_diffs & 0b1111) as i8 - 8,
        );
        let (r_diff, b_diff) = (rg_diff + g_diff, bg_diff + g_diff);

        apply_diff(self.prev, r_diff, g_diff, b_diff)
    }
}

pub trait InfallibleDecodeOutput {
    fn write_pixel<B: ByteOrder>(&mut self, color: u16);
    fn write_many_pixels<B: ByteOrder>(&mut self, color: u16, count: usize);

    /// Returns the maximum number of pixels that can be written to the output buffer.
    ///
    /// `None` if the output buffer is unbounded.
    fn max_len(&self) -> Option<usize>;
    fn current_output_position(&self) -> usize;
}

pub struct UnsafeSliceDecodeOutput<'a, C: ColorFormat> {
    output: &'a mut [C::OutputElement],
    output_idx: usize,
}

impl<'a, C> UnsafeSliceDecodeOutput<'a, C>
where
    C: ColorFormat,
{
    /// # Safety
    ///
    /// This output does not do any bounds checking. The caller needs to ensure that the input Q565
    /// is valid and its header specifies the correct length.
    #[inline]
    pub unsafe fn new(slice: &'a mut [C::OutputElement]) -> Self {
        Self {
            output: slice,
            output_idx: 0,
        }
    }
}

impl<C> InfallibleDecodeOutput for UnsafeSliceDecodeOutput<'_, C>
where
    C: ColorFormat,
{
    #[inline]
    fn write_pixel<B: ByteOrder>(&mut self, color: u16) {
        unsafe {
            *self.output.get_unchecked_mut(self.output_idx) = C::to_output::<B>(color);
        }
        self.output_idx += 1;
    }

    #[inline]
    fn write_many_pixels<B: ByteOrder>(&mut self, color: u16, count: usize) {
        let color = C::to_output::<B>(color);
        unsafe {
            self.output
                .get_unchecked_mut(self.output_idx..)
                .get_unchecked_mut(..count)
                .fill(color);
        }
        self.output_idx += count;
    }

    #[inline]
    fn max_len(&self) -> Option<usize> {
        Some(self.output.len())
    }

    #[inline]
    fn current_output_position(&self) -> usize {
        self.output_idx
    }
}
