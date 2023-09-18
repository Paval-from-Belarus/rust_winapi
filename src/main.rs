#![feature(new_uninit)]
#![feature(let_chains)]
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
mod table;


use std::mem::MaybeUninit;

use winapi::shared::minwindef::*;
use winapi::shared::windef::*;
use winapi::um::libloaderapi::{GetModuleFileNameW, GetModuleHandleW};
use winapi::um::winuser::*;
use std::{cmp, mem, ptr};
use std::borrow::Cow;
use std::cell::OnceCell;
use std::convert::Into;
use std::ffi::c_int;
use std::ops::AddAssign;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use winapi::ctypes::__uint8;
use winapi::um::errhandlingapi::GetLastError;
use winapi::um::processthreadsapi::GetStartupInfoW;
use winapi::um::winbase::{COPYFILE2_MESSAGE_Error, STARTUPINFOEXW};
use winapi::um::wingdi::{BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, MAKEPOINTS, MAKEROP4, PATCOPY, PATINVERT, PS_SOLID, Rectangle, RestoreDC, RGB, SaveDC, SelectObject, SRCCOPY};
use winapi::um::winnt::{LONG, LPCWSTR, LPSTR};
use winapi_util::console::Color;
use crate::background::Background;
use crate::hero::FlyHero;

use crate::resources::{load_resources, TITLE_ICON};
use crate::table::TextTable;
use crate::utils::{BackBuffer, FormParams, WindowsString};


pub struct Window {
      back_buffer: Option<BackBuffer>,
      //can be initialized only with hWindow
      //the window size
      client_window: RECT,
      background: Background,
      table: TextTable,
}

impl Window {
      //this function
      pub fn new(className: &str, windowTitle: &str, params: FormParams) -> (Box<Window>, HWND) {
            let className = className.as_os_str();
            let windowTitle = windowTitle.as_os_str();
            let hInstance = unsafe {
                  GetModuleHandleW(ptr::null_mut())
            };
            let mut resources = load_resources(hInstance).expect("Failed to load resources");
            let background = Background::solid(RGB(100, 200, 50));
            let client_window = params.get_client_window();
            let table = TextTable::new(&client_window, 12, 10);
            let mut window = Box::new(Window {
                  client_window,
                  background,
                  table,
                  back_buffer: None, //back_buffer will be initialized latter in WM_SIZE
            });
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
                        window.as_mut() as *mut Window as _,
                  );
                  debug_assert!(!hWindow.is_null());
                  hWindow
            };
            (window, hWindow)
      }
      fn run(&self, hWindow: HWND) -> Result<WPARAM, ()> {
            let mut msg = MaybeUninit::<MSG>::uninit();
            unsafe {
                  ShowWindow(hWindow, SW_SHOW);
                  UpdateWindow(hWindow);//invalidate window
                  while GetMessageW(msg.as_mut_ptr(), hWindow, 0, 0) > 0 {
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
                              result = Some((*this).handleWindowMessage(hWindow, message, wParam, lParam));
                        }
                  } else {
                        let createStruct = lParam as *const CREATESTRUCTW;
                        let this = (*createStruct).lpCreateParams as *mut Self;
                        // (*this).hWindow = hWindow; //it's already set to it
                        SetWindowLongPtrW(hWindow, GWLP_USERDATA, this as _);
                  }
                  if result.is_none() {
                        result = Some(DefWindowProcW(hWindow, message, wParam, lParam));
                  }
                  result.unwrap()
            }
      }
      pub fn handleWindowMessage(&mut self, hWindow: HWND, message: UINT, wParam: WPARAM, lParam: LPARAM) -> LRESULT {
            unsafe {
                  match message {
                        WM_PAINT => {
                              if let Some(back_buffer) = &self.back_buffer {
                                    let hdc_back = back_buffer.getHDC();
                                    self.background.draw(&self.client_window, hdc_back);
                                    self.table.draw(hdc_back);
                                    let mut paint_struct = MaybeUninit::<PAINTSTRUCT>::uninit();
                                    let hdc = BeginPaint(hWindow, paint_struct.as_mut_ptr());
                                    BitBlt(hdc, 0, 0, self.client_window.right, self.client_window.bottom, hdc_back, 0, 0, SRCCOPY);
                                    EndPaint(hWindow, &paint_struct.assume_init());
                                    InvalidateRect(hWindow, ptr::null_mut(), TRUE);
                              }
                              return 0;
                        }
                        WM_ERASEBKGND => {
                              return TRUE as LRESULT;
                        }
                        WM_SIZE => {
                              let mut rect = MaybeUninit::<RECT>::uninit();
                              let result = GetClientRect(hWindow, rect.as_mut_ptr());
                              if result != FALSE {
                                    let rect = rect.assume_init();
                                    self.table.resize(&rect);
                                    self.client_window = rect;
                                    self.back_buffer = Some(BackBuffer::new(hWindow, utils::rect_width(&rect), utils::rect_height(&rect)));
                                    InvalidateRect(hWindow, ptr::null_mut(), TRUE);
                              } else {
                                    utils::show_error_message("invalid sizing");
                              }

                              // unsafe {
                              //       GetClientRect(hWindow, &mut self.clientWindow);
                              //       let error_code = GetLastError();
                              //       utils::show_error_message(&("Error with code".to_owned() + &error_code.to_string()));
                              // }
                              return 0;
                        }
                        WM_LBUTTONDOWN => {
                              let x = GET_X_LPARAM!(lParam);
                              let y = GET_Y_LPARAM!(lParam);
                              self.table.handle_click(x, y);
                              return 0;
                        }
                        WM_SETFOCUS => {

                              CreateCaret(hWindow, 1 as HBITMAP, 1, 0);

                        }
                        WM_DESTROY => {
                              self.background.finalize();
                              self.table.finalize();
                              //finalization happens via drop method by out of scope
                              PostQuitMessage(0);
                        }
                        _ => {}
                  }
                  DefWindowProcW(hWindow, message, wParam, lParam)
            }
      }
}


fn main() {
      let params = FormParams::getDefaultParams();
      let (window, hWindow) = Window::new("Cool game", "Cool game", params);
      window.run(hWindow)
          .expect("Window running failed");
}
