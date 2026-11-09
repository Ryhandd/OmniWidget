//! System Monitor & RAM Cleaner (TDD 4.8). `sysinfo` dibuat saat tab dibuka dan DILEPAS saat tab ditutup
//! (`sysmon_stop`), refresh selektif. RAM Cleaner sengaja jujur:
//!   A. Trim OmniWidget sendiri (aman)       -> `sysmon_trim_self`
//!   B. Tutup aplikasi pemakai memori besar  -> `sysmon_close` (UI wajib konfirmasi; paksa = konfirmasi kedua)
//! Purge standby list / drop_caches / `purge` TIDAK diimplementasikan (butuh admin & menurunkan performa).

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Serialize;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use tauri::{AppHandle, State};

#[derive(Default)]
pub struct SysState {
    sys: Mutex<Option<System>>,
}

/// Proses sistem/shell yang tidak boleh dipilih untuk ditutup (huruf kecil, dibandingkan tanpa ".exe").
const PROTECTED: &[&str] = &[
    "system", "idle", "registry", "smss", "csrss", "wininit", "winlogon", "services", "lsass", "svchost",
    "dwm", "explorer", "fontdrvhost", "sihost", "ctfmon", "taskhostw", "runtimebroker", "searchhost",
    "startmenuexperiencehost", "shellexperiencehost", "textinputhost", "securityhealthservice", "msmpeng",
    "omniwidget", "msedgewebview2",
    "systemd", "init", "xorg", "xwayland", "gnome-shell", "kwin_x11", "kwin_wayland", "plasmashell",
    "pipewire", "wireplumber", "dbus-daemon", "dbus-broker", "login", "sshd", "gdm", "sddm",
    "kernel_task", "launchd", "windowserver", "loginwindow", "finder", "dock", "systemuiserver",
];

pub fn is_protected(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    let n = n.strip_suffix(".exe").unwrap_or(&n);
    PROTECTED.contains(&n)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    cpu: f32,
    cores: usize,
    mem_used: u64,
    mem_total: u64,
    swap_used: u64,
    swap_total: u64,
    self_rss: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcGroup {
    name: String,
    pids: Vec<u32>,
    memory: u64,
    protected: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrimReport {
    before: u64,
    after: u64,
}

/// sysinfo dibuat malas (saat tab pertama kali meminta data).
fn ensure(g: &mut Option<System>) -> Result<&mut System, String> {
    if g.is_none() {
        *g = Some(System::new().map_err(|e| format!("Gagal membaca info sistem: {e}"))?);
    }
    Ok(g.as_mut().unwrap())
}

fn self_rss(sys: &mut System) -> u64 {
    let Ok(pid) = sysinfo::get_current_pid() else { return 0 };
    sys.refresh_processes_specifics(ProcessesToUpdate::Some(&[pid]), true, ProcessRefreshKind::nothing().with_memory());
    sys.process(pid).map(|p| p.memory()).unwrap_or(0)
}

/// Kembalikan heap proses ini ke OS.
fn release_heap() {
    #[cfg(target_os = "windows")]
    unsafe {
        use windows_sys::Win32::System::ProcessStatus::EmptyWorkingSet;
        use windows_sys::Win32::System::Threading::GetCurrentProcess;
        EmptyWorkingSet(GetCurrentProcess());
    }
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    unsafe {
        libc::malloc_trim(0);
    }
}

/// Kembalikan working set proses lain (mis. child WebView2) ke OS. Diam saja bila akses ditolak.
#[cfg(target_os = "windows")]
fn trim_process(pid: u32) {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::ProcessStatus::EmptyWorkingSet;
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_SET_QUOTA};
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_SET_QUOTA, 0, pid);
        if !h.is_null() {
            EmptyWorkingSet(h);
            CloseHandle(h);
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn trim_process(_pid: u32) {}

/// `pid` adalah turunan `root` (dibatasi kedalaman agar tidak ada loop parent yang aneh)?
fn is_descendant(sys: &System, pid: Pid, root: Pid) -> bool {
    let mut cur = pid;
    for _ in 0..16 {
        let Some(p) = sys.process(cur) else { return false };
        let Some(parent) = p.parent() else { return false };
        if parent == root {
            return true;
        }
        cur = parent;
    }
    false
}

/// Di atas ambang ini widget mulai mengembalikan memori ke OS. 75% cukup dini sebelum Windows
/// mulai menekan WebView2 (yang membuat jendela hilang).
const MEM_PRESSURE: f64 = 0.75;
const GUARD_TICK: std::time::Duration = std::time::Duration::from_secs(25);

/// Penjaga memori otomatis. Saat RAM sistem penuh: kosongkan working set proses inti + seluruh
/// child `msedgewebview2` milik OmniWidget, dan lepas cache thumbnail clipboard. Tidak menutup
/// aplikasi apa pun, jadi widget tetap hidup.
pub fn spawn_memory_guard(app: AppHandle) {
    std::thread::Builder::new()
        .name("memory-guard".into())
        .spawn(move || {
            let Ok(mut sys) = System::new() else { return };
            loop {
                std::thread::sleep(GUARD_TICK);
                sys.refresh_memory();
                let total = sys.total_memory();
                if total == 0 {
                    continue;
                }
                if (sys.used_memory() as f64 / total as f64) < MEM_PRESSURE {
                    continue;
                }
                sys.refresh_processes_specifics(
                    ProcessesToUpdate::All,
                    true,
                    ProcessRefreshKind::nothing().with_memory(),
                );
                let Ok(me) = sysinfo::get_current_pid() else { continue };
                release_heap();
                for (pid, p) in sys.processes() {
                    if *pid == me {
                        continue;
                    }
                    let name = p.name().to_string_lossy().to_ascii_lowercase();
                    if name.starts_with("msedgewebview2") && is_descendant(&sys, *pid, me) {
                        trim_process(pid.as_u32());
                    }
                }
                crate::plugins::clipboard::release_thumbnails(&app);
            }
        })
        .ok();
}

// ---------------------------------------------------------------- Commands

/// Satu sampel ringan (UI memanggil 1x/detik HANYA saat tab terlihat).
#[tauri::command]
pub fn sysmon_sample(state: State<'_, SysState>) -> Result<Sample, String> {
    let mut g = state.sys.lock().unwrap();
    let sys = ensure(&mut g)?;
    sys.refresh_cpu_usage();
    sys.refresh_memory();
    let rss = self_rss(sys);
    Ok(Sample {
        cpu: sys.global_cpu_usage(),
        cores: sys.cpus().len(),
        mem_used: sys.used_memory(),
        mem_total: sys.total_memory(),
        swap_used: sys.used_swap(),
        swap_total: sys.total_swap(),
        self_rss: rss,
    })
}

/// Aplikasi pemakai memori terbesar milik pengguna yang sama, dikelompokkan per nama (mis. semua chrome.exe).
#[tauri::command]
pub fn sysmon_top(state: State<'_, SysState>, limit: Option<usize>) -> Result<Vec<ProcGroup>, String> {
    let mut g = state.sys.lock().unwrap();
    let sys = ensure(&mut g)?;
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_memory().with_user(UpdateKind::OnlyIfNotSet),
    );
    let me = sysinfo::get_current_pid().ok();
    let my_user = me.and_then(|p| sys.process(p)).and_then(|p| p.user_id().cloned());
    let mut groups: HashMap<String, ProcGroup> = HashMap::new();
    for (pid, p) in sys.processes() {
        if Some(*pid) == me || p.thread_kind().is_some() {
            continue;
        }
        if let (Some(mine), Some(theirs)) = (&my_user, p.user_id()) {
            if mine != theirs {
                continue;
            }
        }
        let name = p.name().to_string_lossy().to_string();
        let e = groups.entry(name.clone()).or_insert_with(|| ProcGroup { protected: is_protected(&name), name, pids: vec![], memory: 0 });
        e.pids.push(pid.as_u32());
        e.memory += p.memory();
    }
    let mut v: Vec<ProcGroup> = groups.into_values().collect();
    v.sort_by(|a, b| b.memory.cmp(&a.memory));
    v.truncate(limit.unwrap_or(8).clamp(1, 30));
    Ok(v)
}

/// Tutup proses terpilih. Sopan dulu (Windows: taskkill tanpa /F = WM_CLOSE; Unix: SIGTERM); `force` = paksa.
/// Proses terproteksi, milik pengguna lain, atau OmniWidget sendiri selalu ditolak.
#[tauri::command]
pub fn sysmon_close(state: State<'_, SysState>, pids: Vec<u32>, force: bool) -> Result<usize, String> {
    let mut g = state.sys.lock().unwrap();
    let sys = ensure(&mut g)?;
    let wanted: Vec<Pid> = pids.iter().map(|p| Pid::from_u32(*p)).collect();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::Some(&wanted),
        true,
        ProcessRefreshKind::nothing().with_user(UpdateKind::OnlyIfNotSet),
    );
    let me = sysinfo::get_current_pid().ok();
    let my_user = me.and_then(|p| {
        sys.refresh_processes_specifics(ProcessesToUpdate::Some(&[p]), false, ProcessRefreshKind::nothing().with_user(UpdateKind::OnlyIfNotSet));
        sys.process(p).and_then(|x| x.user_id().cloned())
    });
    let mut closed = 0;
    for pid in wanted {
        let Some(p) = sys.process(pid) else { continue };
        if Some(pid) == me || is_protected(&p.name().to_string_lossy()) {
            return Err(format!("{} dilindungi dan tidak boleh ditutup", p.name().to_string_lossy()));
        }
        if let (Some(a), Some(b)) = (&my_user, p.user_id()) {
            if a != b {
                return Err("Hanya aplikasi milik pengguna ini yang boleh ditutup".into());
            }
        }
        let ok = if force {
            p.kill()
        } else {
            #[cfg(target_os = "windows")]
            {
                use std::os::windows::process::CommandExt;
                std::process::Command::new("taskkill")
                    .args(["/PID", &pid.as_u32().to_string()])
                    .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false)
            }
            #[cfg(not(target_os = "windows"))]
            {
                p.kill_with(sysinfo::Signal::Term).unwrap_or(false)
            }
        };
        if ok {
            closed += 1;
        }
    }
    Ok(closed)
}

/// Tingkat A: kosongkan cache OmniWidget + kembalikan heap ke OS. Laporan = RSS proses inti sebelum/sesudah.
#[tauri::command]
pub fn sysmon_trim_self(app: AppHandle, state: State<'_, SysState>) -> Result<TrimReport, String> {
    let mut g = state.sys.lock().unwrap();
    let before = self_rss(ensure(&mut g)?);
    crate::plugins::clipboard::release_thumbnails(&app);
    release_heap();
    std::thread::sleep(std::time::Duration::from_millis(150));
    let after = self_rss(ensure(&mut g)?);
    Ok(TrimReport { before, after })
}

/// Tab ditinggalkan: lepas seluruh struktur sysinfo (daftar proses bisa ratusan KB).
#[tauri::command]
pub fn sysmon_stop(state: State<'_, SysState>) {
    state.sys.lock().unwrap().take();
    release_heap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daftar_proteksi() {
        assert!(is_protected("explorer.exe"));
        assert!(is_protected("CSRSS.EXE"));
        assert!(is_protected("omniwidget"));
        assert!(!is_protected("chrome.exe"));
    }
}
