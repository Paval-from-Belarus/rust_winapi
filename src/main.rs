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

use std::error::Error;
use std::mem::MaybeUninit;
use std::ops::Deref;
use winapi::shared::minwindef::*;
use winapi::shared::windef::*;
use winapi::um::libloaderapi::{GetModuleFileNameW, GetModuleHandleW};
use winapi::um::winuser::*;
use std::{mem, ptr};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use winapi::um::wingdi::{BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, RestoreDC, SaveDC, SelectObject, SRCCOPY};
use crate::background::Background;
use crate::hero::FlyHero;
use crate::resources::{BACKGROUND_BITMAP, HERO_BITMAP, load_resources, TITLE_ICON};
use crate::utils::{FormParams, showErrorMessage, Vector2, WindowsString};

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

}

#[derive(Copy, Clone, TryFromPrimitive, IntoPrimitive)]
#[repr(usize)]
pub enum MovementEvent {
      Left = 0,
      Up = 1,
      Right = 2,
      Down = 3,
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
            let mut resources = load_resources(hInstance);
            let hWindow = unsafe {
                  let hIcon = (resources.remove(&TITLE_ICON)).expect("Application icon is not loaded") as HICON;
                  let windowClass = WNDCLASSEXW {
                        cbSize: mem::size_of::<WNDCLASSEXW>() as UINT,
                        style: CS_GLOBALCLASS,// CS_OWNDC | CS_HREDRAW | CS_VREDRAW,
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
                  let bitmap = resources.remove(&HERO_BITMAP).unwrap() as HBITMAP;
                  let center = POINT { x: params.width / 2, y: params.height / 2 };
                  FlyHero::new(center, bitmap)
            }.unwrap();
            let clientWindow = RECT { left: 0, top: 0, right: params.width, bottom: params.height };
            window.write(Window { hWindow, mainHero, backBuffer, background, clientWindow });
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
                              result = Some((*this).handleWindowMessage(message, wParam, lParam));
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
      pub fn handleWindowMessage(&mut self, message: UINT, wParam: WPARAM, lParam: LPARAM) -> LRESULT {
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
                        WM_KEYDOWN => {
                              let eventId = (wParam as usize).saturating_sub(VK_LEFT as usize);
                              let event = MovementEvent::try_from(eventId);
                              if event.is_ok() {
                                    self.boostHero(event.unwrap());
                                    InvalidateRect(self.hWindow, ptr::null_mut(), TRUE);
                                    // showErrorMessage(&self.mainHero.rect.bottom.to_string());
                              } //else ignore any keyboard input
                              return 0;
                        }
                        WM_LBUTTONDOWN => {
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
      fn moveHero(&mut self, delta: f32) {
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
            let vectorIndex: usize = event.into();
            let jumpVector = KICK_VECTORS[vectorIndex].multiply(JUMP_LEN);
            self.mainHero.boost(jumpVector);
      }
      fn burdenHero(&mut self) {
            self.mainHero.boost(Window::GRAVITY_VECTOR);
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
