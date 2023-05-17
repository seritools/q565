use super::{ColorFormat, InfallibleDecodeOutput};
use alloc::vec::Vec;
use byteorder::ByteOrder;

pub struct VecDecodeOutput<'a, C: ColorFormat> {
    output: &'a mut Vec<C::OutputElement>,
    output_idx: usize,
}

impl<'a, C> VecDecodeOutput<'a, C>
where
    C: ColorFormat,
{
    #[inline]
    pub fn new(vec: &'a mut Vec<C::OutputElement>) -> Self {
        Self {
            output: vec,
            output_idx: 0,
        }
    }
}

impl<C> InfallibleDecodeOutput for VecDecodeOutput<'_, C>
where
    C: ColorFormat,
{
    #[inline]
    fn write_pixel<B: ByteOrder>(&mut self, color: u16) {
        self.output.push(C::to_output::<B>(color));
        self.output_idx += 1;
    }

    #[inline]
    fn write_many_pixels<B: ByteOrder>(&mut self, color: u16, count: usize) {
        let color = C::to_output::<B>(color);
        self.output.extend(core::iter::repeat(color).take(count));
        self.output_idx += count;
    }

    #[inline]
    fn max_len(&self) -> Option<usize> {
        None
    }

    #[inline]
    fn current_output_position(&self) -> usize {
        self.output_idx
    }
}
