use argh::FromArgs;
use image::{ImageFormat, RgbImage};
use q565::utils::{rgb565_to_rgb888, rgb888_to_rgb565, LittleEndian};
use std::{fs::File, io::BufReader, num::NonZeroU16, str::FromStr};

/// Q565 cli encoder and decoder.
#[derive(FromArgs)]
struct Cli {
    #[argh(subcommand)]
    command: Command,
}

#[derive(FromArgs)]
#[argh(subcommand)]
enum Command {
    Encode(Encode),
    EncodeRaw(EncodeRaw),
    Decode(Decode),
    DecodeRaw(DecodeRaw),
}

#[derive(Debug)]
enum Format {
    Png,
    Jpg,
    Bmp,
}

impl FromStr for Format {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        #[rustfmt::skip]
        let Some(format) = s.eq_ignore_ascii_case("png").then_some(Format::Png)
               .or_else(|| s.eq_ignore_ascii_case("jpg").then_some(Format::Jpg))
               .or_else(|| s.eq_ignore_ascii_case("bmp").then_some(Format::Bmp))
        else { return Err("invalid string"); };

        Ok(format)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Cli { command } = argh::from_env();

    match command {
        Command::Encode(options) => encode(options),
        Command::EncodeRaw(options) => encode_raw(options),
        Command::Decode(options) => decode(options),
        Command::DecodeRaw(options) => decode_raw(options),
    }
}

/// Encodes an image as Q565.
#[derive(FromArgs)]
#[argh(subcommand, name = "encode")]
struct Encode {
    /// input format, optional (png, jpg, bmp)
    #[argh(option)]
    format: Option<Format>,

    /// the input file. If none of the raw flags are set, this may be a PNG, JPG, or BMP.
    #[argh(positional)]
    input: String,
    /// the output file
    #[argh(positional)]
    output: String,
}

fn encode(options: Encode) -> Result<(), Box<dyn std::error::Error>> {
    let Encode {
        format,
        input,
        output,
    } = options;

    let image = match format {
        Some(Format::Png) => {
            image::io::Reader::with_format(BufReader::new(File::open(&input)?), ImageFormat::Png)
                .decode()?
        }
        Some(Format::Jpg) => {
            image::io::Reader::with_format(BufReader::new(File::open(&input)?), ImageFormat::Jpeg)
                .decode()?
        }
        Some(Format::Bmp) => {
            image::io::Reader::with_format(BufReader::new(File::open(&input)?), ImageFormat::Bmp)
                .decode()?
        }
        None => image::io::Reader::open(input)?
            .with_guessed_format()?
            .decode()?,
    };

    let width = image.width();
    let height = image.height();

    println!("Encoding {width}x{height} image");

    if width > u16::MAX as u32 || height > u16::MAX as u32 {
        return Err("image dimensions are too large".into());
    }

    let rgb565_raw = image
        .into_rgb8()
        .pixels()
        .map(|p| rgb888_to_rgb565(p.0))
        .collect::<Vec<_>>();

    let mut v = Vec::with_capacity(1024 * 1024);
    assert!(q565::alloc_api::encode_to_vec(
        width as u16,
        height as u16,
        &rgb565_raw,
        &mut v
    ));

    std::fs::write(&output, &v)?;
    println!("Written {} bytes to `{output}`", v.len());

    Ok(())
}

/// Encodes a raw RGB565LE image as Q565.
#[derive(FromArgs)]
#[argh(subcommand, name = "encode-raw")]
struct EncodeRaw {
    /// image width
    #[argh(option)]
    width: NonZeroU16,
    /// image height
    #[argh(option)]
    height: NonZeroU16,

    /// the input file. If none of the raw flags are set, this may be a PNG, JPG, or BMP.
    #[argh(positional)]
    input: String,
    /// the output file
    #[argh(positional)]
    output: String,
}

fn encode_raw(options: EncodeRaw) -> Result<(), Box<dyn std::error::Error>> {
    let EncodeRaw {
        width,
        height,
        input,
        output,
    } = options;

    println!("Encoding {width}x{height} image");

    let rgb565_raw = std::fs::read(input)?;
    let rgb565_raw: Vec<_> = rgb565_raw
        .chunks_exact(2)
        .map(|c| {
            let &[a, b] = c else { unreachable!() };

            u16::from_le_bytes([a, b])
        })
        .collect();

    let expected_size = width.get() as usize * height.get() as usize;
    if rgb565_raw.len() != expected_size {
        return Err(format!(
            "input file size is not correct, expected {} bytes, got {}",
            expected_size,
            rgb565_raw.len()
        )
        .into());
    }

    let mut v = Vec::with_capacity(1024 * 1024);

    assert!(q565::alloc_api::encode_to_vec(
        width.get(),
        height.get(),
        &rgb565_raw,
        &mut v
    ));

    std::fs::write(&output, &v)?;
    println!("Written {} bytes to `{output}`", v.len());

    Ok(())
}

/// Decodes a Q565 image into a raw RGB565LE image.
#[derive(FromArgs)]
#[argh(subcommand, name = "decode")]
struct Decode {
    /// output format (png, jpg, bmp)
    #[argh(option)]
    format: Format,

    /// the input file. If none of the raw flags are set, this may be a PNG, JPG, or BMP.
    #[argh(positional)]
    input: String,
    /// the output file
    #[argh(positional)]
    output: String,
}

fn decode(options: Decode) -> Result<(), Box<dyn std::error::Error>> {
    let Decode {
        format,
        input,
        output,
    } = options;

    let q565_input = std::fs::read(&input)?;

    println!("Decoding `{input}`");

    let mut v = Vec::with_capacity(1024 * 1024);
    let q565::alloc_api::Header { width, height } =
        q565::alloc_api::decode_to_vec::<LittleEndian>(&q565_input, &mut v)
            .map_err(|e| format!("{e:?}"))?;

    let mut rgb888_raw = Vec::with_capacity(usize::from(width) * usize::from(height) * 3);
    for pixel888 in v.into_iter().map(rgb565_to_rgb888) {
        rgb888_raw.extend_from_slice(&pixel888);
    }

    RgbImage::from_vec(width as u32, height as u32, rgb888_raw)
        .ok_or("failed to create image")?
        .save_with_format(
            &output,
            match format {
                Format::Png => ImageFormat::Png,
                Format::Jpg => ImageFormat::Jpeg,
                Format::Bmp => ImageFormat::Bmp,
            },
        )?;

    println!("Written {width}x{height} image to `{output}`");

    Ok(())
}

/// Decodes a Q565 image.
#[derive(FromArgs)]
#[argh(subcommand, name = "decode-raw")]
struct DecodeRaw {
    /// the input file. If none of the raw flags are set, this may be a PNG, JPG, or BMP.
    #[argh(positional)]
    input: String,
    /// the output file
    #[argh(positional)]
    output: String,
}

fn decode_raw(options: DecodeRaw) -> Result<(), Box<dyn std::error::Error>> {
    let DecodeRaw { input, output } = options;

    let q565_input = std::fs::read(&input)?;

    println!("Decoding `{input}`");

    let mut v = Vec::with_capacity(1024 * 1024);
    let q565::alloc_api::Header { width, height } =
        q565::alloc_api::decode_to_vec::<LittleEndian>(&q565_input, &mut v)
            .map_err(|e| format!("{e:?}"))?;

    let bytes = unsafe { std::slice::from_raw_parts(v.as_ptr().cast::<u8>(), v.len() * 2) };
    std::fs::write(&output, bytes)?;

    println!("Written {width}x{height} image to `{output}`");

    Ok(())
}
