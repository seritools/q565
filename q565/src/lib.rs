//! Reference implementation for the Q565 image format.
//!
//! Q565 is heavily based on the [QOI Image format](https://qoiformat.org/), but altered to support
//! the 16-bit RGB565 pixel format (and that format only).
//!
//! # Differences from QOI
//!
//! ## Header
//!
//! - 4-byte magic: `q565`
//! - u16le width (non-zero)
//! - u16le height (non-zero)
//!
//! ## Color array
//!
//! Q565 uses a simplified color array compared to the one from QOI. The "hash" function was
//! replaced with just adding the bytes of the RGB565 u16 together and keeping the lowest 6 bits of
//! the result. This resulted in smaller images on average and has the additional bonus of
//! eliminating the last usage of multiplication in the decoder loop, making it reasonably fast even
//! on microcontrollers without fast multiplication support.
//!
//! In addition, pixels that can already be described in one byte are not added to the color array
//! ([`Q565_OP_DIFF`](consts::Q565_OP_DIFF)). This helps keep the color array from being
//! flooded with similar colors.
//!
//! ## [`Q565_OP_DIFF_INDEXED`](consts::Q565_OP_DIFF_INDEXED)
//!
//! Since we only have 5/6 bits per channel, `Q565_OP_LUMA` was reduced to represent the green
//! channel difference with just 5 instead of 6 bits. The gained bit is now used as part of the tag,
//! to discern between the new [`Q565_OP_DIFF_INDEXED`](consts::Q565_OP_DIFF_INDEXED) and
//! [`Q565_OP_LUMA`](consts::Q565_OP_LUMA).
//!
//! `Q565_OP_DIFF_INDEXED` is similar to [`Q565_OP_DIFF`](consts::Q565_OP_DIFF), but instead applies
//! the difference to a color from the color array. This results in an up to 5% smaller image size,
//! at the expense of a slower encoder (needing to calculate up to 64 color differences in the worst
//! case). If a faster encoder is needed, this operation can be omitted.
//!
//! # Stream format
//!
//! See [consts] for the different operation types.
#![cfg_attr(not(any(test, feature = "std")), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
pub mod encode;

pub mod decode;
pub mod utils;

pub use decode::Q565DecodeContext;
#[cfg(feature = "alloc")]
pub use encode::Q565EncodeContext;

#[derive(Debug, Clone)]
pub struct HeaderInfo {
    pub width: u16,
    pub height: u16,
}

pub mod consts {
    /// Re-emit a pixel from the color array.
    ///
    /// ```plain
    /// .- Q565_OP_INDEX ---------.
    /// |         Byte[0]         |
    /// |  7  6  5  4  3  2  1  0 |
    /// |-------+-----------------|
    /// |  0  0 |     index       |
    /// `-------------------------`
    /// ```
    ///
    /// - 2-bit tag b00
    /// - 6-bit index into the color array: 0..63
    /// - A valid encoder must not issue 2 or more consecutive Q565_OP_INDEX chunks to the same
    ///   index. Q565_OP_RUN should be used instead.
    pub const Q565_OP_INDEX: u8 = 0b0000_0000;

    /// Calculate a pixel based on a 2-bit difference from the previous pixel.
    ///
    /// ```plain
    /// .- Q565_OP_DIFF ----------.
    /// |         Byte[0]         |
    /// |  7  6  5  4  3  2  1  0 |
    /// |-------+-----+-----+-----|
    /// |  0  1 |  dr |  dg |  db |
    /// `-------------------------`
    /// ```
    ///
    /// - 2-bit tag b01
    /// - 2-bit red channel difference from the previous pixel between -2..1, stored with a bias
    ///   of 2
    /// - 2-bit green channel difference from the previous pixel between -2..1, stored with a bias
    ///   of 2
    /// - 2-bit blue channel difference from the previous pixel between -2..1, stored with a bias
    ///   of 2
    ///
    /// Since the resulting pixel is already encoded in one byte, it is _not_ added to the color
    /// array.
    pub const Q565_OP_DIFF: u8 = 0b0100_0000;

    /// Calculate a pixel based on a 5-bit green-channel difference from the previous pixel, and
    /// differences to the green-channel difference for red and blue.
    ///
    ///  ```plain
    /// .- Q565_OP_LUMA ------------------------------------.
    /// |         Byte[0]         |         Byte[1]         |
    /// |  7  6  5  4  3  2  1  0 |  7  6  5  4  3  2  1  0 |
    /// |----------+--------------+-------------+-----------|
    /// |  1  0  0 |  green diff  |   dr - dg   |  db - dg  |
    /// `---------------------------------------------------`
    /// ```
    ///
    /// - 3-bit tag b100
    /// - 5-bit green channel difference from the previous pixel (`-16..15`), stored with a bias of
    ///   16
    /// - 4-bit red channel difference minus green channel difference (`-8..7`), stored with a bias
    ///   of 8
    /// - 4-bit blue channel difference minus green channel difference (`-8..7`), stored with a bias
    ///   of 8
    pub const Q565_OP_LUMA: u8 = 0b1000_0000;

    /// Calculate a pixel based on a color in the color array, and applying a difference to it.
    ///
    /// ```plain
    /// .- Q565_OP_DIFF_INDEXED ----------------------------.
    /// |         Byte[0]         |         Byte[1]         |
    /// |  7  6  5  4  3  2  1  0 |  7  6  5  4  3  2  1  0 |
    /// |----------+--------------+------+------------------|
    /// |  1  0  1 | dg    |  dr  |  db  |            index |
    /// `---------------------------------------------------`
    /// ```
    ///
    /// - 3-bit tag b101
    /// - 3-bit green channel difference from the indexed array pixel between -4..3
    /// - 2-bit   red channel difference from the indexed array pixel between -2..1
    /// - 2-bit  blue channel difference from the indexed array pixel between -2..1
    /// - 6-bit index into the color array: 0..63
    pub const Q565_OP_DIFF_INDEXED: u8 = 0b1010_0000;

    /// Repeats the last pixel.
    ///
    /// ```plain
    /// .- Q565_OP_RUN -----------.
    /// |         Byte[0]         |
    /// |  7  6  5  4  3  2  1  0 |
    /// |-------+-----------------|
    /// |  1  1 |       run       |
    /// `-------------------------`
    /// ```
    ///
    /// - 2-bit tag b11
    /// - 6-bit run-length repeating the previous pixel: 1..62
    /// - The run-length is stored with a bias of -1. Note that the run-lengths 63 and 64 (`b111110`
    ///   and `b111111`) are illegal as they are occupied by the Q565_OP_RGB565 and Q565_OP_END tag.
    pub const Q565_OP_RUN: u8 = 0b1100_0000;

    /// Emits a full raw pixel.
    ///
    /// ```plain
    /// .- Q565_OP_RGB565 ----------------------------.
    /// |         Byte[0]         | Byte[1] | Byte[2] |
    /// |  7  6  5  4  3  2  1  0 | 7 .. 0  | 7 .. 0  |
    /// |-------------------------+---------+---------|
    /// |  1  1  1  1  1  1  1  0 | RGB565LE          |
    /// `---------------------------------------------`
    /// ```
    ///
    /// - 8-bit tag b11111110
    /// - 16-bit RGB565 pixel, little-endian
    pub const Q565_OP_RGB565: u8 = 0b1111_1110;

    /// Marks the end of the stream.
    ///
    /// ```plain
    /// .- Q565_OP_END -----------.
    /// |         Byte[0]         |
    /// |  7  6  5  4  3  2  1  0 |
    /// |-------------------------+
    /// |  1  1  1  1  1  1  1  1 |
    /// `-------------------------`
    /// ```
    pub const Q565_OP_END: u8 = 0b1111_1111;
}
