use crate::{
    decode::Q565DecodeContext,
    utils::{apply_diff, hash, ByteOrder},
    HeaderInfo,
};
use alloc::vec::Vec;
use snafu::Snafu;

#[derive(Debug, Snafu)]
pub enum DecodeToVecError {
    UnexpectedEof,
    InvalidMagic,
}

impl Q565DecodeContext {
    pub fn decode_to_vec<T: ByteOrder>(
        data: &[u8],
        w: &mut Vec<u16>,
    ) -> Result<HeaderInfo, DecodeToVecError> {
        let mut state = Q565DecodeContext::new();
        state.decode_to_vec_with_state::<T>(data, w)
    }

    pub fn decode_to_vec_with_state<T: ByteOrder>(
        &mut self,
        data: &[u8],
        w: &mut Vec<u16>,
    ) -> Result<HeaderInfo, DecodeToVecError> {
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
        let header = HeaderInfo { width, height };

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
                        w.extend(
                            core::iter::repeat(T::to_wire(self.prev)).take(usize::from(count)),
                        );
                        continue;
                    }
                },
                0b00 => {
                    let pixel = self.arr[usize::from(byte)];
                    self.prev = pixel;
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

                        apply_diff(self.prev, r_diff, g_diff, b_diff)
                    } else {
                        let g_diff = ((byte & 0b0001_1100) >> 2) as i8 - 4;
                        let r_diff = (byte & 0b0000_0011) as i8 - 2;
                        let second_byte = next()?;
                        let b_diff = (second_byte >> 6) as i8 - 2;
                        let index = usize::from(second_byte & 0b0011_1111);

                        apply_diff(self.arr[index], r_diff, g_diff, b_diff)
                    }
                }
                0b01 => {
                    let (r_diff, g_diff, b_diff) = (
                        ((byte >> 4) & 0b11) as i8 - 2,
                        ((byte >> 2) & 0b11) as i8 - 2,
                        (byte & 0b11) as i8 - 2,
                    );

                    let pixel = apply_diff(self.prev, r_diff, g_diff, b_diff);
                    self.prev = pixel;
                    w.push(T::to_wire(pixel));
                    continue;
                }
                _ => unreachable!(),
            };

            let index = usize::from(hash(pixel));
            self.arr[index] = pixel;
            self.prev = pixel;
            w.push(T::to_wire(pixel));
        }

        Ok(header)
    }
}
