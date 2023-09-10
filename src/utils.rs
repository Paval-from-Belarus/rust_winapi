use std::ptr;
use winapi::shared::minwindef::*;
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
}

pub struct Vector2 {
      pub x: f32,
      pub y: f32,
}

pub enum VectorAxis {
      Horizontal,
      Vertical,
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
      pub fn add_coordinates(&self, x: f32, y: f32) -> Vector2 {
            Vector2 { x: x + self.x, y: y + self.y }
      }
      pub fn multiply(&self, scalar: f32) -> Vector2 {
            Vector2 { x: self.x * scalar, y: self.y * scalar }
      }
      pub fn dest2(&self, other: &Vector2) -> f32 {
            f32::powi((self.x - other.x), 2) + f32::powi((self.y - other.y), 2)
      }
      pub fn abs2(&self) -> f32 {
            f32::powi(self.x, 2) + f32::powi(self.y, 2)
      }
}

pub unsafe fn showErrorMessage(description: &str) {
      MessageBoxW(ptr::null_mut(), description.as_os_str().as_ptr(), "Error".as_os_str().as_ptr(), MB_ICONEXCLAMATION | MB_OK);
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
      const DEFAULT_STYLE: DWORD = (WS_VISIBLE | WS_OVERLAPPEDWINDOW) & !(WS_SIZEBOX | WS_MAXIMIZEBOX);
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