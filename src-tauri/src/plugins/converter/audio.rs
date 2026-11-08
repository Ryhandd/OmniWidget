//! Konversi audio tanpa biner eksternal: dekode dengan `symphonia`, tulis ulang dengan encoder
//! murni Rust (`mp3lame-encoder`, `flacenc`, `vorbis_rs`, `hound`).
//!
//! Semua hasil dinormalisasi agar laju sampelnya didukung oleh encoder target. Seluruh pekerjaan
//! berjalan di thread blocking pemanggil (`spawn_blocking`), jadi tidak ada runtime async di sini.

use super::{error::ConvError, formats::Format, CancelToken};
use std::{
    fs::File,
    io::ErrorKind,
    num::{NonZeroU32, NonZeroU8},
    path::Path,
    sync::atomic::Ordering,
};
use symphonia::{
    core::{
        audio::SampleBuffer,
        codecs::{DecoderOptions, CODEC_TYPE_NULL},
        errors::Error as SymError,
        formats::FormatOptions,
        io::MediaSourceStream,
        meta::MetadataOptions,
        probe::Hint,
    },
    default::{get_codecs, get_probe},
};

/// Laju sampel yang didukung LAME (dan aman untuk semua encoder lain).
const SUPPORTED_RATES: [u32; 9] = [8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000];

#[derive(Default)]
struct Pcm {
    channels: usize,
    sample_rate: u32,
    /// Satu vektor per kanal (planar), nilai ternormalisasi -1.0..=1.0.
    data: Vec<Vec<f32>>,
}

impl Pcm {
    fn frames(&self) -> usize {
        self.data.first().map_or(0, Vec::len)
    }
}

pub fn convert(
    input: &Path,
    output: &Path,
    to: Format,
    cancel: &CancelToken,
    progress: &dyn Fn(f32),
) -> Result<(), ConvError> {
    progress(0.02);
    let mut pcm = decode(input, cancel)?;
    if pcm.frames() == 0 {
        return Err(ConvError::Audio("Berkas audio tidak berisi sampel".into()));
    }
    progress(0.55);

    let target_rate = normalize_rate(pcm.sample_rate);
    resample(&mut pcm, target_rate);
    progress(0.65);

    match to {
        Format::Wav => write_wav(&pcm, output)?,
        Format::Mp3 => write_bytes(&pcm, output, encode_mp3)?,
        Format::Flac => write_bytes(&pcm, output, encode_flac)?,
        Format::Ogg => write_bytes(&pcm, output, encode_vorbis)?,
        _ => return Err(ConvError::NoRoute),
    }
    progress(1.0);
    Ok(())
}

// ---------- Dekode ----------

fn decode(input: &Path, cancel: &CancelToken) -> Result<Pcm, ConvError> {
    let file = File::open(input)?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = input.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| ConvError::Audio(format!("Tidak bisa membaca berkas audio: {e}")))?;
    let mut format = probed.format;

    let track = format
        .default_track()
        .filter(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| ConvError::Audio("Berkas ini tidak memiliki trek audio yang didukung".into()))?;
    let track_id = track.id;
    let mut decoder = get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| ConvError::Audio(format!("Kodek audio tidak didukung: {e}")))?;

    let mut pcm = Pcm::default();

    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(ConvError::Cancelled);
        }
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymError::IoError(e)) if e.kind() == ErrorKind::UnexpectedEof => break,
            Err(SymError::ResetRequired) => break,
            Err(SymError::IoError(e)) => return Err(ConvError::Io(e)),
            Err(e) => return Err(ConvError::Audio(format!("Gagal membaca paket audio: {e}"))),
        };
        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            // Satu paket rusak tidak membatalkan seluruh konversi.
            Err(SymError::DecodeError(_)) => continue,
            Err(e) => return Err(ConvError::Audio(format!("Gagal mendekode audio: {e}"))),
        };

        let spec = *decoded.spec();
        let channels = spec.channels.count();
        if channels == 0 {
            continue;
        }
        if pcm.channels == 0 {
            pcm.channels = channels;
            pcm.sample_rate = spec.rate;
            pcm.data = vec![Vec::new(); channels];
        }

        // Buffer baru per paket: kapasitas paket bisa berubah, dan `copy_interleaved_ref`
        // membutuhkan buffer yang cukup.
        let mut buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        buf.copy_interleaved_ref(decoded);

        if channels == pcm.channels {
            for frame in buf.samples().chunks_exact(channels) {
                for (c, sample) in frame.iter().enumerate() {
                    pcm.data[c].push(*sample);
                }
            }
        }
    }

    Ok(pcm)
}

// ---------- Resampling ----------

fn normalize_rate(rate: u32) -> u32 {
    if SUPPORTED_RATES.contains(&rate) {
        return rate;
    }
    // Laju eksotis (mis. 96000) diturunkan ke 44100 agar kompatibel dengan semua encoder.
    44100
}

/// Interpolasi linear sederhana. Cukup untuk konversi umum, tanpa dependensi tambahan.
fn resample(pcm: &mut Pcm, target: u32) {
    if pcm.sample_rate == target || pcm.frames() == 0 {
        pcm.sample_rate = target;
        return;
    }
    let in_len = pcm.frames();
    if in_len < 2 {
        pcm.sample_rate = target;
        return;
    }
    let ratio = pcm.sample_rate as f64 / target as f64;
    let out_len = ((in_len as f64) / ratio).round().max(1.0) as usize;

    for channel in pcm.data.iter_mut() {
        let mut out = Vec::with_capacity(out_len);
        for i in 0..out_len {
            let pos = i as f64 * ratio;
            let i0 = (pos.floor() as usize).min(in_len - 1);
            let i1 = (i0 + 1).min(in_len - 1);
            let frac = (pos - i0 as f64) as f32;
            out.push(channel[i0] * (1.0 - frac) + channel[i1] * frac);
        }
        *channel = out;
    }
    pcm.sample_rate = target;
}

// ---------- Encoder ----------

fn write_bytes(
    pcm: &Pcm,
    output: &Path,
    encode: fn(&Pcm) -> Result<Vec<u8>, ConvError>,
) -> Result<(), ConvError> {
    let bytes = encode(pcm)?;
    std::fs::write(output, bytes)?;
    Ok(())
}

fn to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16
}

/// LAME hanya mendukung mono/stereo. Kanal > 2 dirata-ratakan menjadi satu kanal.
fn mp3_channels(pcm: &Pcm) -> Result<(Option<Vec<f32>>, Option<Vec<f32>>, Option<Vec<f32>>), ConvError> {
    match pcm.channels {
        1 => Ok((Some(pcm.data[0].clone()), None, None)),
        2 => Ok((None, Some(pcm.data[0].clone()), Some(pcm.data[1].clone()))),
        n if n > 2 => {
            let mut mono = vec![0.0f32; pcm.frames()];
            for channel in &pcm.data {
                for (dst, src) in mono.iter_mut().zip(channel.iter()) {
                    *dst += *src / n as f32;
                }
            }
            Ok((Some(mono), None, None))
        }
        _ => Err(ConvError::Audio("Tidak ada kanal audio".into())),
    }
}

fn encode_mp3(pcm: &Pcm) -> Result<Vec<u8>, ConvError> {
    use mp3lame_encoder::{Builder, Bitrate, DualPcm, FlushNoGap, MonoPcm, Quality};

    let (mono, left, right) = mp3_channels(pcm)?;
    let channels = if mono.is_some() { 1u8 } else { 2u8 };

    let mut encoder = Builder::new()
        .ok_or_else(|| ConvError::Audio("Encoder MP3 tidak tersedia".into()))?
        .with_num_channels(channels)
        .and_then(|b| b.with_sample_rate(pcm.sample_rate))
        .and_then(|b| b.with_brate(Bitrate::Kbps192))
        .and_then(|b| b.with_quality(Quality::Best))
        .and_then(Builder::build)
        .map_err(|e| ConvError::Audio(format!("Pengaturan encoder MP3 tidak valid: {e}")))?;

    let mut out = Vec::new();

    // LAME menulis ke *spare capacity* Vec, jadi kapasitas harus disediakan sebelum setiap
    // pemanggilan. Ukuran aman menurut dokumentasi LAME: 1,25 * jumlah sampel + 7200.
    const CHUNK: usize = 1152 * 16;
    fn reserve_for(out: &mut Vec<u8>, samples: usize) {
        out.reserve(samples * 5 / 4 + 7200);
    }

    match (mono, left, right) {
        (Some(mono), _, _) => {
            for chunk in mono.chunks(CHUNK) {
                reserve_for(&mut out, chunk.len());
                encoder
                    .encode_to_vec(MonoPcm(chunk), &mut out)
                    .map_err(|e| ConvError::Audio(format!("Gagal mengodekan MP3: {e}")))?;
            }
        }
        (_, Some(left), Some(right)) => {
            for (l, r) in left.chunks(CHUNK).zip(right.chunks(CHUNK)) {
                reserve_for(&mut out, l.len());
                encoder
                    .encode_to_vec(DualPcm { left: l, right: r }, &mut out)
                    .map_err(|e| ConvError::Audio(format!("Gagal mengodekan MP3: {e}")))?;
            }
        }
        _ => return Err(ConvError::Audio("Tidak ada kanal audio".into())),
    }

    reserve_for(&mut out, 0);
    encoder
        .flush_to_vec::<FlushNoGap>(&mut out)
        .map_err(|e| ConvError::Audio(format!("Gagal menutup MP3: {e}")))?;
    Ok(out)
}

fn encode_flac(pcm: &Pcm) -> Result<Vec<u8>, ConvError> {
    use flacenc::{component::BitRepr, error::Verify};

    let mut interleaved = Vec::with_capacity(pcm.frames() * pcm.channels);
    for frame in 0..pcm.frames() {
        for channel in &pcm.data {
            interleaved.push(to_i16(channel[frame]) as i32);
        }
    }

    let config = flacenc::config::Encoder::default()
        .into_verified()
        .map_err(|_| ConvError::Audio("Pengaturan encoder FLAC tidak valid".into()))?;
    let source = flacenc::source::MemSource::from_samples(
        &interleaved,
        pcm.channels,
        16,
        pcm.sample_rate as usize,
    );
    let stream = flacenc::encode_with_fixed_block_size(&config, source, config.block_size)
        .map_err(|e| ConvError::Audio(format!("Gagal mengodekan FLAC: {e}")))?;

    let mut sink = flacenc::bitsink::ByteSink::new();
    stream
        .write(&mut sink)
        .map_err(|e| ConvError::Audio(format!("Gagal menulis FLAC: {e}")))?;
    Ok(sink.into_inner())
}

fn encode_vorbis(pcm: &Pcm) -> Result<Vec<u8>, ConvError> {
    use vorbis_rs::VorbisEncoderBuilder;
    let mut sink = Vec::new();
    {
        let rate = NonZeroU32::new(pcm.sample_rate)
            .ok_or_else(|| ConvError::Audio("Laju sampel tidak valid".into()))?;
        let channels = NonZeroU8::new(pcm.channels as u8)
            .ok_or_else(|| ConvError::Audio("Jumlah kanal tidak valid".into()))?;
        let mut encoder = VorbisEncoderBuilder::new(rate, channels, &mut sink)
            .map_err(|e| ConvError::Audio(format!("Gagal membuat encoder OGG: {e}")))?
            .build()
            .map_err(|e| ConvError::Audio(format!("Gagal membuat encoder OGG: {e}")))?;
        encoder
            .encode_audio_block(&pcm.data)
            .map_err(|e| ConvError::Audio(format!("Gagal mengodekan OGG: {e}")))?;
        encoder
            .finish()
            .map_err(|e| ConvError::Audio(format!("Gagal menutup OGG: {e}")))?;
    }
    Ok(sink)
}

fn write_wav(pcm: &Pcm, output: &Path) -> Result<(), ConvError> {
    let spec = hound::WavSpec {
        channels: pcm.channels as u16,
        sample_rate: pcm.sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(output, spec)
        .map_err(|e| ConvError::Audio(format!("Gagal membuat WAV: {e}")))?;
    for frame in 0..pcm.frames() {
        for channel in &pcm.data {
            writer
                .write_sample(to_i16(channel[frame]))
                .map_err(|e| ConvError::Audio(format!("Gagal menulis WAV: {e}")))?;
        }
    }
    writer
        .finalize()
        .map_err(|e| ConvError::Audio(format!("Gagal menutup WAV: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nada sinus 0,1 detik mono 44,1 kHz: cukup panjang agar encoder mengeluarkan frame.
    fn nada() -> Pcm {
        let frames = 4410;
        let data = vec![(0..frames)
            .map(|i| (i as f32 * 0.05).sin() * 0.5)
            .collect::<Vec<f32>>()];
        Pcm {
            channels: 1,
            sample_rate: 44100,
            data,
        }
    }

    #[test]
    fn laju_eksotis_dinormalisasi() {
        assert_eq!(normalize_rate(96000), 44100);
        assert_eq!(normalize_rate(48000), 48000);
        assert_eq!(normalize_rate(8000), 8000);
    }

    #[test]
    fn resample_mengubah_panjang_sesuai_rasio() {
        let mut pcm = nada();
        resample(&mut pcm, 22050);
        assert_eq!(pcm.sample_rate, 22050);
        let frame = pcm.frames() as i64;
        assert!((frame - 2205).abs() <= 2, "frames={frame}");
    }

    #[test]
    fn semua_encoder_menghasilkan_berkas_dengan_magic_benar() {
        let pcm = nada();

        let mp3 = encode_mp3(&pcm).expect("MP3");
        assert!(mp3.len() > 100, "mp3 terlalu pendek: {}", mp3.len());
        // Setiap frame MPEG diawali sync word 11 bit.
        assert!(mp3.windows(2).any(|w| w[0] == 0xFF && (w[1] & 0xE0) == 0xE0));

        let flac = encode_flac(&pcm).expect("FLAC");
        assert_eq!(&flac[..4], b"fLaC");

        let ogg = encode_vorbis(&pcm).expect("OGG");
        assert_eq!(&ogg[..4], b"OggS");

        let dir = tempfile::tempdir().unwrap();
        let wav_path = dir.path().join("nada.wav");
        write_wav(&pcm, &wav_path).expect("WAV");
        let wav = std::fs::read(&wav_path).unwrap();
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
    }

    #[test]
    fn kanal_lebih_dari_dua_diturunkan_ke_mono_untuk_mp3() {
        let mut pcm = nada();
        pcm.channels = 4;
        pcm.data = vec![pcm.data[0].clone(); 4];
        let (mono, left, right) = mp3_channels(&pcm).unwrap();
        assert!(mono.is_some() && left.is_none() && right.is_none());
        assert_eq!(mono.unwrap().len(), pcm.frames());
    }
}
