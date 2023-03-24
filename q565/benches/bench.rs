use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use q565::utils::LittleEndian;

fn decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("test_images");

    for image in std::fs::read_dir("test_images").unwrap() {
        let image_path = image.unwrap().path();

        let mut reader = png::Decoder::new(std::fs::File::open(&image_path).unwrap())
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

        let image_name = image_path.file_name().unwrap().to_string_lossy();

        let mut encoded = Vec::with_capacity(pixel_count * 2);
        assert!(q565::alloc_api::encode_to_vec(
            width as u16,
            height as u16,
            &input,
            &mut encoded
        ));

        group.throughput(criterion::Throughput::Elements(pixel_count as u64));
        group.bench_with_input(
            BenchmarkId::new("unsafe", &image_name),
            &encoded,
            |b, input| {
                let mut output = vec![0; pixel_count];
                let mut state = q565::Q565Context::new();
                b.iter(|| unsafe {
                    q565::decode_to_slice_unchecked::<LittleEndian>(&mut state, input, &mut output)
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("streaming_no_header", &image_name),
            &encoded,
            |b, input| {
                let input = &input[8..];
                let mut streaming_decoded = vec![0; pixel_count];
                b.iter(|| {
                    let mut state = q565::streaming_no_header::Q565StreamingDecodeContext::new();
                    let mut streaming_output_buf = &mut streaming_decoded[..];
                    for chunk in input.chunks(512) {
                        let pixels_written = unsafe {
                            q565::streaming_no_header::streaming_decode_to_slice_unchecked::<
                                LittleEndian,
                            >(&mut state, chunk, streaming_output_buf)
                        };
                        streaming_output_buf = &mut streaming_output_buf[pixels_written..];
                    }
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("safe", &image_name),
            &encoded,
            |b, input| {
                let mut output = Vec::with_capacity(pixel_count);
                b.iter(|| {
                    output.clear();
                    q565::alloc_api::decode_to_vec::<LittleEndian>(input, &mut output)
                })
            },
        );
    }
}

fn encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("test_images");

    for image in std::fs::read_dir("test_images").unwrap() {
        let image_path = image.unwrap().path();

        let mut reader = png::Decoder::new(std::fs::File::open(&image_path).unwrap())
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

        let image_name = image_path.file_name().unwrap().to_string_lossy();

        group.throughput(criterion::Throughput::Elements(pixel_count as u64));

        group.bench_with_input(
            BenchmarkId::new("encode", &image_name),
            &input,
            |b, input| {
                let mut encoded = Vec::with_capacity(pixel_count * 2);
                b.iter(|| {
                    encoded.clear();
                    q565::alloc_api::encode_to_vec(
                        width as u16,
                        height as u16,
                        &input,
                        &mut encoded,
                    )
                })
            },
        );
    }
}

criterion_group!(benches, decode, encode);
criterion_main!(benches);
