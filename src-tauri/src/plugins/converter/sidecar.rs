use super::{error::ConvError, formats::{Engine, Format}, CancelToken};
use std::{
    ffi::OsString, io::Read, path::{Path, PathBuf}, process::{Command, Stdio},
    sync::atomic::Ordering, thread, time::{Duration, Instant},
};

pub fn resolve(app_data: &Path, name: &'static str) -> Option<PathBuf> {
    let exe = if cfg!(windows) { format!("{name}.exe") } else { name.to_string() };
    let local = app_data.join("sidecars").join(name).join(&exe);
    if local.is_file() { return Some(local); }
    let flat = app_data.join("sidecars").join(&exe);
    if flat.is_file() { return Some(flat); }
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).map(|d| d.join(&exe)).find(|p| p.is_file())
    })
}

pub fn convert(engine: Engine, app_data: &Path, input: &Path, output: &Path, from: Format, to: Format, cancel: &CancelToken, progress: &dyn Fn(f32)) -> Result<(), ConvError> {
    let name = match engine {
        Engine::Ffmpeg => "ffmpeg",
        Engine::Pandoc => "pandoc",
        _ => return Err(ConvError::NoRoute),
    };
    let bin = resolve(app_data, name).ok_or(ConvError::SidecarMissing(name))?;
    let args = match engine {
        Engine::Ffmpeg => ffmpeg_args(input, output, to),
        Engine::Pandoc => pandoc_args(input, output, from, to),
        _ => unreachable!(),
    };
    progress(0.1);
    run(&bin, name, &args, cancel, Duration::from_secs(600))?;
    progress(1.0);
    Ok(())
}

fn ffmpeg_args(i: &Path, o: &Path, to: Format) -> Vec<OsString> {
    let mut a: Vec<OsString> = ["-nostdin", "-y", "-loglevel", "error", "-i"].map(Into::into).to_vec();
    a.push(i.into());
    let tail: &[&str] = match to {
        Format::Mp3  => &["-vn", "-c:a", "libmp3lame", "-q:a", "2", "-f", "mp3"],
        Format::Wav  => &["-vn", "-c:a", "pcm_s16le", "-f", "wav"],
        Format::Flac => &["-vn", "-c:a", "flac", "-f", "flac"],
        Format::Ogg  => &["-vn", "-c:a", "libvorbis", "-q:a", "5", "-f", "ogg"],
        Format::M4a  => &["-vn", "-c:a", "aac", "-b:a", "192k", "-f", "ipod"],
        Format::Opus => &["-vn", "-c:a", "libopus", "-b:a", "128k", "-f", "opus"],
        _ => &[],
    };
    a.extend(tail.iter().map(OsString::from));
    a.push(o.into());
    a
}

fn pandoc_args(i: &Path, o: &Path, from: Format, to: Format) -> Vec<OsString> {
    let r = match from { Format::Docx => "docx", Format::Md => "gfm", Format::Html => "html", _ => "markdown" };
    let w = match to { Format::Docx => "docx", Format::Md => "gfm", Format::Html => "html5", Format::Txt => "plain", _ => "plain" };
    let mut a: Vec<OsString> = vec![i.into(), "-f".into(), r.into(), "-t".into(), w.into()];
    if to == Format::Html { a.push("-s".into()); }
    a.push("-o".into());
    a.push(o.into());
    a
}

fn run(bin: &Path, name: &'static str, args: &[OsString], cancel: &CancelToken, timeout: Duration) -> Result<(), ConvError> {
    let mut child = Command::new(bin).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn()?;
    let mut err = child.stderr.take();
    let reader = thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(e) = err.as_mut() { let _ = e.read_to_end(&mut buf); }
        let start = buf.len().saturating_sub(2048);
        String::from_utf8_lossy(&buf[start..]).trim().to_string()
    });
    let t0 = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            let msg = reader.join().unwrap_or_default();
            return if status.success() { Ok(()) } else { Err(ConvError::SidecarFailed(name, msg)) };
        }
        if cancel.load(Ordering::Relaxed) { let _ = child.kill(); let _ = child.wait(); return Err(ConvError::Cancelled); }
        if t0.elapsed() > timeout { let _ = child.kill(); let _ = child.wait(); return Err(ConvError::Timeout(timeout.as_secs())); }
        thread::sleep(Duration::from_millis(100));
    }
}
