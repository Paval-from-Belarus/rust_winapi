#![cfg(windows)]
// Let's put this so that it won't open the console
#![windows_subsystem = "windows"]
#![allow(non_snake_case)]

extern crate core;
extern crate utils;

mod resources;
mod hero;
mod background;
mod table;

use std::mem::MaybeUninit;

use winapi::shared::minwindef::*;
use winapi::shared::windef::*;
use winapi::um::libloaderapi::{GetModuleFileNameW, GetModuleHandleW};
use winapi::um::winuser::*;
use std::{cmp, isize, mem, ptr};
use std::borrow::Cow;
use std::cell::OnceCell;
use std::convert::Into;
use std::ffi::{c_char, c_int, CString};
use std::ops::AddAssign;
use std::os::windows::raw::HANDLE;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use winapi::ctypes::__uint8;
use winapi::shared::winerror::TRUST_E_ACTION_UNKNOWN;
use winapi::um::errhandlingapi::GetLastError;
use winapi::um::processthreadsapi::{GetCurrentProcess, GetStartupInfoW};
use winapi::um::winbase::{COPYFILE2_MESSAGE_Error, STARTUPINFOEXW};
use winapi::um::wingdi::{BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetTextMetricsW, MAKEPOINTS, MAKEROP4, PATCOPY, PATINVERT, PS_SOLID, Rectangle, RestoreDC, RGB, SaveDC, SelectObject, SRCCOPY, TEXTMETRICW};
use winapi::um::winnt::{LONG, LPCWSTR, LPSTR, SCRUB_DATA_INPUT};
use winapi_util::console::Color;
use crate::background::Background;
use crate::hero::FlyHero;

use crate::resources::{load_resources, TITLE_ICON};
use crate::table::TextTable;
use utils::{BackBuffer, FormParams, WindowsString, GET_X_LPARAM, GET_Y_LPARAM, StringSearchParams};

#[link(name = "string_replace.dll", kind = "dylib")]
extern {
      fn replace(params: *const StringSearchParams) -> INT;
}

pub enum ScrollEvent {
      Up(INT),
      Down(INT),
}

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
            let background = Background::solid(RGB(100, 200, 50), params.width, params.height);
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
                        TranslateMessage(&msg);
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
                        SetWindowLongPtrW(hWindow, GWLP_USERDATA, this as _);
                        (*this).init_text_metrics(hWindow);
                  }
                  if result.is_none() {
                        result = Some(DefWindowProcW(hWindow, message, wParam, lParam));
                  }
                  result.unwrap()
            }
      }
      unsafe fn init_scroll_info(&mut self, hWindow: HWND) {
            let scroll_info = SCROLLINFO {
                  cbSize: mem::size_of::<SCROLLINFO>() as UINT,
                  fMask: SIF_RANGE | SIF_PAGE,
                  nMin: 0,
                  nMax: 12,
                  nPage: 750 / 16,
                  nPos: 0,
                  nTrackPos: 0,
            };
            let result = SetScrollInfo(hWindow, SB_VERT as INT, &scroll_info, TRUE);
            println!("{}", result);
      }
      unsafe fn init_text_metrics(&mut self, hWindow: HWND) {
            let mut metrics = MaybeUninit::<TEXTMETRICW>::uninit();
            let hdc = GetDC(hWindow);
            GetTextMetricsW(hdc, metrics.as_mut_ptr());
            ReleaseDC(hWindow, hdc);
            ;
            let metrics = metrics.assume_init();
            self.table.set_char_properties(metrics.tmAveCharWidth, metrics.tmHeight + metrics.tmExternalLeading);
      }
      pub fn handleWindowMessage(&mut self, hWindow: HWND, message: UINT, wParam: WPARAM, lParam: LPARAM) -> LRESULT {
            unsafe {
                  static mut Y_CURRENT_SCROLL: isize = 0;
                  static mut Y_MAX_SCROLL: isize = 0;
                  match message {
                        WM_PAINT => {
                              if let Some(back_buffer) = &self.back_buffer {
                                    let hdc_back = back_buffer.getHDC();
                                    self.background.draw(hdc_back);
                                    self.table.draw(hdc_back);
                                    let mut paint_struct = MaybeUninit::<PAINTSTRUCT>::uninit();
                                    let hdc = BeginPaint(hWindow, paint_struct.as_mut_ptr());
                                    BitBlt(hdc, 0, 0, self.client_window.right, self.client_window.bottom, hdc_back, 0, Y_CURRENT_SCROLL as INT, SRCCOPY);
                                    EndPaint(hWindow, &paint_struct.assume_init());
                              }
                              return 0;
                        }
                        WM_VSCROLL => {
                              let x_delta = 0;
                              let mut y_delta = 0;
                              let mut y_new_pos = Y_CURRENT_SCROLL;
                              match LOWORD(wParam as DWORD) as isize {
                                    SB_PAGEUP => {
                                          y_new_pos = Y_CURRENT_SCROLL - 30;
                                    }
                                    SB_PAGEDOWN => {
                                          y_new_pos = Y_CURRENT_SCROLL + 30;
                                    }
                                    SB_LINEUP => {
                                          y_new_pos = Y_CURRENT_SCROLL - 10;
                                    }
                                    SB_LINEDOWN => {
                                          y_new_pos = Y_CURRENT_SCROLL + 10;
                                    }
                                    SB_THUMBPOSITION => {
                                          y_new_pos = HIWORD(wParam as DWORD) as isize;
                                    }
                                    _ => {
                                          y_new_pos = Y_CURRENT_SCROLL;
                                    }
                              }
                              y_new_pos = isize::max(0, y_new_pos);
                              y_new_pos = isize::min(y_new_pos, Y_MAX_SCROLL);
                              if y_new_pos == Y_CURRENT_SCROLL {
                                    return DefWindowProcA(hWindow, message, wParam, lParam);
                              }
                              y_delta = y_new_pos - Y_CURRENT_SCROLL;
                              Y_CURRENT_SCROLL = y_new_pos;
                              print!("scroll_pos: {}", Y_CURRENT_SCROLL);
                              ScrollWindowEx(hWindow, -x_delta, (-y_delta) as i32, ptr::null_mut(), ptr::null_mut(),
                                             ptr::null_mut(), ptr::null_mut(), SW_INVALIDATE);
                              let mut scrollInfo = MaybeUninit::<SCROLLINFO>::zeroed().assume_init();
                              scrollInfo.cbSize = mem::size_of::<SCROLLINFO>() as UINT;
                              scrollInfo.fMask = SIF_POS;
                              scrollInfo.nPos = Y_CURRENT_SCROLL as INT;
                              SetScrollInfo(hWindow, SB_VERT as INT, &scrollInfo, TRUE);
                              InvalidateRect(hWindow, ptr::null_mut(), TRUE);
                        }
                        WM_ERASEBKGND => {
                              return TRUE as LRESULT;
                        }
                        WM_GETMINMAXINFO => {
                              let min_max_info = &mut (*(lParam as *mut MINMAXINFO));
                              min_max_info.ptMinTrackSize.x = 200;
                              min_max_info.ptMinTrackSize.y = 200;
                        }
                        WM_SIZE => {
                              let mut rect = MaybeUninit::<RECT>::uninit();
                              let result = GetClientRect(hWindow, rect.as_mut_ptr());
                              if result != FALSE {
                                    let rect = rect.assume_init();
                                    let hdc = GetDC(hWindow);
                                    self.table.resize(hdc, &rect);
                                    ReleaseDC(hWindow, hdc);
                                    let table_height = self.table.height();
                                    self.client_window = rect;
                                    self.back_buffer = Some(BackBuffer::new(hWindow, utils::rect_width(&rect), table_height as INT));
                                    self.background.resize(utils::rect_width(&rect), table_height as LONG);
                                    let window_height = utils::rect_height(&rect) as isize;
                                    Y_MAX_SCROLL = isize::max((table_height as isize) - window_height, 0);
                                    Y_CURRENT_SCROLL = isize::min(Y_CURRENT_SCROLL, Y_MAX_SCROLL);
                                    Window::update_scroll_info(hWindow, Y_CURRENT_SCROLL as INT, window_height as UINT, self.table.height() as INT);
                                    // println!("table_height: {}", self.table.height());
                                    InvalidateRect(hWindow, ptr::null_mut(), TRUE);
                              } else {
                                    utils::show_error_message("invalid sizing");
                              }
                              return 0;
                        }
                        WM_LBUTTONDOWN => {
                              let x = GET_X_LPARAM!(lParam);
                              let y = GET_Y_LPARAM!(lParam) + Y_CURRENT_SCROLL as LONG;
                              // utils::show_error_message(&(x.to_string() + &" <->" + &y.to_string()));
                              self.table.handle_click(x, y);
                              InvalidateRect(hWindow, ptr::null_mut(), TRUE);
                              return 0;
                        }
                        WM_CHAR => {
                              let old_table_height = self.table.height();
                              let hdc = GetDC(hWindow);
                              self.table.handle_type(hdc, wParam as INT);
                              ReleaseDC(hWindow, hdc);
                              if old_table_height != self.table.height() {
                                    let window_height = utils::rect_height(&self.client_window) as isize;
                                    let table_height = self.table.height();
                                    Y_MAX_SCROLL = isize::max((table_height as isize) - window_height, 0);
                                    Y_CURRENT_SCROLL = isize::min(Y_CURRENT_SCROLL, Y_MAX_SCROLL);
                                    Window::update_scroll_info(hWindow, Y_CURRENT_SCROLL as INT, window_height as UINT, table_height as INT);
                                    self.back_buffer = Some(BackBuffer::new(hWindow, utils::rect_width(&self.client_window), table_height as INT));
                                    self.background.resize(utils::rect_width(&self.client_window), table_height as LONG);
                                    // let lParam = (window_height << 16) | (utils::rect_width(&self.client_window) as isize);
                                    // SendMessageW(hWindow, WM_SIZE, SIZE_RESTORED, lParam);
                              }
                              InvalidateRect(hWindow, ptr::null_mut(), TRUE);
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
      fn update_scroll_info(hWindow: HWND, current_scroll_offset: INT, window_height: UINT, table_height: INT) {
            let scroll_info = SCROLLINFO {
                  cbSize: mem::size_of::<SCROLLINFO>() as UINT,
                  fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
                  nMin: 0,
                  nMax: table_height,
                  nPage: window_height,
                  nPos: current_scroll_offset,
                  nTrackPos: 0,
            };
            unsafe {
                  SetScrollInfo(hWindow, SB_VERT as INT, &scroll_info, TRUE);
            }
      }
}

fn replace_string(params: &StringSearchParams) -> Result<(), INT> {
      let result = unsafe {
            replace(params)
      };
      if result == 0 {
            Ok(())
      } else {
            Err(result)
      }
}

fn show_error_message_with_error_code(message: &str, error_code: INT) {
      let mut description = message.to_owned();
      description.push_str(error_code.to_string().as_str());
      utils::show_error_message(description.as_str());
}

pub fn run() {
      let handle = unsafe {
            GetCurrentProcess()
      };
      debug_assert!(!handle.is_null());
      let hacker_string = b"HACKER\0";
      let tail_string = b"_STRING\0";//common for each
      let mut params = StringSearchParams {
            szSearch: ptr::null_mut(),
            cbSearchLen: 0,
            szReplace: hacker_string.as_ptr() as _,
            cbReplaceLen: hacker_string.len(),
            hProcess: handle,
      };
      let static_string = b"STATIC_STRING\0";//pure CString
      utils::show_alert_message("Static replacement. Origin string: ", String::from_utf8(static_string.to_vec()).unwrap().as_str());
      params.szSearch = static_string.as_ptr() as _;
      params.cbSearchLen = static_string.len();
      if let Err(error_code) = replace_string(&params) {
            show_error_message_with_error_code("Static replacement failed with error code ", error_code);
      } else {
            utils::show_alert_message("Finished successfully. Result string: ", String::from_utf8(static_string.to_vec()).unwrap().as_str());
      }
      let mut heap_string = vec!(b'H', b'E', b'A', b'P');
      tail_string.iter()
          .for_each(|letter| heap_string.push(*letter));
      params.szSearch = heap_string.as_ptr() as _;
      params.cbSearchLen = heap_string.len();
      utils::show_alert_message("Heap replacement. Origin string: ", String::from_utf8(heap_string.clone()).unwrap().as_str());
      if let Err(error_code) = replace_string(&params) {
            show_error_message_with_error_code("Heap replacement failed with error code ", error_code);
      } else {
            utils::show_alert_message("Finished successfully. Result string: ", String::from_utf8(heap_string.clone()).unwrap().as_str());
      }
      let stack_string = [b'S', b'T', b'A', b'C', b'K', b'_', b'S', b'T', b'R', b'I', b'N', b'G', 0];
      utils::show_alert_message("Stack replacement. Origin string: ", String::from_utf8(stack_string.to_vec()).unwrap().as_str());
      params.szSearch = stack_string.as_ptr() as _;
      params.cbSearchLen = stack_string.len();
      if let Err(error_code) = replace_string(&params) {
            show_error_message_with_error_code("Stack replacement failed with error code ", error_code);
      } else {
            utils::show_alert_message("Finished successfully. Result string: ", String::from_utf8(heap_string.clone()).unwrap().as_str());
      }
}

fn main() {
      run();
      // let params = FormParams::getDefaultParams();
      // let (window, hWindow) = Window::new("Cool game", "Cool game", params);
      // window.run(hWindow)
      //     .expect("Window running failed");
}
