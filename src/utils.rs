use std::mem::MaybeUninit;
use std::ptr;
use std::task::ready;
use winapi::shared::minwindef::*;
use winapi::shared::windef::{COLORREF, HBITMAP, HBRUSH, HDC, HGDIOBJ, HPEN, HWND, LPRECT, POINT, RECT};
use winapi::um::processthreadsapi::{GetStartupInfoW, LPSTARTUPINFOW, STARTUPINFOW};
use winapi::um::wingdi::{CreateCompatibleBitmap, CreateCompatibleDC, CreatePen, CreateSolidBrush, DeleteDC, DeleteObject, GetCharWidth32W, RestoreDC, SaveDC, SelectObject};
use winapi::um::winnt::LONG;
use winapi::um::winuser::*;

pub trait WindowsString {
      fn as_os_str(&self) -> Vec<u16>;
}

pub struct FormParams {
      pub(crate) style: DWORD,
      pub(crate) position: (LONG, LONG),
      pub(crate) width: LONG,
      pub(crate) height: LONG,
      pub(crate) startup_info: STARTUPINFOW,

}

pub struct Vector2 {
      pub x: f32,
      pub y: f32,
}

pub enum VectorAxis {
      Horizontal,
      Vertical,
}
macro_rules! GET_X_LPARAM {
    ($lp:expr) => {
        ($lp & 0xffff) as LONG
    };
}

macro_rules! GET_Y_LPARAM {
    ($lp:expr) => {
        (($lp >> 16) & 0xffff) as LONG
    };
}
impl Vector2 {
      pub const ZERO: Vector2 = Vector2 { x: 0.0, y: 0.0 };
      pub const UP: Vector2 = Vector2 { x: 0.0, y: -1.0 };
      pub const DOWN: Vector2 = Vector2 { x: 0.0, y: 1.0 };
      pub const LEFT: Vector2 = Vector2 { x: -1.0, y: 0.0 };
      pub const RIGHT: Vector2 = Vector2 { x: 1.0, y: 0.0 };
      pub fn add_vector(&self, other: Vector2) -> Vector2 {
            Vector2 { x: self.x + other.x, y: self.y + other.y }
      }
      pub fn sub_vector(&self, other: Vector2) -> Vector2 {
            Vector2 { x: self.x - other.x, y: self.y - other.y }
      }
      pub fn sub_coordinates(&self, x: f32, y: f32) -> Vector2 {
            Vector2 { x: self.x - x, y: self.y - y }
      }
      pub fn add_coordinates(&self, x: f32, y: f32) -> Vector2 {
            Vector2 { x: x + self.x, y: y + self.y }
      }
      pub fn multiply(&self, scalar: f32) -> Vector2 {
            Vector2 { x: self.x * scalar, y: self.y * scalar }
      }
      pub fn dest2(&self, other: &Vector2) -> f32 {
            f32::powi(self.x - other.x, 2) + f32::powi(self.y - other.y, 2)
      }
      pub fn len2(&self) -> f32 {
            f32::powi(self.x, 2) + f32::powi(self.y, 2)
      }
}

pub fn show_error_message(description: &str) {
      unsafe {
            MessageBoxW(ptr::null_mut(), description.as_os_str().as_ptr(), "Error".as_os_str().as_ptr(), MB_ICONEXCLAMATION | MB_OK);
      }
}

pub struct BackBuffer {
      hdc: HDC,
      hBitmap: HBITMAP,
}

impl Drop for BackBuffer {
      fn drop(&mut self) {
            self.finalize();
      }
}

impl BackBuffer {
      pub fn new(hWindow: HWND, width: INT, height: INT) -> BackBuffer {
            unsafe {
                  let hdcWindow = GetDC(hWindow);
                  let hdc = CreateCompatibleDC(hdcWindow);
                  let hBitmap = CreateCompatibleBitmap(hdcWindow, width, height);
                  SaveDC(hdc);
                  SelectObject(hdc, hBitmap as HGDIOBJ);
                  ReleaseDC(hWindow, hdcWindow);
                  BackBuffer {
                        hdc,
                        hBitmap,
                  }
            }
      }
      pub fn getHDC(&self) -> HDC {
            self.hdc
      }
      pub fn finalize(&mut self) {
            unsafe {
                  RestoreDC(self.hdc, -1);
                  DeleteObject(self.hBitmap as HGDIOBJ);
                  DeleteDC(self.hdc);
            }
      }
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

pub fn offset_rect(rect: &mut RECT, delta_x: INT, delta_y: INT) {
      unsafe {
            OffsetRect(rect, delta_x, delta_y);
      }
}

pub fn copy_rect(dest: &mut RECT, source: &RECT) {
      unsafe {
            CopyRect(dest, source);
      }
}

pub fn create_solid_brush(color: COLORREF) -> HBRUSH {
      unsafe {
            CreateSolidBrush(color)
      }
}

pub fn create_pen(style: DWORD, width: DWORD, color: COLORREF) -> HPEN {
      unsafe {
            CreatePen(style as INT, width as INT, color)
      }
}

#[inline]
pub fn point_in_rect(rect: &RECT, x: LONG, y: LONG) -> bool {
      let point = POINT {x, y};
      let result = unsafe { PtInRect(rect, point) };
      result != FALSE
}

pub fn get_char_width(hdc: HDC, char: u16) -> INT {
      let mut char_width: INT = 0;
      let letter = char as UINT;
      unsafe {
            GetCharWidth32W(hdc, letter, letter, &mut char_width)
      };
      return char_width;
}

pub fn rect_width(rect: &RECT) -> LONG {
      LONG::abs(rect.right - rect.left)
}

pub fn rect_height(rect: &RECT) -> LONG {
      LONG::abs(rect.bottom - rect.top)
}

pub fn get_client_rect(hWindow: HWND, rect: &mut RECT) {
      unsafe {
            GetClientRect(hWindow, rect);
      }
}

impl FormParams {
      const DEFAULT_STYLE: DWORD = (WS_VISIBLE | WS_OVERLAPPEDWINDOW | WS_VSCROLL);
      //& !(WS_SIZEBOX | WS_MAXIMIZEBOX);
      const DEFAULT_WIDTH: INT = 800;
      const DEFAULT_HEIGHT: INT = 600;
      pub fn getDefaultParams() -> FormParams {
            let (xOffset, yOffset) = unsafe {
                  let xOffset = GetSystemMetrics(SM_CXSCREEN) / 2 - FormParams::DEFAULT_WIDTH / 2;
                  let yOffset = GetSystemMetrics(SM_CYSCREEN) / 2 - FormParams::DEFAULT_HEIGHT / 2;
                  (xOffset, yOffset)
            };
            let startup_info = unsafe {
                  let mut info = MaybeUninit::<STARTUPINFOW>::uninit();
                  GetStartupInfoW(info.as_mut_ptr() as LPSTARTUPINFOW);
                  info.assume_init()
            };
            FormParams {
                  style: FormParams::DEFAULT_STYLE,
                  position: (xOffset, yOffset),
                  width: FormParams::DEFAULT_WIDTH,
                  height: FormParams::DEFAULT_HEIGHT,
                  startup_info,
            }
      }
      pub fn get_client_window(&self) -> RECT {
            RECT { left: 0, top: 0, right: self.width, bottom: self.height }
      }
}
