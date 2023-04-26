#![no_std]

use core::mem::{align_of, size_of};
use q565::utils::{BigEndian, LittleEndian};

#[panic_handler]
fn panic_handler(_info: &core::panic::PanicInfo) -> ! {
    unsafe { core::hint::unreachable_unchecked() }
}

#[repr(C)]
pub struct Q565DecodeContext {
    pub internal: [u16; 65],
}

const _: () = {
    assert!(size_of::<Q565DecodeContext>() == size_of::<q565::decode::Q565DecodeContext>());
    assert!(align_of::<Q565DecodeContext>() == align_of::<q565::decode::Q565DecodeContext>());
};

/// Decodes a Q565 image from the given input buffer into the given output buffer that is RGB565
/// (little-endian).
///
/// - `context`: Pointer to space for the context struct
/// - `input`: Pointer to the input buffer
/// - `input_len`: Length of the input buffer, in bytes
/// - `output`: Pointer to the output buffer
/// - `output_len`: Length of the output buffer, in 16-bit words
///
/// Returns the number of pixels written to the output buffer, if successful, or -1 otherwise.
///
/// # Safety
///
/// Behavior is undefined if the input is not a valid Q565 image stream.
#[no_mangle]
pub unsafe extern "C" fn q565_decode_le(
    context: *mut Q565DecodeContext,
    input: *const u8,
    input_len: usize,
    output: *mut u16,
    output_len: usize,
) -> isize {
    let input = unsafe { core::slice::from_raw_parts(input, input_len) };
    let output = unsafe { core::slice::from_raw_parts_mut(output, output_len) };

    match q565::decode::Q565DecodeContext::decode_to_slice_unchecked_with_state::<LittleEndian>(
        &mut *context.cast::<q565::decode::Q565DecodeContext>(),
        input,
        output,
    ) {
        Ok(len) => len as isize,
        Err(_) => -1,
    }
}

/// Decodes a Q565 image from the given input buffer into the given output buffer that is RGB565
/// (big-endian).
///
/// - `context`: Pointer to space for the context struct
/// - `input`: Pointer to the input buffer
/// - `input_len`: Length of the input buffer, in bytes
/// - `output`: Pointer to the output buffer
/// - `output_len`: Length of the output buffer, in 16-bit words
///
/// Returns the number of pixels written to the output buffer, if successful, or -1 otherwise.
///
/// # Safety
///
/// Behavior is undefined if the input is not a valid Q565 image stream.
#[no_mangle]
pub unsafe extern "C" fn q565_decode_be(
    context: *mut Q565DecodeContext,
    input: *const u8,
    input_len: usize,
    output: *mut u16,
    output_len: usize,
) -> isize {
    let input = unsafe { core::slice::from_raw_parts(input, input_len) };
    let output = unsafe { core::slice::from_raw_parts_mut(output, output_len) };

    match q565::decode::Q565DecodeContext::decode_to_slice_unchecked_with_state::<BigEndian>(
        &mut *context.cast::<q565::decode::Q565DecodeContext>(),
        input,
        output,
    ) {
        Ok(len) => len as isize,
        Err(_) => -1,
    }
}

#[repr(C)]
pub struct Q565StreamingDecodeContext {
    pub internal: [u16; 66],
}

const _: () = {
    assert!(
        size_of::<Q565StreamingDecodeContext>()
            == size_of::<q565::decode::streaming_no_header::Q565StreamingDecodeContext>()
    );
    assert!(
        align_of::<Q565StreamingDecodeContext>()
            == align_of::<q565::decode::streaming_no_header::Q565StreamingDecodeContext>()
    );
};

/// Decodes a Q565 image (*without header*) from the given input buffer into the given output
/// buffer that is RGB565 (little-endian).
///
/// - `context`: Pointer to space for the context struct. This needs to be zero-initialized before
///   the first call to this function, for each new frame.
/// - `input`: Pointer to the input buffer
/// - `input_len`: Length of the input buffer, in bytes
/// - `output`: Pointer to the output buffer
/// - `output_len`: Length of the output buffer, in 16-bit words
///
/// Returns the number of *pixels* written to the output buffer. Note that this
/// doesn't accumulate over multiple calls. You'll need to keep track of the number of pixels
/// written and pass the correct output pointer to further calls.
///
/// # Safety
///
/// Behavior is undefined if:
/// - the concatenated input is not a valid Q565 image stream
/// - if the context is mutated between calls belonging to the same Q565 image stream
/// - if the output buffer is too small to fit the decoded image.
#[no_mangle]
pub unsafe extern "C" fn q565_streaming_decode_le(
    context: *mut Q565StreamingDecodeContext,
    input: *const u8,
    input_len: usize,
    output: *mut u16,
    output_len: usize,
) -> isize {
    let input = unsafe { core::slice::from_raw_parts(input, input_len) };
    let output = unsafe { core::slice::from_raw_parts_mut(output, output_len) };

    q565::decode::streaming_no_header::Q565StreamingDecodeContext::streaming_decode_to_slice_unchecked::<LittleEndian>(
        &mut *context.cast::<q565::decode::streaming_no_header::Q565StreamingDecodeContext>(),
        input,
        output,
    ) as isize
}

/// Decodes a Q565 image (*without header*) from the given input buffer into the given output
/// buffer that is RGB565 (big-endian).
///
/// - `context`: Pointer to space for the context struct. This needs to be zero-initialized before
///   the first call to this function, for each new frame.
/// - `input`: Pointer to the input buffer
/// - `input_len`: Length of the input buffer, in bytes
/// - `output`: Pointer to the output buffer
/// - `output_len`: Length of the output buffer, in 16-bit words
///
/// Returns the number of *pixels* written to the output buffer. Note that this
/// doesn't accumulate over multiple calls. You'll need to keep track of the number of pixels
/// written and pass the correct output pointer to further calls.
///
/// # Safety
///
/// Behavior is undefined if:
/// - the concatenated input is not a valid Q565 image stream
/// - if the context is mutated between calls belonging to the same Q565 image stream
/// - if the output buffer is too small to fit the decoded image.
#[no_mangle]
pub unsafe extern "C" fn q565_streaming_decode_be(
    context: *mut Q565StreamingDecodeContext,
    input: *const u8,
    input_len: usize,
    output: *mut u16,
    output_len: usize,
) -> isize {
    let input = unsafe { core::slice::from_raw_parts(input, input_len) };
    let output = unsafe { core::slice::from_raw_parts_mut(output, output_len) };

    q565::decode::streaming_no_header::Q565StreamingDecodeContext::streaming_decode_to_slice_unchecked::<BigEndian>(
        &mut *context.cast::<q565::decode::streaming_no_header::Q565StreamingDecodeContext>(),
        input,
        output,
    ) as isize
}
