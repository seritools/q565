use crate::{
    consts::*,
    encode::Q565EncodeContext,
    utils::{decode_565, diff_n, hash},
};
use snafu::{ensure, ResultExt, Snafu};
use std::io::Write;

#[derive(Debug, Snafu)]
pub enum EncodeError {
    #[snafu(display(
        "Specified image dimensions don't match the number of pixels: {width} * {height} == {} pixels, but {pixel_count} pixels were given",
        width * height
    ))]
    InvalidDimensions {
        width: usize,
        height: usize,
        pixel_count: usize,
    },
    WriteIo {
        source: std::io::Error,
    },
}

impl Q565EncodeContext {
    pub fn encode<W: Write>(
        width: u16,
        height: u16,
        pixels: &[u16],
        w: W,
    ) -> Result<(), EncodeError> {
        let mut ctx = Q565EncodeContext::new();
        ctx.encode_with_state(width, height, pixels, w)
    }

    pub fn encode_header<W: Write>(width: u16, height: u16, mut w: W) -> Result<(), EncodeError> {
        let [w1, w2] = width.to_le_bytes();
        let [h1, h2] = height.to_le_bytes();
        let header = [b'q', b'5', b'6', b'5', w1, w2, h1, h2];
        w.write_all(&header).context(WriteIoSnafu)
    }

    pub fn encode_with_state<W: Write>(
        &mut self,
        width: u16,
        height: u16,
        pixels: &[u16],
        mut w: W,
    ) -> Result<(), EncodeError> {
        ensure!(
            usize::from(width) * usize::from(height) == pixels.len(),
            InvalidDimensionsSnafu {
                width,
                height,
                pixel_count: pixels.len()
            }
        );

        Self::encode_header(width, height, &mut w)?;
        self.encode_pixels(pixels, w)?;

        Ok(())
    }

    pub fn encode_pixels<W: Write>(&mut self, pixels: &[u16], mut w: W) -> Result<(), EncodeError> {
        macro_rules! w {
            ($bytes:expr) => {
                w.write_all($bytes).context(WriteIoSnafu)
            };
        }

        let mut pixels = pixels.iter();

        loop {
            let Some(&pixel) = pixels.next() else {
                break;
            };

            if pixel == self.prev {
                let slice = pixels.as_slice();
                let repeats = slice.iter().take_while(|&&p| p == self.prev).count();
                pixels = slice[repeats..].iter();

                // account for initial `pixel` from above
                let count = repeats + 1;

                let max_count_count = count / 62;
                let rest_count = count % 62;
                for _ in 0..max_count_count {
                    w!(&[Q565_OP_RUN | (62 - 1)])?;
                }
                if rest_count > 0 {
                    w!(&[Q565_OP_RUN | (rest_count - 1) as u8])?;
                }

                // already same as prev and already in color array
                continue;
            }

            self.prev = pixel;
            let (r, g, b) = decode_565(pixel);
            let [r_prev, g_prev, b_prev] = self.prev_components;
            self.prev_components = [r, g, b];

            let hash = hash(pixel);
            let index = usize::from(hash);

            if self.arr[index] == pixel {
                w!(&[Q565_OP_INDEX | hash])?;

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

                w!(&[b])?;
            } else {
                let rg_diff = r_diff - g_diff;
                let bg_diff = b_diff - g_diff;

                if matches!((rg_diff, g_diff, bg_diff), (-8..=7, -16..=15, -8..=7)) {
                    let bytes = [
                        (Q565_OP_LUMA | ((g_diff + 16) as u8)),
                        (((rg_diff + 8) as u8) << 4 | (bg_diff + 8) as u8),
                    ];

                    w!(&bytes)?;
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
                    w!(&bytes)?;
                } else {
                    let [a, b] = pixel.to_le_bytes();

                    w!(&[Q565_OP_RGB565, a, b])?;
                }

                // add to color array
                self.arr[index] = pixel;
                self.arr_components[index] = [r, g, b];
            }
        }

        w!(&[Q565_OP_END])?;

        Ok(())
    }
}
