//! Desfase de la hora local respecto de UTC (`time.localtime(t).tm_gmtoff` de Python), para
//! el gráfico de ocupación y las fechas de las listas guardadas.

/// Segundos que la hora local va por delante de UTC en el instante `unix`; si el sistema no
/// lo sabe, 0 (UTC).
#[cfg(not(windows))]
pub fn local_offset_s(unix: i64) -> i64 {
    let t = unix as libc::time_t; // `time_t` es de 64 bits en los sistemas de escritorio
    // SAFETY: `tm` es una estructura C de enteros (y un puntero que `localtime_r` llena): los
    // ceros son un valor válido.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    // SAFETY: los dos punteros son de variables locales vivas; `localtime_r` es reentrante y
    // solo escribe en `tm`.
    if unsafe { libc::localtime_r(&t, &mut tm) }.is_null() {
        return 0;
    }
    tm.tm_gmtoff as i64
}

#[cfg(windows)]
pub fn local_offset_s(unix: i64) -> i64 {
    use windows_sys::Win32::Foundation::{FILETIME, SYSTEMTIME};
    use windows_sys::Win32::System::Time::{
        FileTimeToSystemTime, SystemTimeToFileTime, SystemTimeToTzSpecificLocalTime,
    };
    // FILETIME: intervalos de 100 ns desde 1601-01-01.
    let Some(utc_ft) = unix
        .checked_add(11_644_473_600)
        .and_then(|s| s.checked_mul(10_000_000))
        .filter(|&v| v >= 0)
    else {
        return 0;
    };
    let ft = FILETIME {
        dwLowDateTime: utc_ft as u32,
        dwHighDateTime: (utc_ft >> 32) as u32,
    };
    let (mut utc, mut local) = (SYSTEMTIME::default(), SYSTEMTIME::default());
    let mut out = FILETIME::default();
    // SAFETY: todos los punteros son de variables locales vivas; la zona nula pide la del
    // sistema; cada llamada solo escribe en su salida.
    let ok = unsafe {
        FileTimeToSystemTime(&ft, &mut utc) != 0
            && SystemTimeToTzSpecificLocalTime(std::ptr::null(), &utc, &mut local) != 0
            && SystemTimeToFileTime(&local, &mut out) != 0
    };
    if !ok {
        return 0;
    }
    let local_ft = (i64::from(out.dwHighDateTime) << 32) | i64::from(out.dwLowDateTime);
    (local_ft - utc_ft) / 10_000_000
}
