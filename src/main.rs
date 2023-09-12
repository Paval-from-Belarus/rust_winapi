#![feature(new_uninit)]
#![cfg(windows)]
// Let's put this so that it won't open the console
#![windows_subsystem = "windows"]
#![allow(non_snake_case)]

extern crate core;

mod resources;
#[macro_use]
mod utils;
mod hero;
mod background;


use std::mem::MaybeUninit;

use winapi::shared::minwindef::*;
use winapi::shared::windef::*;
use winapi::um::libloaderapi::{GetModuleFileNameW, GetModuleHandleW};
use winapi::um::winuser::*;
use std::{cmp, mem, ptr};
use std::ffi::c_int;
use std::ops::AddAssign;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use winapi::ctypes::__uint8;
use winapi::um::processthreadsapi::GetStartupInfoW;
use winapi::um::winbase::STARTUPINFOEXW;
use winapi::um::wingdi::{BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, MAKEPOINTS, MAKEROP4, PATCOPY, PATINVERT, RestoreDC, SaveDC, SelectObject, SRCCOPY};
use winapi::um::winnt::LONG;
use crate::background::Background;
use crate::hero::FlyHero;

use crate::resources::{BACKGROUND_BITMAP, HERO_FORE_BITMAP, HERO_MASK_BITMAP, load_resources, TITLE_ICON};
use crate::utils::{FormParams, show_error_message, Vector2, WindowsString};

unsafe fn onLeftButtonDown(hWindow: HWND) {
      let hInstance = GetModuleHandleW(ptr::null_mut());
      let mut name: Vec<u16> = Vec::with_capacity(MAX_PATH);
      let read_len = GetModuleFileNameW(hInstance, name.as_mut_ptr(), MAX_PATH as u32);
      name.set_len(read_len as usize);
      // We could convert name to String using String::from_utf16_lossy(&name)
      MessageBoxW(
            hWindow,
            name.as_ptr(),
            "This program is:".as_os_str().as_ptr(),
            MB_OK | MB_ICONINFORMATION,
      );
}

pub struct BackBuffer {
      hdc: HDC,
      hBitmap: HBITMAP,
}

pub struct Window {
      hWindow: HWND,
      backBuffer: BackBuffer,
      mainHero: FlyHero,
      background: Background,
      //the window size
      clientWindow: RECT,
      isShiftPressed: bool,
}

#[derive(Copy, Clone, IntoPrimitive)]
#[repr(i32)]
pub enum MovementEvent {
      Left = 0,
      Up = 1,
      Right = 2,
      Down = 3,
}

impl MovementEvent {
      pub fn is_horizontal(&self) -> bool {
            matches!(self, MovementEvent::Left) || matches!(self, MovementEvent::Right)
      }
      pub fn is_vertical(&self) -> bool {
            matches!(self, MovementEvent::Up) || matches!(self, MovementEvent::Down)
      }
}

impl TryFrom<i32> for MovementEvent {
      type Error = ();
      fn try_from(value: i32) -> Result<Self, Self::Error> {
            match value {
                  VK_LEFT => Ok(MovementEvent::Left),
                  VK_RIGHT => Ok(MovementEvent::Right),
                  VK_UP => Ok(MovementEvent::Up),
                  VK_DOWN => Ok(MovementEvent::Down),
                  _ => Err(())
            }
      }
}

impl Window {
      //this function
      pub fn new(className: &str, windowTitle: &str, params: FormParams) -> Box<Self> {
            let className = className.as_os_str();
            let windowTitle = windowTitle.as_os_str();
            let hInstance = unsafe {
                  GetModuleHandleW(ptr::null_mut())
            };

            let mut window = Box::<Window>::new_uninit();
            let mut resources = load_resources(hInstance).expect("Failed to load resources");
            let hWindow = unsafe {
                  let hIcon = (resources.remove(&TITLE_ICON)).unwrap() as HICON;
                  let windowClass = WNDCLASSEXW {
                        cbSize: mem::size_of::<WNDCLASSEXW>() as UINT,
                        style: CS_GLOBALCLASS, //| CS_OWNDC | CS_HREDRAW | CS_VREDRAW,
                        lpfnWndProc: Some(Self::windowProc),
                        cbClsExtra: 0,
                        cbWndExtra: 0,
                        hInstance,
                        hIcon,
                        hCursor: LoadCursorW(ptr::null_mut(), IDC_HAND),
                        hbrBackground: ptr::null_mut(), //COLOR_WINDOWFRAME as HBRUSH,
                        lpszMenuName: ptr::null_mut(),
                        lpszClassName: className.as_ptr(),
                        hIconSm: hIcon,
                  };
                  let atom = RegisterClassExW(&windowClass);
                  debug_assert!(atom != 0);
                  let hWindow = CreateWindowExW(
                        0,
                        className.as_ptr(),
                        windowTitle.as_ptr(),
                        params.style,
                        params.position.0,
                        params.position.1,
                        params.width,
                        params.height,
                        ptr::null_mut(),
                        ptr::null_mut(),
                        hInstance,
                        window.as_mut_ptr() as _,
                  );
                  debug_assert!(!hWindow.is_null());
                  hWindow
            };
            let backBuffer = BackBuffer::new(hWindow, params.width, params.height);
            let background = {
                  let bitmap = resources.remove(&BACKGROUND_BITMAP).unwrap() as HBITMAP;
                  Background::new(bitmap)
            };
            let mainHero = {
                  let fore_hbitmap = resources.remove(&HERO_FORE_BITMAP).unwrap() as HBITMAP;
                  let mask_hbitmap = resources.remove(&HERO_MASK_BITMAP).unwrap() as HBITMAP;
                  let center = POINT { x: params.width / 2, y: params.height / 2 };
                  FlyHero::new(center, fore_hbitmap, mask_hbitmap)
            }.unwrap();
            let clientWindow = RECT { left: 0, top: 0, right: params.width, bottom: params.height };
            window.write(Window { hWindow, mainHero, backBuffer, background, clientWindow, isShiftPressed: false });
            unsafe { window.assume_init() }
      }
      fn run(&self) -> Result<WPARAM, ()> {
            let mut msg = MaybeUninit::<MSG>::uninit();
            unsafe {
                  ShowWindow(self.hWindow, SW_SHOW);
                  UpdateWindow(self.hWindow);//invalidate window
                  while GetMessageW(msg.as_mut_ptr(), self.hWindow, 0, 0) > 0 {
                        let msg = msg.assume_init();
                        // TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                  }
                  let msg = msg.assume_init();
                  Ok(msg.wParam)
            }
      }
      extern "system" fn windowProc(hWindow: HWND, message: UINT, wParam: WPARAM, lParam: LPARAM) -> LRESULT {
            unsafe {
                  let mut result: Option<LRESULT> = None;
                  if message != WM_CREATE {
                        let this = GetWindowLongPtrW(hWindow, GWLP_USERDATA) as *mut Self;
                        if !this.is_null() {
                              result = Some((*this).handleWindowMessage(message, wParam, lParam, GET_X_LPARAM!(lParam) as f32));
                        }
                  } else {
                        let createStruct = lParam as *const CREATESTRUCTW;
                        let this = (*createStruct).lpCreateParams as *mut Self;
                        // (*this).hWindow = hWindow; //it's already set to it
                        SetWindowLongPtrW(hWindow, GWLP_USERDATA, this as _);
                        InvalidateRect(hWindow, ptr::null_mut(), TRUE);
                  }
                  if result.is_none() {
                        result = Some(DefWindowProcW(hWindow, message, wParam, lParam));
                  }
                  result.unwrap()
            }
      }
      pub fn handleWindowMessage(&mut self, message: UINT, wParam: WPARAM, lParam: LPARAM, x: f32) -> LRESULT {
            unsafe {
                  match message {
                        WM_PAINT => {
                              let hdcBack = self.backBuffer.getHDC();
                              self.background.draw(&self.clientWindow, hdcBack);
                              self.mainHero.draw(hdcBack);
                              self.moveHero(0.05);
                              let mut paintStruct = MaybeUninit::<PAINTSTRUCT>::uninit();
                              let hdc = BeginPaint(self.hWindow, paintStruct.as_mut_ptr());
                              BitBlt(hdc, 0, 0, self.clientWindow.right, self.clientWindow.bottom, self.backBuffer.hdc, 0, 0, SRCCOPY);
                              EndPaint(self.hWindow, &paintStruct.assume_init());
                              InvalidateRect(self.hWindow, ptr::null_mut(), TRUE);
                              // sleep(time::Duration::from_millis());
                              return 0;
                        }
                        WM_ERASEBKGND => {
                              return 1;
                        }
                        WM_SIZE => {
                              GetClientRect(self.hWindow, &mut self.clientWindow);
                              self.backBuffer.finalize();
                              self.backBuffer = BackBuffer::new(self.hWindow, self.clientWindow.right, self.clientWindow.bottom);//automatically drop last value
                              InvalidateRect(self.hWindow, ptr::null_mut(), TRUE);
                        }
                        WM_MOUSEWHEEL => {
                              if self.isShiftPressed {
                                    let delta = GET_WHEEL_DELTA_WPARAM(wParam);
                                    let keys = GET_KEYSTATE_WPARAM(wParam);
                                    let event = Window::assignMouseMovementEvent(delta as isize, keys as usize);
                                    self.manuallyMoveHero(delta as isize, event);
                                    const MOUSE_MOVEMENT_DELTA: usize = 50;
                                    InvalidateRect(self.hWindow, ptr::null_mut(), TRUE);
                              }
                        }
                        WM_KEYUP => {
                              if wParam == VK_SHIFT as usize {
                                    self.isShiftPressed = false;
                              }
                              return 0;
                        }
                        WM_KEYDOWN => {
                              if wParam == VK_SHIFT as usize || self.isShiftPressed {
                                    self.isShiftPressed = true;
                                    self.mainHero.stop();
                                    return 0;
                              }
                              // show_error_message(&(wParam as usize).to_string());
                              let event = MovementEvent::try_from(wParam as i32);
                              if event.is_ok() {
                                    self.boostHero(event.unwrap());
                                    InvalidateRect(self.hWindow, ptr::null_mut(), TRUE);
                                    // showErrorMessage(&self.mainHero.rect.bottom.to_string());
                              } //else ignore any keyboard input
                              return 0;
                        }
                        WM_LBUTTONDOWN => {
                              if self.isShiftPressed {
                                    return 0;
                              }
                              const IMPULSE_MODULE: f32 = 10.0f32;
                              const MIN_DIRECTION_MODULE: f32 = 15.0f32;
                              let xPos = x;
                              let yPos = GET_Y_LPARAM!(lParam) as f32;
                              let clickedPosition = Vector2 { x: xPos, y: yPos };
                              let direction = clickedPosition.sub_vector(self.mainHero.position());
                              let directionAbs = direction.abs2();
                              if directionAbs >= MIN_DIRECTION_MODULE {
                                    let directionAbs = f32::sqrt(directionAbs);
                                    let impulse = direction.multiply(1.0f32 / directionAbs).multiply(IMPULSE_MODULE);
                                    self.mainHero.boost(impulse);
                              }
                              // onLeftButtonDown(self.hWindow);
                              return 0;
                        }
                        WM_DESTROY => {
                              self.backBuffer.finalize();
                              self.mainHero.finalize();
                              //finalization happens via drop method by out of scope
                              PostQuitMessage(0);
                        }
                        _ => {}
                  }
                  DefWindowProcW(self.hWindow, message, wParam, lParam)
            }
      }
      fn assignMouseMovementEvent(mouse_delta: isize, key_state: usize) -> MovementEvent {
            let events;
            let key_state = unsafe {
                  GetKeyState(VK_CONTROL)
            };
            if key_state < 0 {
                  events = [MovementEvent::Right, MovementEvent::Left];
            } else {
                  events = [MovementEvent::Down, MovementEvent::Up];
            }
            let event;
            if mouse_delta < 0 {
                  event = events[0];
            } else {
                  event = events[1];
            }
            event
      }
      fn manuallyMoveHero(&mut self, wheel_delta: isize, event: MovementEvent) {
            const MOUSE_MOVEMENT_STEP: isize = 50; //the size of mouse step
            // let wheel_delta = (wheel_delta / WHEEL_DELTA as WORD) * MOUSE_MOVEMENT_STEP;
            let delta = ((wheel_delta / WHEEL_DELTA as isize).abs() * MOUSE_MOVEMENT_STEP) as i32;
            let hero_rect = self.mainHero.rect();
            let window_rect = self.clientWindow;
            let x_offset: i32;
            let y_offset: i32;
            match event {
                  MovementEvent::Left => {
                        y_offset = 0;
                        x_offset = -cmp::min((hero_rect.left - window_rect.left), delta);
                  }
                  MovementEvent::Up => {
                        x_offset = 0;
                        y_offset = -cmp::min((hero_rect.top - window_rect.top), delta);
                  }
                  MovementEvent::Right => {
                        y_offset = 0;
                        x_offset = cmp::min((window_rect.right - hero_rect.right), delta);
                  }
                  MovementEvent::Down => {
                        x_offset = 0;
                        y_offset = cmp::min((window_rect.bottom - hero_rect.bottom), delta);
                  }
            }
            self.mainHero.shift(x_offset as isize, y_offset as isize);
      }
      fn moveHero(&mut self, delta: f32) {
            self.burdenHero();
            let hero = &mut self.mainHero;
            if !hero.collides(self.clientWindow) {
                  hero.makeMove(delta);
            } else {
                  hero.quickJump();
                  hero.makeMove(0.3);//too huge delta to prevent following collisions
            }
      }
      const GRAVITY_VECTOR: Vector2 = Vector2 { x: 0.0, y: 10.0 };
      fn boostHero(&mut self, event: MovementEvent) {
            const KICK_VECTORS: [Vector2; 4] = [Vector2::LEFT, Vector2::UP, Vector2::RIGHT, Vector2::DOWN];
            const JUMP_LEN: f32 = 10.0;
            let vectorIndex = event as usize;
            let jumpVector = KICK_VECTORS[vectorIndex].multiply(JUMP_LEN);
            self.mainHero.boost(jumpVector);
      }
      fn burdenHero(&mut self) {
            let velocity = self.mainHero.velocity();
            self.mainHero.boost(velocity.multiply(-0.001));
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
      fn finalize(&mut self) {
            unsafe {
                  RestoreDC(self.hdc, -1);
                  DeleteObject(self.hBitmap as HGDIOBJ);
                  DeleteDC(self.hdc);
            }
      }
}

fn main() {
      let params = FormParams::getDefaultParams();
      let window = Window::new("Cool game", "Cool game", params);
      window.run()
          .expect("Window running failed");
}
