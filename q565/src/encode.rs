use crate::{
    consts::*,
    utils::{decode_565, diff_n, hash},
};
use alloc::vec::Vec;

#[cfg(feature = "std")]
mod std_api;
#[cfg(feature = "std")]
pub use std_api::*;

#[derive(Debug, Clone, Copy)]
pub struct Q565EncodeContext {
    pub prev: u16,
    pub prev_components: [u8; 3],

    pub arr: [u16; 64],
    pub arr_components: [[u8; 3]; 64],
}

impl Q565EncodeContext {
    pub const fn new() -> Self {
        Self {
            prev: 0,
            prev_components: [0; 3],

            arr: [0; 64],
            arr_components: [[0; 3]; 64],
        }
    }
}

impl Default for Q565EncodeContext {
    fn default() -> Self {
        Self::new()
    }
}

impl Q565EncodeContext {
    pub fn encode_to_vec(width: u16, height: u16, pixels: &[u16], w: &mut Vec<u8>) -> bool {
        let mut state = Q565EncodeContext::new();
        state.encode_to_vec_with_state(width, height, pixels, w)
    }

    pub fn encode_to_vec_with_state(
        &mut self,
        width: u16,
        height: u16,
        pixels: &[u16],
        w: &mut Vec<u8>,
    ) -> bool {
        if usize::from(width) * usize::from(height) != pixels.len() {
            return false;
        }

        w.extend_from_slice(b"q565");
        w.extend_from_slice(&width.to_le_bytes());
        w.extend_from_slice(&height.to_le_bytes());

        let mut pixels = pixels.iter();

        loop {
            let Some(&pixel) = pixels.next() else {
                break;
            };

            if pixel == self.prev {
                let slice = pixels.as_slice();
                let repeats = slice.iter().take_while(|&&p| p == self.prev).count();
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

            self.prev = pixel;
            let (r, g, b) = decode_565(pixel);
            let [r_prev, g_prev, b_prev] = self.prev_components;
            self.prev_components = [r, g, b];

            let hash = hash(pixel);
            let index = usize::from(hash);

            if self.arr[index] == pixel {
                w.push(hash);
                // already in arr
                continue;
            }

            let (r_diff, g_diff, b_diff) = (
                diff_n::<5>(r, r_prev),
                diff_n::<6>(g, g_prev),
                diff_n::<5>(b, b_prev),
            );

            if matches!((r_diff, g_diff, b_diff), (-2..=1, -2..=1, -2..=1)) {
                let mut b = Q565_OP_DIFF;
                b |= ((r_diff + 2) << 4) as u8;
                b |= ((g_diff + 2) << 2) as u8;
                b |= (b_diff + 2) as u8;
                w.push(b);
            } else {
                let rg_diff = r_diff - g_diff;
                let bg_diff = b_diff - g_diff;

                if matches!((rg_diff, g_diff, bg_diff), (-8..=7, -16..=15, -8..=7)) {
                    let bytes = [
                        (Q565_OP_LUMA | ((g_diff + 16) as u8)),
                        (((rg_diff + 8) as u8) << 4 | (bg_diff + 8) as u8),
                    ];
                    w.extend_from_slice(&bytes);
                } else if let Some(bytes) = self.arr_components.iter().enumerate().find_map(
                    |(i, &[r_arr, g_arr, b_arr])| {
                        let (r_diff, g_diff, b_diff) = (
                            diff_n::<5>(r, r_arr),
                            diff_n::<6>(g, g_arr),
                            diff_n::<5>(b, b_arr),
                        );

                        if matches!((r_diff, g_diff, b_diff), (-2..=1, -4..=3, -2..=1)) {
                            let bytes = [
                                (Q565_OP_DIFF_INDEXED
                                    | ((g_diff + 4) as u8) << 2
                                    | ((r_diff + 2) as u8)),
                                (((b_diff + 2) as u8) << 6 | i as u8),
                            ];
                            Some(bytes)
                        } else {
                            None
                        }
                    },
                ) {
                    w.extend_from_slice(&bytes);
                } else {
                    let [a, b] = pixel.to_le_bytes();
                    w.extend_from_slice(&[Q565_OP_RGB565, a, b]);
                }

                // add to color array
                self.arr[index] = pixel;
                self.arr_components[index] = [r, g, b];
            }
        }

        w.push(Q565_OP_END);

        true
    }
}
