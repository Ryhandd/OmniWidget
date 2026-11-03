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
            // JPEG tidak punya alpha: gabungkan ke latar putih (bukan hitam) bila gambar transparan.
            let rgb = if img.color().has_alpha() { flatten_on_white(&img) } else { img.to_rgb8() };
            let enc = JpegEncoder::new_with_quality(BufWriter::new(File::create(output)?), 90);
            DynamicImage::ImageRgb8(rgb).write_with_encoder(enc)?;
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

/// Alpha-blend ke latar putih. Hanya dipanggil untuk gambar ber-alpha (salinan RGBA sementara).
fn flatten_on_white(img: &DynamicImage) -> image::RgbImage {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut out = image::RgbImage::new(w, h);
    for (x, y, p) in rgba.enumerate_pixels() {
        let a = p[3] as u32;
        let mix = |c: u8| (((c as u32 * a) + 255 * (255 - a) + 127) / 255) as u8;
        out.put_pixel(x, y, image::Rgb([mix(p[0]), mix(p[1]), mix(p[2])]));
    }
    out
}
