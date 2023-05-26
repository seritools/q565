use crate::{
    decode::ops::{direct_bigger_diff, direct_small_diff, indexed_diff},
    utils::hash,
};
use byteorder::{ByteOrder, NativeEndian};
use core::hint::unreachable_unchecked;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Q565StreamingDecodeContext {
    state: Q565StreamingDecodeState,
    prev: u16,
    arr: [u16; 64],
}

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
enum Q565StreamingDecodeState {
    Default = 0,
    LumaOrDiffIndexedByte2(u8),
    RawRgb565Byte1,
    RawRgb565Byte2(u8),
}

impl Default for Q565StreamingDecodeContext {
    fn default() -> Self {
        Self::new()
    }
}

impl Q565StreamingDecodeContext {
    pub const fn new() -> Self {
        Self {
            state: Q565StreamingDecodeState::Default,
            prev: 0,
            arr: [0; 64],
        }
    }

    /// Decodes a Q565 image into a buffer in a streaming fashion, without the header.
    ///
    /// Returns the number of pixels written to the output buffer, if successful. Note that this
    /// doesn't accumulate over multiple calls. You'll need to keep track of the number of pixels
    /// written and pass the correct output slice to the next call.
    ///
    /// # Safety
    ///
    /// This function does not do *any* output bounds checks.
    ///
    /// The caller needs to ensure that the input is a valid Q565 image. Any failure to do so
    /// results in undefined behavior.
    pub unsafe fn streaming_decode_to_slice_unchecked<B: ByteOrder>(
        &mut self,
        input: &[u8],
        output: &mut [u16],
    ) -> usize {
        let mut output_idx = 0;
        let mut input_idx = 0;

        macro_rules! next {
            () => {
                if let Some(&b) = input.get(input_idx) {
                    input_idx += 1;
                    b
                } else {
                    return output_idx;
                }
            };
        }

        unsafe fn set_pixel<B: ByteOrder>(
            state: &mut Q565StreamingDecodeContext,
            pixel: u16,
            output: &mut [u16],
            output_idx: &mut usize,
        ) {
            state.prev = pixel;

            let mut buf = [0u8; 2];
            NativeEndian::write_u16(&mut buf, pixel);

            *output.get_unchecked_mut(*output_idx) = B::read_u16(&buf);
            *output_idx += 1;
        }

        loop {
            let byte = next!();
            let pixel = match self.state {
                Q565StreamingDecodeState::Default => {
                    let op = byte >> 6;

                    match op {
                        0b00 => {
                            let pixel = *self.arr.get_unchecked(usize::from(byte));
                            set_pixel::<B>(self, pixel, output, &mut output_idx);
                            continue;
                        }
                        0b01 => {
                            let pixel = direct_small_diff(self.prev, byte);
                            set_pixel::<B>(self, pixel, output, &mut output_idx);

                            continue;
                        }
                        0b10 => {
                            self.state = Q565StreamingDecodeState::LumaOrDiffIndexedByte2(byte);
                            continue;
                        }
                        0b11 => {
                            if byte == 0xFE {
                                self.state = Q565StreamingDecodeState::RawRgb565Byte1;
                                continue;
                            } else if byte != 0xFF {
                                let count = (byte & 0b0011_1111) + 1;
                                let count = usize::from(count);

                                let mut buf = [0u8; 2];
                                NativeEndian::write_u16(&mut buf, self.prev);

                                output
                                    .get_unchecked_mut(output_idx..)
                                    .get_unchecked_mut(..count)
                                    .fill(B::read_u16(&buf));
                                output_idx += count;

                                continue;
                            } else {
                                return output_idx;
                            }
                        }
                        _ => unsafe { unreachable_unchecked() },
                    }
                }
                Q565StreamingDecodeState::LumaOrDiffIndexedByte2(byte1) => {
                    let op = byte1 >> 5;
                    match op {
                        0b100 => direct_bigger_diff(self.prev, byte1, byte),
                        0b101 => indexed_diff(&self.arr, byte1, byte),
                        _ => unsafe { unreachable_unchecked() },
                    }
                }
                Q565StreamingDecodeState::RawRgb565Byte1 => {
                    self.state = Q565StreamingDecodeState::RawRgb565Byte2(byte);
                    continue;
                }
                Q565StreamingDecodeState::RawRgb565Byte2(byte1) => {
                    u16::from_le_bytes([byte1, byte])
                }
            };

            let index = hash(pixel);
            *self.arr.get_unchecked_mut(usize::from(index)) = pixel;
            set_pixel::<B>(self, pixel, output, &mut output_idx);
            self.state = Q565StreamingDecodeState::Default;
        }
    }
}
