#![allow(clippy::upper_case_acronyms)] // Windows API types use uppercase names

use std::ffi::c_void;
use std::mem::MaybeUninit;

#[cfg(target_os = "windows")]
fn enable_windows_per_monitor_dpi_awareness() {
    use std::ffi::c_void;

    type HMODULE = *mut c_void;
    type FARPROC = *mut c_void;
    type BOOL = i32;
    type HRESULT = i32;

    const PROCESS_PER_MONITOR_DPI_AWARE: i32 = 2;

    fn wide_null_terminated(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryW(lp_lib_file_name: *const u16) -> HMODULE;
        fn GetProcAddress(h_module: HMODULE, lp_proc_name: *const i8) -> FARPROC;
    }

    unsafe {
        let user32 = LoadLibraryW(wide_null_terminated("user32.dll").as_ptr());
        if !user32.is_null() {
            let set_context = GetProcAddress(user32, c"SetProcessDpiAwarenessContext".as_ptr());
            if !set_context.is_null() {
                type SetProcessDpiAwarenessContextFn =
                    unsafe extern "system" fn(*mut c_void) -> BOOL;
                let set_context: SetProcessDpiAwarenessContextFn = std::mem::transmute(set_context);
                if set_context((-4isize) as *mut c_void) != 0 {
                    return;
                }
            }
        }

        let shcore = LoadLibraryW(wide_null_terminated("shcore.dll").as_ptr());
        if !shcore.is_null() {
            let set_awareness = GetProcAddress(shcore, c"SetProcessDpiAwareness".as_ptr());
            if !set_awareness.is_null() {
                type SetProcessDpiAwarenessFn = unsafe extern "system" fn(i32) -> HRESULT;
                let set_awareness: SetProcessDpiAwarenessFn = std::mem::transmute(set_awareness);
                if set_awareness(PROCESS_PER_MONITOR_DPI_AWARE) == 0 {
                    return;
                }
            }
        }

        if !user32.is_null() {
            let set_dpi_aware = GetProcAddress(user32, c"SetProcessDPIAware".as_ptr());
            if !set_dpi_aware.is_null() {
                type SetProcessDPIAwareFn = unsafe extern "system" fn() -> BOOL;
                let set_dpi_aware: SetProcessDPIAwareFn = std::mem::transmute(set_dpi_aware);
                let _ = set_dpi_aware();
            }
        }
    }
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
struct MonitorRect {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    dpi: u32,
    scale: f32,
}

#[cfg(target_os = "windows")]
fn monitor_rects() -> Vec<MonitorRect> {
    use std::ptr;

    type HMONITOR = *mut c_void;
    type HDC = *mut c_void;
    type LPARAM = isize;
    type BOOL = i32;
    type UINT = u32;
    type HRESULT = i32;

    const CCHDEVICENAME: usize = 32;
    const CCHFORMNAME: usize = 32;
    const ENUM_CURRENT_SETTINGS: u32 = 0xFFFF_FFFF;
    const MDT_EFFECTIVE_DPI: i32 = 0;

    #[repr(C)]
    struct RECT {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    #[allow(non_snake_case)]
    struct MONITORINFOEXW {
        cbSize: u32,
        rcMonitor: RECT,
        rcWork: RECT,
        dwFlags: u32,
        szDevice: [u16; CCHDEVICENAME],
    }

    #[repr(C)]
    #[allow(non_snake_case)]
    struct POINTL {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    #[allow(non_snake_case)]
    struct DEVMODEW {
        dmDeviceName: [u16; CCHDEVICENAME],
        dmSpecVersion: u16,
        dmDriverVersion: u16,
        dmSize: u16,
        dmDriverExtra: u16,
        dmFields: u32,
        dmPosition: POINTL,
        dmDisplayOrientation: u32,
        dmDisplayFixedOutput: u32,
        dmColor: i16,
        dmDuplex: i16,
        dmYResolution: i16,
        dmTTOption: i16,
        dmCollate: i16,
        dmFormName: [u16; CCHFORMNAME],
        dmLogPixels: u16,
        dmBitsPerPel: u32,
        dmPelsWidth: u32,
        dmPelsHeight: u32,
        dmDisplayFlags: u32,
        dmDisplayFrequency: u32,
        dmICMMethod: u32,
        dmICMIntent: u32,
        dmMediaType: u32,
        dmDitherType: u32,
        dmReserved1: u32,
        dmReserved2: u32,
        dmPanningWidth: u32,
        dmPanningHeight: u32,
    }

    type MonitorEnumProc =
        Option<unsafe extern "system" fn(HMONITOR, HDC, *mut RECT, LPARAM) -> BOOL>;

    #[link(name = "user32")]
    extern "system" {
        fn EnumDisplayMonitors(
            hdc: HDC,
            lprcClip: *const RECT,
            lpfnEnum: MonitorEnumProc,
            dwData: LPARAM,
        ) -> BOOL;
        fn GetMonitorInfoW(hMonitor: HMONITOR, lpmi: *mut MONITORINFOEXW) -> BOOL;
        fn EnumDisplaySettingsW(
            lpszDeviceName: *const u16,
            iModeNum: u32,
            lpDevMode: *mut DEVMODEW,
        ) -> BOOL;
    }

    #[link(name = "Shcore")]
    extern "system" {
        fn GetDpiForMonitor(
            hmonitor: HMONITOR,
            dpiType: i32,
            dpiX: *mut UINT,
            dpiY: *mut UINT,
        ) -> HRESULT;
    }

    unsafe extern "system" fn enum_monitor_cb(
        hmonitor: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        data: LPARAM,
    ) -> BOOL {
        let monitors: &mut Vec<MonitorRect> = &mut *(data as *mut Vec<MonitorRect>);

        let mut mi = MaybeUninit::<MONITORINFOEXW>::zeroed();
        (*mi.as_mut_ptr()).cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(hmonitor, mi.as_mut_ptr()) != 0 {
            let mi = mi.assume_init();
            let r = mi.rcMonitor;

            let mut dpi_x: UINT = 96;
            let mut dpi_y: UINT = 96;
            let _ = GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
            let scale = dpi_x as f32 / 96.0;

            let mut dm = MaybeUninit::<DEVMODEW>::zeroed();
            (*dm.as_mut_ptr()).dmSize = std::mem::size_of::<DEVMODEW>() as u16;
            let dm_ok =
                EnumDisplaySettingsW(mi.szDevice.as_ptr(), ENUM_CURRENT_SETTINGS, dm.as_mut_ptr())
                    != 0;
            if dm_ok {
                let dm = dm.assume_init();
                println!(
                    "device={:?} rcMonitor=({},{} {}x{}) devmode=({},{} {}x{}) dpi={} scale={:.0}%",
                    String::from_utf16_lossy(&mi.szDevice).trim_end_matches('\0'),
                    r.left,
                    r.top,
                    (r.right - r.left).max(0),
                    (r.bottom - r.top).max(0),
                    dm.dmPosition.x,
                    dm.dmPosition.y,
                    dm.dmPelsWidth,
                    dm.dmPelsHeight,
                    dpi_x,
                    scale * 100.0,
                );
                monitors.push(MonitorRect {
                    x: dm.dmPosition.x,
                    y: dm.dmPosition.y,
                    width: dm.dmPelsWidth,
                    height: dm.dmPelsHeight,
                    dpi: dpi_x,
                    scale,
                });
            } else {
                println!(
                    "device={:?} rcMonitor=({},{} {}x{}) devmode=<failed> dpi={} scale={:.0}%",
                    String::from_utf16_lossy(&mi.szDevice).trim_end_matches('\0'),
                    r.left,
                    r.top,
                    (r.right - r.left).max(0),
                    (r.bottom - r.top).max(0),
                    dpi_x,
                    scale * 100.0,
                );
                monitors.push(MonitorRect {
                    x: r.left,
                    y: r.top,
                    width: (r.right - r.left).max(0) as u32,
                    height: (r.bottom - r.top).max(0) as u32,
                    dpi: dpi_x,
                    scale,
                });
            }
        }

        1
    }

    let mut monitors = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            ptr::null_mut(),
            ptr::null(),
            Some(enum_monitor_cb),
            (&mut monitors as *mut Vec<MonitorRect>) as LPARAM,
        );
    }
    monitors
}

fn main() {
    #[cfg(target_os = "windows")]
    {
        enable_windows_per_monitor_dpi_awareness();
        let _ = monitor_rects();
    }

    #[cfg(not(target_os = "windows"))]
    {
        println!("This binary is intended to run on Windows.");
    }
}
