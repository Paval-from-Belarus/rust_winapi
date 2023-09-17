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
use std::cell::OnceCell;
use std::ffi::c_int;
use std::ops::AddAssign;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use winapi::ctypes::__uint8;
use winapi::um::processthreadsapi::GetStartupInfoW;
use winapi::um::winbase::STARTUPINFOEXW;
use winapi::um::wingdi::{BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, MAKEPOINTS, MAKEROP4, PATCOPY, PATINVERT, RestoreDC, RGB, SaveDC, SelectObject, SRCCOPY};
use winapi::um::winnt::{LONG, LPCWSTR, LPSTR};
use winapi_util::console::Color;
use crate::background::Background;
use crate::hero::FlyHero;

use crate::resources::{BACKGROUND_BITMAP, HERO_FORE_BITMAP, HERO_MASK_BITMAP, load_resources, TITLE_ICON};
use crate::utils::{FormParams, show_error_message, Vector2, WindowsString};


pub struct BackBuffer {
      hdc: HDC,
      hBitmap: HBITMAP,
}

pub struct Window<'a> {
      hWindow: HWND,
      backBuffer: BackBuffer,
      //the window size
      clientWindow: RECT,
      background: Background,
      table: TextTable<'a>,
}

pub struct TextTable<'a> {
      rows: Vec<TextRow>,
      chosen_ceil: Option<&'a mut TextCeil>,
      row_width: usize,
      row_height: usize,
      row_cnt: usize,
      column_cnt: usize,
}

pub struct TextRow {
      row: Vec<TextCeil>,
      max_height: usize,
      // start_x: usize,
      // //the upper lower bound of row
      // start_y: usize,
}

pub struct TextCeil {
      text: Vec<u16>,
      rect: RECT,
      //to draw
      format: UINT,
}

impl<'a> TextTable<'a> {
      ///dimensions are width and height
      pub fn new<'b>(client_window: &'b RECT, row_cnt: usize, column_cnt: usize) -> TextTable<'a> {
            let mut rows = Vec::<TextRow>::with_capacity(row_cnt);
            let table_width = utils::rect_width(client_window) as usize;
            let table_height = utils::rect_height(client_window) as usize;
            let row_height = table_height / row_cnt;
            let row_width = table_width;
            let mut start_y = client_window.top as usize;
            let start_x = client_window.left as usize;
            for _ in 0..row_cnt {
                  let row = TextRow::new((row_width, row_height), start_x, start_y, column_cnt);
                  rows.push(row);
                  start_y += row_height;
            }
            TextTable { rows, row_cnt, column_cnt, row_height, row_width, chosen_ceil: None }
      }
      pub fn draw(&mut self, hdc: HDC) {
            self.rows.iter_mut().for_each(|row| row.draw(hdc));
      }
      pub fn resize(&mut self, client_window: &RECT) {
            let table_height = utils::rect_height(client_window) as usize;
            let table_width = utils::rect_width(client_window) as usize;
            let row_height = table_height / self.row_cnt;
            let row_width = table_width;
            let mut start_y = client_window.top as usize;
            let start_x = client_window.left as usize;
            for row in self.rows.iter_mut() {
                  row.resize((row_width, row_height), start_x, start_y);
                  start_y += row_height;
            }
            self.row_height = row_height;
            self.row_width = row_width;
      }
      pub fn handle_click(&'a mut self, x: LONG, y: LONG) {
            let column_offset = x as usize / self.row_width;
            let row_offset = y as usize / self.row_height;
            debug_assert!(self.rows.len() > row_offset);
            let row = self.rows.get_mut(row_offset).unwrap();
            let ceil = row.ceil(column_offset).unwrap();
            if let Some(last_chosen) = &mut self.chosen_ceil {
                  last_chosen.text = String::from("A").as_os_str();
            }
            ceil.text = String::from("B").as_os_str();
            self.chosen_ceil = Some(ceil);
      }
}

impl TextRow {
      const DEFAULT_CEIL_FORMAT: UINT = DT_CENTER;
      ///dimension are width and height corresponding
      pub fn new(dimensions: (usize, usize), start_x: usize, start_y: usize, column_cnt: usize) -> TextRow {
            debug_assert!(column_cnt >= 1);
            let ceil_width = (dimensions.0 / column_cnt);
            let ceil_height = (dimensions.1);
            let mut ceil_rect = RECT {
                  left: start_x as LONG,
                  top: start_y as LONG,
                  right: (start_x + ceil_width) as LONG,
                  bottom: (start_y + ceil_height) as LONG,
            };
            let mut row = Vec::<TextCeil>::with_capacity(column_cnt);
            for _ in 0..column_cnt {
                  let ceil = TextCeil::new(ceil_rect.clone(), TextRow::DEFAULT_CEIL_FORMAT);
                  row.push(ceil);
                  utils::offset_rect(&mut ceil_rect, ceil_width as INT, 0);
            }
            TextRow { row, max_height: ceil_height }
      }
      ///as always the first element of dimension is width, the second is height
      pub fn resize(&mut self, dimensions: (usize, usize), start_x: usize, start_y: usize) {
            let ceil_width = (dimensions.0 / self.row.len());
            let ceil_height = dimensions.1;
            let mut ceil_rect = RECT {
                  left: start_x as LONG,
                  right: (start_x + ceil_width) as LONG,
                  top: start_y as LONG,
                  bottom: (start_y + ceil_height) as LONG,
            };
            for ceil in self.row.iter_mut() {
                  utils::copy_rect(&mut ceil.rect, &ceil_rect);
                  utils::offset_rect(&mut ceil_rect, ceil_width as INT, 0);
            }
      }
      pub fn shift(&mut self, delta_x: isize, delta_y: isize) {
            // self.start_x += delta_x;
            // self.start_y += delta_y;
            for ceil in self.row.iter_mut() {
                  utils::offset_rect(&mut ceil.rect, delta_x as INT, delta_y as INT);
            }
      }
      ///return current row height
      pub fn shrink(&mut self, height: usize) -> usize {
            if self.max_height >= height { //do nothing where row is already huge
                  return self.max_height;
            }
            let max_height = self.row.iter()
                .map(|ceil| ceil.height())
                .max().unwrap();
            self.max_height = max_height;
            self.row.iter_mut()
                .for_each(|ceil| ceil.set_height(max_height));
            self.max_height
      }
      pub fn set_format(&mut self, format: UINT) {
            self.row.iter_mut().for_each(|ceil| ceil.set_format(format));
      }
      pub fn draw(&mut self, hdc: HDC) {
            self.row.iter_mut().for_each(|ceil| ceil.draw_text(hdc));
      }
      pub fn ceil(&mut self, ceil_index: usize) -> Option<&mut TextCeil> {
            self.row.get_mut(ceil_index)
      }
}

impl TextCeil {
      pub fn new(rect: RECT, format: UINT) -> TextCeil {
            let text = String::from("A").as_os_str();
            TextCeil { rect, format, text }
      }
      pub fn draw_text(&mut self, hdc: HDC) {
            unsafe {
                  DrawTextW(hdc, self.text.as_ptr() as LPCWSTR, self.text.len() as INT, &mut self.rect as _, self.format);
            }
      }
      pub fn height(&self) -> usize {//ceil supports such invariant that height is only positive
            let rect = self.rect;
            (rect.bottom - rect.top) as usize
      }
      pub fn set_format(&mut self, format: UINT) {
            self.format = format;
      }
      pub fn set_height(&mut self, height: usize) {
            self.rect.bottom = self.rect.top + height as LONG;
      }
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

impl<'a> Window<'a> {
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
            let clientWindow = RECT { left: 0, top: 0, right: params.width, bottom: params.height };
            let background = Background::solid(RGB(100, 200, 50));
            let table = TextTable::new(&clientWindow, 12, 10);
            window.write(Window { hWindow, backBuffer, clientWindow, background, table });
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
                  }
                  if result.is_none() {
                        result = Some(DefWindowProcW(hWindow, message, wParam, lParam));
                  }
                  result.unwrap()
            }
      }
      pub fn handleWindowMessage(&'a mut self, message: UINT, wParam: WPARAM, lParam: LPARAM) -> LRESULT {
            unsafe {
                  match message {
                        WM_PAINT => {
                              let hdcBack = self.backBuffer.getHDC();
                              self.background.draw(&self.clientWindow, hdcBack);
                              self.table.draw(hdcBack);
                              let mut paintStruct = MaybeUninit::<PAINTSTRUCT>::uninit();
                              let hdc = BeginPaint(self.hWindow, paintStruct.as_mut_ptr());
                              BitBlt(hdc, 0, 0, self.clientWindow.right, self.clientWindow.bottom, hdcBack, 0, 0, SRCCOPY);
                              EndPaint(self.hWindow, &paintStruct.assume_init());
                              InvalidateRect(self.hWindow, ptr::null_mut(), TRUE);
                              return 0;
                        }
                        WM_ERASEBKGND => {
                              return TRUE as LRESULT;
                        }
                        WM_SIZE => {}
                        WM_MOUSEWHEEL => {}
                        WM_KEYUP => {}
                        WM_KEYDOWN => {}
                        WM_LBUTTONDOWN => {
                              let x = GET_X_LPARAM!(lParam);
                              let y = GET_Y_LPARAM!(lParam);
                              self.table.handle_click(x, y);
                        }
                        WM_DESTROY => {
                              self.backBuffer.finalize();
                              self.background.finalize();
                              //finalization happens via drop method by out of scope
                              PostQuitMessage(0);
                        }
                        _ => {}
                  }
                  DefWindowProcW(self.hWindow, message, wParam, lParam)
            }
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
