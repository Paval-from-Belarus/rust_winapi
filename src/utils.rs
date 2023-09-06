use std::error::Error;
use std::ptr;
use winapi::shared::minwindef::*;
use winapi::shared::windef::*;
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::um::winnt::LONG;
use winapi::um::winuser::*;
use crate::resources::{TITLE_ICON};

pub trait WindowsString {
    fn as_os_str(&self) -> Vec<u16>;
}
pub struct FormParams {
    pub(crate) style: DWORD,
    pub(crate) position: (LONG, LONG),
    pub(crate) width: LONG,
    pub(crate) height: LONG,
}


pub unsafe fn showErrorMessage(description: &str) {
    MessageBoxW(ptr::null_mut(), description.as_os_str().as_ptr(), "Error".as_os_str().as_ptr(), MB_ICONEXCLAMATION | MB_OK);
}


pub unsafe fn createMainWindow(className: &str, windowLabel: &str, eventHandler: WNDPROC, params: &FormParams) -> Result<HWND, Box<dyn Error>> {
    let className = className.as_os_str();
    let windowLabel = windowLabel.as_os_str();
    let hInstance = GetModuleHandleW(ptr::null_mut());
    let hIcon = LoadImageW(
        hInstance,
        MAKEINTRESOURCEW(TITLE_ICON),
        IMAGE_ICON,
        0, 0, 0) as HICON;
    let windowClass = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as UINT,
        style: CS_OWNDC | CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: eventHandler,
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance,
        hIcon,
        hCursor: LoadCursorW(ptr::null_mut(), IDC_HAND),
        hbrBackground: COLOR_WINDOWFRAME as HBRUSH,
        lpszMenuName: ptr::null_mut(),
        lpszClassName: className.as_ptr(),
        hIconSm: LoadIconW(ptr::null_mut(), IDI_APPLICATION),
    };
    if RegisterClassExW(&windowClass) == 0 {
        showErrorMessage("Window registration failed");
        return Result::Err("Register class error".into());
    }
    let hWindow = CreateWindowExW(
        0,                                // dwExStyle
        className.as_ptr(),                    // lpClassName
        windowLabel.as_ptr(),                   // lpWindowName
        params.style, // dwStyle
        params.position.0,                    // Int x
        params.position.1,                    // Int y
        params.width,                    // Int nWidth
        params.height,
        ptr::null_mut(),
        ptr::null_mut(),
        hInstance,
        ptr::null_mut(),
    );
    if hWindow.is_null() {
        showErrorMessage("Window creation failed");
        return Result::Err("Window creation failed".into());
    }
    return Result::Ok(hWindow);
}
impl WindowsString for str {
    fn as_os_str(&self) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        std::ffi::OsStr::new(self)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }
}
impl FormParams {
    const DEFAULT_STYLE: DWORD = (WS_VISIBLE | WS_OVERLAPPEDWINDOW);
    //& !(WS_SIZEBOX | WS_MAXIMIZEBOX);
    const DEFAULT_WIDTH: INT = 800;
    const DEFAULT_HEIGHT: INT = 600;
    pub fn getDefaultParams() -> FormParams {
        let (xOffset, yOffset) = unsafe {
            let xOffset = GetSystemMetrics(SM_CXSCREEN) / 2 - FormParams::DEFAULT_WIDTH / 2;
            let yOffset = GetSystemMetrics(SM_CYSCREEN) / 2 - FormParams::DEFAULT_HEIGHT / 2;
            (xOffset, yOffset)
        };
        FormParams {
            style: FormParams::DEFAULT_STYLE,
            position: (xOffset, yOffset),
            width: FormParams::DEFAULT_WIDTH,
            height: FormParams::DEFAULT_HEIGHT,
        }
    }
}