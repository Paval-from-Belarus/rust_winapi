use winapi::shared::windef::{COLORREF, HBITMAP, HBRUSH, HDC, HGDIOBJ, RECT};
use winapi::um::wingdi::{CreatePatternBrush, CreateSolidBrush, DeleteObject, RestoreDC, SaveDC};
use winapi::um::winuser::FillRect;
use winapi_util::console::Color;

pub struct Background {
      brush: HBRUSH,
}

impl Background {
      pub fn solid(color: COLORREF) -> Background {
            let brush = unsafe { CreateSolidBrush(color) };
            Background { brush }
      }
      pub fn new(sprite: HBITMAP) -> Background {
            let brush = unsafe { CreatePatternBrush(sprite) } as HBRUSH;
            unsafe { DeleteObject(sprite as HGDIOBJ); }
            Background { brush }
      }
      pub fn draw(&mut self, window: &RECT, hdc: HDC) {
            unsafe {
                  SaveDC(hdc);
                  // SelectObject(hdc, self.sprite as HGDIOBJ);
                  // Rectangle(hdc, window.left, window.top, window.right, window.bottom);
                  FillRect(hdc, window, self.brush);
                  RestoreDC(hdc, -1);
            }
      }
      pub fn finalize(&mut self) {
            unsafe {
                  DeleteObject(self.brush as HGDIOBJ);
            }
      }
}
