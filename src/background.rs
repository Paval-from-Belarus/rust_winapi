use winapi::shared::windef::{COLORREF, HBITMAP, HBRUSH, HDC, HGDIOBJ, RECT};
use winapi::um::wingdi::{CreatePatternBrush, CreateSolidBrush, DeleteObject, RestoreDC, SaveDC};
use winapi::um::winnt::LONG;
use winapi::um::winuser::FillRect;
use winapi_util::console::Color;

pub struct Background {
      brush: HBRUSH,
      rect: RECT,
}

impl Background {
      pub fn solid(color: COLORREF, width: LONG, height: LONG) -> Background {
            let brush = unsafe { CreateSolidBrush(color) };
            let rect = RECT { left: 0, top: 0, right: width, bottom: height };
            Background { brush, rect}
      }
      pub fn resize(&mut self, width: LONG, height: LONG) {
            self.rect.right = width;
            self.rect.bottom = height;
      }
      // pub fn new(sprite: HBITMAP) -> Background {
      //       let brush = unsafe { CreatePatternBrush(sprite) } as HBRUSH;
      //       unsafe { DeleteObject(sprite as HGDIOBJ); }
      //       Background { brush }
      // }
      pub fn draw(&mut self, hdc: HDC) {
            unsafe {
                  SaveDC(hdc);
                  // SelectObject(hdc, self.sprite as HGDIOBJ);
                  // Rectangle(hdc, window.left, window.top, window.right, window.bottom);
                  FillRect(hdc, &self.rect, self.brush);
                  RestoreDC(hdc, -1);
            }
      }
      pub fn finalize(&mut self) {
            unsafe {
                  DeleteObject(self.brush as HGDIOBJ);
            }
      }
}
