use super::{error::ConvError, formats::Format, CancelToken};
use image::{codecs::jpeg::JpegEncoder, imageops::FilterType, DynamicImage, ImageFormat, ImageReader};
use std::{fs::File, io::BufWriter, path::Path, sync::atomic::Ordering};

const MAX_MEGAPIXELS: u64 = 100;

pub fn convert(input: &Path, output: &Path, to: Format, cancel: &CancelToken, progress: &dyn Fn(f32)) -> Result<(), ConvError> {
    let (w, h) = ImageReader::open(input)?.with_guessed_format()?.into_dimensions()?;
    let mp = (w as u64 * h as u64) / 1_000_000;
    if mp > MAX_MEGAPIXELS { return Err(ConvError::PixelBomb(mp, MAX_MEGAPIXELS)); }
    progress(0.15);

    let img = ImageReader::open(input)?.with_guessed_format()?.decode()?;
    if cancel.load(Ordering::Relaxed) { return Err(ConvError::Cancelled); }
    progress(0.6);

    match to {
        Format::Jpg => {
            let enc = JpegEncoder::new_with_quality(BufWriter::new(File::create(output)?), 90);
            DynamicImage::ImageRgb8(img.to_rgb8()).write_with_encoder(enc)?;
        }
        Format::Ico => {
            let img = if img.width() > 256 || img.height() > 256 { img.resize(256, 256, FilterType::Lanczos3) } else { img };
            img.save_with_format(output, ImageFormat::Ico)?;
        }
        Format::Png => img.save_with_format(output, ImageFormat::Png)?,
        Format::Webp => img.save_with_format(output, ImageFormat::WebP)?,
        Format::Bmp => img.save_with_format(output, ImageFormat::Bmp)?,
        Format::Gif => img.save_with_format(output, ImageFormat::Gif)?,
        _ => return Err(ConvError::NoRoute),
    }
    progress(1.0);
    Ok(())
}
