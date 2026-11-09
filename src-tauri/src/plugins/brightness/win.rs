//! Hardware brightness lewat WMI (Windows Management Instrumentation).
//! Kelas `WmiMonitorBrightness` untuk membaca level, `WmiMonitorBrightnessMethods` untuk menulis.
//! Hanya monitor yang mengekspos kelas WMI ini (umumnya panel laptop) yang bisa diatur.

use windows::core::{BSTR, GUID, Interface, PCWSTR};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSIDFromProgID, CLSCTX_INPROC_SERVER,
    COINIT_MULTITHREADED, DISPPARAMS, DISPATCH_METHOD, IDispatch,
};
use windows::Win32::System::Variant::{VariantClear, VariantToInt32, VARIANT, VT_I4};
use windows::Win32::System::Wmi::{
    ISWbemLocator, ISWbemObjectSet, ISWbemServices, WBEM_FLAG_FORWARD_ONLY,
    WBEM_FLAG_RETURN_IMMEDIATELY,
};

fn err<E: std::fmt::Display>(e: E) -> String {
    format!("Brightness: {e}")
}

const UNSUPPORTED: &str =
    "Monitor ini tidak mendukung pengaturan brightness lewat Windows (umumnya monitor eksternal)";

/// Jaga keseimbangan CoInitializeEx/CoUninitialize. Bila thread sudah punya apartment lain,
/// `CoInitializeEx` gagal dan kita tidak boleh memanggil `CoUninitialize`.
struct Com(bool);

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}

fn com() -> Com {
    Com(unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok())
}

fn services() -> Result<ISWbemServices, String> {
    unsafe {
        let clsid = CLSIDFromProgID(windows::core::w!("WbemScripting.SWbemLocator")).map_err(err)?;
        let locator: ISWbemLocator =
            CoCreateInstance(&clsid, None::<&windows::core::IUnknown>, CLSCTX_INPROC_SERVER).map_err(err)?;
        let empty = BSTR::new();
        locator
            .ConnectServer(
                &BSTR::from("."),
                &BSTR::from("ROOT\\WMI"),
                &empty,
                &empty,
                &empty,
                &empty,
                0,
                None::<&IDispatch>,
            )
            .map_err(err)
    }
}

fn query(svc: &ISWbemServices, wql: &str) -> Result<ISWbemObjectSet, String> {
    unsafe {
        svc.ExecQuery(
            &BSTR::from("WQL"),
            &BSTR::from(wql),
            WBEM_FLAG_FORWARD_ONLY.0 | WBEM_FLAG_RETURN_IMMEDIATELY.0,
            None::<&IDispatch>,
        )
        .map_err(err)
    }
}

/// Object pertama dari query; `None` bila monitor tidak mengekspos kelas WMI tersebut.
fn first(svc: &ISWbemServices, wql: &str) -> Result<Option<windows::Win32::System::Wmi::ISWbemObject>, String> {
    let set = query(svc, wql)?;
    let count = unsafe { set.Count() }.map_err(err)?;
    if count <= 0 {
        return Ok(None);
    }
    unsafe { set.ItemIndex(0) }.map(Some).map_err(err)
}

pub fn get() -> Result<Option<u8>, String> {
    let _com = com();
    let svc = services()?;
    // Monitor eksternal sering membuat query ini gagal dengan 0x80004005 (WBEM_E_FAILED).
    // Itu berarti "tidak tersedia", bukan error yang perlu ditampilkan ke pengguna.
    let set = match query(&svc, "SELECT CurrentBrightness FROM WmiMonitorBrightness") {
        Ok(set) => set,
        Err(_) => return Ok(None),
    };
    let count = unsafe { set.Count() }.map_err(err)?;
    if count <= 0 {
        return Ok(None);
    }
    let obj = unsafe { set.ItemIndex(0) }.map_err(err)?;
    let props = unsafe { obj.Properties_() }.map_err(err)?;
    let prop = unsafe { props.Item(&BSTR::from("CurrentBrightness"), 0) }.map_err(err)?;
    let value = unsafe { prop.Value() }.map_err(err)?;
    let level = unsafe { VariantToInt32(&value) }.map_err(err)?;
    Ok(Some(level.clamp(0, 100) as u8))
}

fn variant_i4(value: i32) -> VARIANT {
    let mut v = VARIANT::default();
    unsafe {
        (*v.Anonymous.Anonymous).vt = VT_I4;
        (*v.Anonymous.Anonymous).Anonymous.lVal = value;
    }
    v
}

pub fn set(level: u8) -> Result<(), String> {
    let _com = com();
    let svc = services()?;
    let Some(obj) = first(&svc, "SELECT * FROM WmiMonitorBrightnessMethods").map_err(|_| UNSUPPORTED.to_string())? else {
        return Err(UNSUPPORTED.into());
    };
    let dispatch: IDispatch = obj.cast().map_err(err)?;
    let level = level.min(100) as i32;
    unsafe {
        // COM menerima argumen dalam urutan terbalik: Brightness dulu, lalu Timeout.
        let mut args = [variant_i4(level), variant_i4(1)];
        let name: PCWSTR = windows::core::w!("WmiSetBrightness");
        let mut dispid = 0i32;
        let call = dispatch
            .GetIDsOfNames(&GUID::zeroed(), &name, 1, 0, &mut dispid)
            .and_then(|_| {
                let params = DISPPARAMS {
                    rgvarg: args.as_mut_ptr(),
                    rgdispidNamedArgs: std::ptr::null_mut(),
                    cArgs: 2,
                    cNamedArgs: 0,
                };
                dispatch.Invoke(dispid, &GUID::zeroed(), 0, DISPATCH_METHOD, &params, None, None, None)
            });
        for arg in args.iter_mut() {
            let _ = VariantClear(arg);
        }
        call.map_err(err)?;
    }
    Ok(())
}
