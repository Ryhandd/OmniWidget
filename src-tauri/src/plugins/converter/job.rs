//! Registri job konversi: id unik, token pembatalan, dan antrean dengan konkurensi terbatas
//! (default 1, sesuai TDD 4.1). State ini kecil dan hidup di Rust, bukan di WebView.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
};
use tokio::sync::{Notify, Semaphore};

/// Dibaca oleh kode blocking (image/audio/dokumen/PDF) di thread worker.
pub type CancelToken = Arc<AtomicBool>;

/// Konkurensi default. Konversi menyentuh CPU/disk berat; satu per satu menjaga RAM tetap kecil.
pub const DEFAULT_CONCURRENCY: usize = 1;

#[derive(Clone)]
pub struct JobHandle {
    pub id: String,
    pub cancel: CancelToken,
    wake: Arc<Notify>,
}

impl JobHandle {
    /// Menunggu hingga job dibatalkan (dipakai saat job masih antre).
    pub async fn cancelled(&self) {
        if self.cancel.load(Ordering::SeqCst) {
            return;
        }
        // `notify_one` menyimpan permit bila belum ada penunggu, jadi tidak ada wakeup yang hilang.
        self.wake.notified().await;
    }
}

pub struct JobRegistry {
    jobs: Mutex<HashMap<String, JobHandle>>,
    gate: Arc<Semaphore>,
    seq: AtomicU64,
}

impl JobRegistry {
    pub fn new(concurrency: usize) -> Self {
        Self {
            jobs: Mutex::new(HashMap::new()),
            gate: Arc::new(Semaphore::new(concurrency.max(1))),
            seq: AtomicU64::new(1),
        }
    }

    pub fn gate(&self) -> Arc<Semaphore> {
        self.gate.clone()
    }

    pub fn register(&self) -> JobHandle {
        let id = format!("job-{}", self.seq.fetch_add(1, Ordering::Relaxed));
        let handle = JobHandle {
            id: id.clone(),
            cancel: Arc::new(AtomicBool::new(false)),
            wake: Arc::new(Notify::new()),
        };
        self.jobs.lock().unwrap().insert(id, handle.clone());
        handle
    }

    /// Minta job berhenti. `false` bila id tidak dikenal (sudah selesai).
    pub fn cancel(&self, id: &str) -> bool {
        let handle = self.jobs.lock().unwrap().get(id).cloned();
        match handle {
            Some(h) => {
                h.cancel.store(true, Ordering::SeqCst);
                h.wake.notify_one();
                true
            }
            None => false,
        }
    }

    pub fn finish(&self, id: &str) {
        self.jobs.lock().unwrap().remove(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_unik_dan_cancel_menyalakan_token() {
        let r = JobRegistry::new(1);
        let a = r.register();
        let b = r.register();
        assert_ne!(a.id, b.id);
        assert!(!a.cancel.load(Ordering::SeqCst));
        assert!(r.cancel(&a.id));
        assert!(a.cancel.load(Ordering::SeqCst));
        assert!(!b.cancel.load(Ordering::SeqCst));
        r.finish(&a.id);
        assert!(!r.cancel(&a.id), "job selesai tidak dapat dibatalkan lagi");
        assert!(!r.cancel("tidak-ada"));
    }

    #[tokio::test]
    async fn pembatalan_membangunkan_job_yang_antre() {
        let r = Arc::new(JobRegistry::new(1));
        let h = r.register();
        let waiter = {
            let h = h.clone();
            tokio::spawn(async move { h.cancelled().await })
        };
        // Cancel sebelum/ sesudah waiter mulai menunggu: keduanya harus selesai.
        r.cancel(&h.id);
        tokio::time::timeout(std::time::Duration::from_secs(2), waiter)
            .await
            .expect("tidak boleh menggantung")
            .unwrap();
    }

    #[tokio::test]
    async fn antrean_membatasi_konkurensi() {
        let r = JobRegistry::new(1);
        let gate = r.gate();
        let p1 = gate.clone().acquire_owned().await.unwrap();
        assert!(gate.clone().try_acquire_owned().is_err(), "slot kedua harus menunggu");
        drop(p1);
        assert!(gate.clone().try_acquire_owned().is_ok());
    }
}
