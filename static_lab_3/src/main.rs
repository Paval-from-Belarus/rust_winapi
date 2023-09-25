#![cfg(windows)]
// Let's put this so that it won't open the console
#![windows_subsystem = "windows"]
#![allow(non_snake_case)]

extern crate core;
extern crate utils;


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
use utils::{BackBuffer, FormParams, WindowsString, GET_X_LPARAM, GET_Y_LPARAM, StringSearchParams};

#[link(name = "string_replace.dll", kind = "dylib")]
extern {
      fn replace(params: *const StringSearchParams) -> INT;
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