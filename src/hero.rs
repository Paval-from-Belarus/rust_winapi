use std::ffi::c_int;
use std::mem;
use std::mem::MaybeUninit;
use winapi::shared::minwindef::{DWORD, LPVOID};
use winapi::shared::windef::{HBITMAP, HBRUSH, HDC, HGDIOBJ, POINT, RECT};
use winapi::um::errhandlingapi::GetLastError;
use winapi::um::wingdi::{BitBlt, BITMAP, CreateCompatibleDC, CreatePatternBrush, DeleteDC, DeleteObject, GetObjectW, RestoreDC, SaveDC, SelectObject, SRCAND, SRCCOPY, SRCPAINT};
use winapi::um::winnt::{LONG};
use winapi::um::winuser::{FillRect};
use winapi_util::console::Intense::No;
use crate::MovementEvent;
use crate::utils::{show_error_message, Vector2, VectorAxis};
use crate::utils::VectorAxis::{Horizontal, Vertical};

pub struct FlyHero {
      center_rect: RECT,
      fore_hbitmap: HBITMAP,
      mask_hbitmap: HBITMAP,
      velocity: Vector2,
      position: Vector2,
}

impl FlyHero {
      const DEFAULT_WIDTH: LONG = 100;
      const DEFAULT_HEIGHT: LONG = 100;
      const MAX_VELOCITY: f32 = 40_f32;
      const MAX_VELOCITY_POWER_TWO: f32 = FlyHero::MAX_VELOCITY * FlyHero::MAX_VELOCITY;
      pub fn new(center: POINT, fore_hbitmap: HBITMAP, mask_hbitmap: HBITMAP) -> Result<FlyHero, DWORD> {
            let center_rect = FlyHero::center_to_rect(center.x, center.y, FlyHero::DEFAULT_WIDTH, FlyHero::DEFAULT_HEIGHT);
            Ok(FlyHero {
                  center_rect,
                  fore_hbitmap,
                  mask_hbitmap,
                  velocity: Vector2::ZERO,
                  position: Vector2 { x: center.x as f32, y: center.y as f32 },
            })
      }
      pub fn position(&self) -> Vector2 {
            Vector2 { x: self.position.x, y: self.position.y }
      }
      //no hard calculation
      pub fn collides(&self, window: RECT) -> Option<VectorAxis> {
            let rect = self.center_rect;
            let is_horizontal_collision = rect.left < window.left || rect.right > window.right;
            let is_vertical_collision = rect.top < window.top || rect.bottom > window.bottom;
            if is_horizontal_collision {
                  return Some(Horizontal);
            }
            if is_vertical_collision {
                  return Some(Vertical);
            }
            return None;
            // let can_move = rect.left >= window.left && rect.right <= window.right &&
            //     rect.top >= window.top && rect.bottom <= window.bottom;
            // !can_move
      }
      pub fn shift(&mut self, x_offset: isize, y_offset: isize) {
            self.position = self.position.add_coordinates(x_offset as f32, y_offset as f32);
            self.makeMove(0_f32);
      }
      pub fn rect(&self) -> RECT {
            self.center_rect
      }
      pub fn stop(&mut self) {
            self.velocity = Vector2::ZERO;
      }
      pub fn boost(&mut self, impulse: Vector2) -> bool {
            const MIN_BOOSTED_DIFF: f32 = 0.9;
            let boosted_velocity = self.velocity.add_vector(impulse);
            let was_boosted;
            let boosted_power_two = boosted_velocity.len2();
            if boosted_power_two < MIN_BOOSTED_DIFF {
                  self.velocity = Vector2::ZERO;
                  return false;
            }
            if boosted_power_two < FlyHero::MAX_VELOCITY_POWER_TWO {
                  self.velocity = boosted_velocity;
                  was_boosted = true;
            } else {
                  let boosted_abs = boosted_power_two.sqrt();
                  let x = self.velocity.x + (boosted_velocity.x - self.velocity.x) / boosted_abs;
                  let y = self.velocity.y + (boosted_velocity.y - self.velocity.y) / boosted_abs;
                  self.velocity = Vector2 { x, y };
                  was_boosted = false;
            }
            was_boosted
      }
      pub fn velocity(&self) -> Vector2 {
            Vector2 { x: self.velocity.x, y: self.velocity.y }
      }
      //reflect the current vector in reverse direction
      pub fn quickJump(&mut self, axis: VectorAxis) {
            match axis {
                  Horizontal => {
                        self.velocity.x *= -1.0f32;
                  }
                  Vertical => {
                        self.velocity.y *= -1.0f32;
                  }
            }
            // self.velocity = self.velocity.multiply(-1.0f32);
      }
      pub fn makeMove(&mut self, delta: f32) {
            let deltaX = self.velocity.x * delta;
            let deltaY = self.velocity.y * delta;
            self.position = self.position.add_coordinates(deltaX, deltaY);
            self.center_rect = FlyHero::center_to_rect(self.position.x as LONG, self.position.y as LONG,
                                                       FlyHero::DEFAULT_WIDTH, FlyHero::DEFAULT_HEIGHT);
      }
      pub fn setPosition(&mut self, center: POINT) {
            let center_rect = FlyHero::center_to_rect(center.x, center.y, FlyHero::DEFAULT_WIDTH, FlyHero::DEFAULT_HEIGHT);
            self.velocity = Vector2::ZERO;
            self.center_rect = center_rect;
      }

      pub fn draw(&self, hdc: HDC) {
            let positions = self.center_rect;
            unsafe {
                  SaveDC(hdc);
                  let mem_hdc = CreateCompatibleDC(hdc);
                  if mem_hdc.is_null() {
                        show_error_message("Failed to create compatible dc for hero");
                  }
                  let width = positions.right - positions.left;
                  let height = positions.bottom - positions.top;
                  SelectObject(mem_hdc, self.mask_hbitmap as HGDIOBJ);
                  BitBlt(hdc, positions.left, positions.top, width, height, mem_hdc, 0, 0, SRCAND);
                  SelectObject(mem_hdc, self.fore_hbitmap as HGDIOBJ);
                  BitBlt(hdc, positions.left, positions.top, width, height, mem_hdc, 0, 0, SRCPAINT);
                  DeleteDC(mem_hdc);
                  RestoreDC(hdc, -1); //restore previous hdc
            }
      }

      pub fn finalize(&mut self) {
            unsafe {
                  DeleteObject(self.mask_hbitmap as HGDIOBJ);
                  DeleteObject(self.fore_hbitmap as HGDIOBJ);
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