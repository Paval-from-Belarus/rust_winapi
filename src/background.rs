use winapi::shared::windef::{HBITMAP, HBRUSH, HDC, HGDIOBJ, RECT};
use winapi::um::wingdi::{CreatePatternBrush, DeleteObject, RestoreDC, SaveDC};
use winapi::um::winuser::FillRect;

pub struct Background {
      sprite: HBITMAP,
      brush: HBRUSH,
}

impl Background {
      pub fn new(sprite: HBITMAP) -> Background {
            let brush = unsafe { CreatePatternBrush(sprite) } as HBRUSH;
            Background { sprite, brush }
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
                  DeleteObject(self.sprite as HGDIOBJ);
                  DeleteObject(self.brush as HGDIOBJ);
            }
      }
}
