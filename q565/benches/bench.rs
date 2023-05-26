use byteorder::LittleEndian;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use image::ImageFormat;
use q565::{utils::rgb888_to_rgb565, Rgb565, Rgb888};
use std::io::BufReader;

fn decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("test_images decode");

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

        let image_name = image_path.file_name().unwrap().to_string_lossy();

        let mut encoded = Vec::with_capacity(pixel_count * 2);
        assert!(q565::encode::Q565EncodeContext::encode_to_vec(
            width as u16,
            height as u16,
            &input,
            &mut encoded
        ));

        group.throughput(criterion::Throughput::Elements(pixel_count as u64));
        group.bench_with_input(
            BenchmarkId::new("unsafe rgb565", &image_name),
            &encoded,
            |b, input| {
                let mut output = vec![0; pixel_count];
                b.iter(|| unsafe {
                    q565::decode::Q565DecodeContext::decode_unchecked::<LittleEndian>(
                        input,
                        q565::decode::UnsafeSliceDecodeOutput::<Rgb565>::new(&mut output),
                    )
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("unsafe rgb888", &image_name),
            &encoded,
            |b, input| {
                let mut output = vec![[0; 3]; pixel_count];
                b.iter(|| unsafe {
                    q565::decode::Q565DecodeContext::decode_unchecked::<LittleEndian>(
                        input,
                        q565::decode::UnsafeSliceDecodeOutput::<Rgb888>::new(&mut output),
                    )
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("safe rgb565", &image_name),
            &encoded,
            |b, input| {
                let mut output = Vec::with_capacity(pixel_count);
                b.iter(|| {
                    q565::decode::Q565DecodeContext::decode::<LittleEndian>(
                        input,
                        q565::decode::VecDecodeOutput::<Rgb565>::new(&mut output),
                    )
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("safe rgb888", &image_name),
            &encoded,
            |b, input| {
                let mut output = Vec::with_capacity(pixel_count);
                b.iter(|| {
                    q565::decode::Q565DecodeContext::decode::<LittleEndian>(
                        input,
                        q565::decode::VecDecodeOutput::<Rgb888>::new(&mut output),
                    )
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
                    let mut state =
                        q565::decode::streaming_no_header::Q565StreamingDecodeContext::new();
                    let mut streaming_output_buf = &mut streaming_decoded[..];
                    for chunk in input.chunks(512) {
                        let pixels_written = unsafe {
                            state.streaming_decode_to_slice_unchecked::<LittleEndian>(
                                chunk,
                                streaming_output_buf,
                            )
                        };
                        streaming_output_buf = &mut streaming_output_buf[pixels_written..];
                    }
                })
            },
        );
    }
}

fn encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("test_images encode");

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

        let image_name = image_path.file_name().unwrap().to_string_lossy();

        group.throughput(criterion::Throughput::Elements(pixel_count as u64));

        group.bench_with_input(
            BenchmarkId::new("encode_to_vec", &image_name),
            &input,
            |b, input| {
                let mut encoded = Vec::with_capacity(pixel_count * 2);
                b.iter(|| {
                    encoded.clear();
                    q565::encode::Q565EncodeContext::encode_to_vec(
                        width as u16,
                        height as u16,
                        input,
                        &mut encoded,
                    )
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("encode_std", &image_name),
            &input,
            |b, input| {
                let mut encoded = Vec::with_capacity(pixel_count * 2);
                b.iter(|| {
                    encoded.clear();
                    q565::encode::Q565EncodeContext::encode(
                        width as u16,
                        height as u16,
                        input,
                        &mut encoded,
                    )
                })
            },
        );
    }
}

criterion_group!(benches, decode, encode);
criterion_main!(benches);
