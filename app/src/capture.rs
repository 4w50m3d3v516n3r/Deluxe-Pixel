//! Screen capture (the companion Camera program) and printing through Windows.

use crate::convert::Rgba;

/// Grab the whole primary screen as a true-colour picture.
#[cfg(windows)]
pub fn grab_screen() -> Result<Rgba, String> {
    use windows_sys::Win32::Graphics::Gdi::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
    unsafe {
        let w = GetSystemMetrics(SM_CXSCREEN);
        let h = GetSystemMetrics(SM_CYSCREEN);
        if w <= 0 || h <= 0 {
            return Err("Couldn't read the screen size".into());
        }
        let screen = GetDC(std::ptr::null_mut());
        let mem = CreateCompatibleDC(screen);
        let bmp = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(mem, bmp as HGDIOBJ);
        let ok = BitBlt(mem, 0, 0, w, h, screen, 0, 0, SRCCOPY);
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = w;
        info.bmiHeader.biHeight = -h;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        let mut buf = vec![0u8; (w * h * 4) as usize];
        let lines = GetDIBits(mem, bmp, 0, h as u32, buf.as_mut_ptr() as *mut _, &mut info, DIB_RGB_COLORS);
        SelectObject(mem, old);
        DeleteObject(bmp as HGDIOBJ);
        DeleteDC(mem);
        ReleaseDC(std::ptr::null_mut(), screen);
        if ok == 0 || lines == 0 {
            return Err("Couldn't capture the screen".into());
        }
        let px = buf.chunks_exact(4).map(|p| [p[2], p[1], p[0]]).collect();
        Ok(Rgba { w, h, px })
    }
}

#[cfg(not(windows))]
pub fn grab_screen() -> Result<Rgba, String> {
    Err("Screen capture is only available on Windows".into())
}

/// Send a saved picture file to the default printer.
#[cfg(windows)]
pub fn print_file(path: &std::path::Path) -> Result<(), String> {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;
    let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let verb = wide("print");
    let file = wide(&path.to_string_lossy());
    let r = unsafe { ShellExecuteW(std::ptr::null_mut(), verb.as_ptr(), file.as_ptr(), std::ptr::null(), std::ptr::null(), SW_HIDE) };
    if (r as isize) <= 32 {
        Err("Couldn't send the picture to the printer. You must first choose a printer in Windows.".into())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
pub fn print_file(_path: &std::path::Path) -> Result<(), String> {
    Err("Printing is only available on Windows".into())
}
