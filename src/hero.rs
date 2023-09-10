use std::fmt::{Debug, Formatter};
use std::mem::MaybeUninit;
use winapi::shared::minwindef::DWORD;
use winapi::shared::windef::{HBITMAP, HBRUSH, HDC, HGDIOBJ, LPRECT, POINT, RECT};
use winapi::um::errhandlingapi::GetLastError;
use winapi::um::wingdi::{CreatePatternBrush, CreateSolidBrush, DeleteObject, Rectangle, RectInRegion, RestoreDC, RGB, SaveDC, SelectObject};
use winapi::um::winnt::{LONG, LPWSTR};
use winapi::um::winuser::{FillRect, IntersectRect, OffsetRect};
use crate::utils::{Vector2, VectorAxis};

pub struct FlyHero {
      center_rect: RECT,
      sprite: HBITMAP,
      brush: HBRUSH,
      velocity: Vector2,
      position: Vector2,
}

impl FlyHero {
      const DEFAULT_WIDTH: LONG = 100;
      const DEFAULT_HEIGHT: LONG = 100;
      const BORDER_AWARE: LONG = 30;
      const MAX_VELOCITY: f32 = 1000_f32;
      pub fn new(center: POINT, sprite: HBITMAP) -> Result<FlyHero, DWORD> {
            let brush = unsafe {
                  CreatePatternBrush(sprite)
            };
            if brush.is_null() {
                  let error_code = unsafe { GetLastError() };
                  return Err(error_code);
            }
            let center_rect = FlyHero::center_to_rect(center.x, center.y, FlyHero::DEFAULT_WIDTH, FlyHero::DEFAULT_HEIGHT);
            Ok(FlyHero {
                  center_rect,
                  sprite,
                  brush,
                  velocity: Vector2::ZERO,
                  position: Vector2 { x: center.x as f32, y: center.y as f32 },
            })
      }

      //no hard calculation
      pub fn collides(&self, window: RECT) -> bool {
            // let delta_x: LONG = (self.velocity.x * delta) as LONG;
            // let delta_y: LONG = (self.velocity.y * delta) as LONG;
            // let mut rect = RECT::clone(&self.center_rect);
            // unsafe {
            //       OffsetRect(&mut rect, delta_x, delta_y);
            // };
            // let border_limit = 0;
            let rect = self.center_rect;
            let can_move = rect.left >= window.left && rect.right <= window.right &&
                rect.top >= window.top && rect.bottom <= window.bottom;
            !can_move
      }

      pub fn boost(&mut self, impulse: Vector2) -> bool{
            let boosted_velocity = self.velocity.add_vector(impulse);
            let was_boosted;
            if boosted_velocity.abs2() < FlyHero::MAX_VELOCITY {
                  self.velocity = boosted_velocity;
                  was_boosted = true;
            } else {
                  was_boosted = false;
            }
            was_boosted
      }
      //reflect the current vector in reverse direction
      pub fn quickJump(&mut self) {
            self.velocity = self.velocity.multiply(-1.0f32);
      }
      pub fn makeMove(&mut self, delta: f32) {
            let deltaX = (self.velocity.x * delta);
            let deltaY = (self.velocity.y * delta);
            self.position = self.position.add_coordinates(deltaX, deltaY);
            self.center_rect = FlyHero::center_to_rect(self.position.x as LONG, self.position.y as LONG,
                                                       FlyHero::DEFAULT_WIDTH, FlyHero::DEFAULT_HEIGHT);
      }

      pub fn draw(&self, hdc: HDC) {
            let positions = self.center_rect;
            unsafe {
                  SaveDC(hdc);
                  SelectObject(hdc, self.brush as HGDIOBJ);
                  FillRect(hdc, &self.center_rect, self.brush);
                  // Rectangle(hdc, positions.left, positions.top, positions.right, positions.bottom);
                  RestoreDC(hdc, -1); //restore previous hdc
            }
      }

      pub fn finalize(&mut self) {
            unsafe {
                  DeleteObject(self.brush as HGDIOBJ);
                  DeleteObject(self.sprite as HGDIOBJ);
            }
      }

      fn center_to_rect(x: LONG, y: LONG, width: LONG, height: LONG) -> RECT {
            RECT {
                  left: x - width / 2,
                  top: y - height / 2,
                  right: x + width / 2,
                  bottom: y + height / 2,

            }
      }
}