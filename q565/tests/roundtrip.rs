use q565::utils::LittleEndian;

#[test]
fn roundtrip() {
    for image in std::fs::read_dir("test_images").unwrap() {
        let image_path = image.unwrap().path();

        let mut reader = png::Decoder::new(std::fs::File::open(image_path).unwrap())
            .read_info()
            .unwrap();
        let width = reader.info().width as usize;
        let height = reader.info().height as usize;
        let bpp = reader.info().bytes_per_pixel();

        let mut raw_input = vec![0; reader.output_buffer_size()];
        reader.next_frame(&mut raw_input).unwrap();
        drop(reader);

        let pixel_count = width * height;

        let mut input = Vec::with_capacity(pixel_count);
        input.extend(raw_input.chunks(bpp).map(|p| {
            let &[r, g, b, ..] = p else { panic!("no rgb subpixels?") };
            let r = r as u16 >> 3;
            let g = g as u16 >> 2;
            let b = b as u16 >> 3;

            (r << 11) | (g << 5) | b
        }));

        let mut encoded = Vec::with_capacity(pixel_count * 2);
        assert!(q565::alloc_api::encode_to_vec(
            width as u16,
            height as u16,
            &input,
            &mut encoded
        ));

        let mut safe_decoded = Vec::with_capacity(pixel_count);
        q565::alloc_api::decode_to_vec::<LittleEndian>(&encoded, &mut safe_decoded).unwrap();
        assert_eq!(input, safe_decoded, "safe decoding failed");

        let mut unsafe_decoded = vec![0; pixel_count];
        let mut state = q565::Q565Context::new();
        unsafe {
            q565::decode_to_slice_unchecked::<LittleEndian>(
                &mut state,
                &encoded,
                &mut unsafe_decoded,
            )
        }
        .unwrap();
        assert_eq!(input, unsafe_decoded, "unsafe decoding failed");

        let mut streaming_decoded = vec![0; pixel_count];
        let mut state = q565::streaming_no_header::Q565StreamingDecodeContext::new();
        let mut streaming_output_buf = &mut streaming_decoded[..];
        for chunk in encoded[8..].chunks(512) {
            let pixels_written = unsafe {
                q565::streaming_no_header::streaming_decode_to_slice_unchecked::<LittleEndian>(
                    &mut state,
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
