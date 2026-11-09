//! Brightness hardware hanya diimplementasikan di Windows untuk saat ini.

pub fn get() -> Result<Option<u8>, String> {
    Ok(None)
}

pub fn set(_level: u8) -> Result<(), String> {
    Err("Pengaturan brightness hardware baru tersedia di Windows".into())
}
