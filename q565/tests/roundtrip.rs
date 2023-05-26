use image::ImageFormat;
use q565::{byteorder::LittleEndian, utils::rgb888_to_rgb565, Rgb565};
use std::io::BufReader;

#[test]
fn roundtrip() {
    for image in std::fs::read_dir("../test_images").unwrap() {
        let image_path = image.unwrap().path();

        let image = image::load(
            BufReader::new(std::fs::File::open(&image_path).unwrap()),
            ImageFormat::Png,
        )
        .unwrap();

        let width = image.width() as usize;
        let height = image.height() as usize;
        let pixel_count = width * height;

        let mut input = Vec::with_capacity(pixel_count);
        input.extend(image.into_rgb8().pixels().map(|p| rgb888_to_rgb565(p.0)));

        let mut encoded = Vec::with_capacity(pixel_count * 2);
        assert!(q565::encode::Q565EncodeContext::encode_to_vec(
            width as u16,
            height as u16,
            &input,
            &mut encoded
        ));

        let mut encoded2 = Vec::with_capacity(pixel_count * 2);
        q565::encode::Q565EncodeContext::encode(width as u16, height as u16, &input, &mut encoded2)
            .unwrap();

        assert_eq!(encoded, encoded2, "encoding mismatch");

        let mut decoded_to_vec = Vec::with_capacity(pixel_count);
        let decoded_to_vec_output =
            q565::decode::VecDecodeOutput::<Rgb565>::new(&mut decoded_to_vec);
        q565::decode::Q565DecodeContext::decode::<LittleEndian>(&encoded, decoded_to_vec_output)
            .unwrap();
        assert_eq!(input, decoded_to_vec, "safe decoding failed");

        let mut unsafe_decoded_to_slice = vec![0u16; pixel_count];
        unsafe {
            let unsafe_decoded_to_slice_output =
                q565::decode::UnsafeSliceDecodeOutput::<Rgb565>::new(&mut unsafe_decoded_to_slice);
            q565::decode::Q565DecodeContext::decode_unchecked::<LittleEndian>(
                &encoded,
                unsafe_decoded_to_slice_output,
            )
            .unwrap()
        };
        assert_eq!(input, unsafe_decoded_to_slice, "unsafe decoding failed");

        let mut streaming_decoded = vec![0; pixel_count];
        let mut state = q565::decode::streaming_no_header::Q565StreamingDecodeContext::new();
        let mut streaming_output_buf = &mut streaming_decoded[..];
        for chunk in encoded[8..].chunks(512) {
            let pixels_written = unsafe {
                state.streaming_decode_to_slice_unchecked::<LittleEndian>(
                    chunk,
                    streaming_output_buf,
                )
            };
            streaming_output_buf = &mut streaming_output_buf[pixels_written..];
        }
        assert_eq!(
            input, streaming_decoded,
            "streaming_no_header decoding failed"
        );
    }
}
