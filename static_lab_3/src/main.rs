#![cfg(windows)]
// Let's put this so that it won't open the console
#![windows_subsystem = "windows"]
#![allow(non_snake_case)]

extern crate core;
extern crate utils;


use winapi::shared::minwindef::*;
use std::{ptr};
use winapi::um::processthreadsapi::{GetCurrentProcess};
use utils::{StringSearchParams};

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


pub fn run() {
      let hacker_string = b"HACKER\0";
      let tail_string = b"_STRING\0";//common for each
      let mut params = StringSearchParams {
            szSearch: ptr::null_mut(),
            cbSearchLen: 0,
            szReplace: hacker_string.as_ptr() as _,
            cbReplaceLen: hacker_string.len()
      };
      let static_string = b"STATIC_STRING\0";//pure CString
      utils::show_alert_message("Static replacement. Origin string: ", String::from_utf8(static_string.to_vec()).unwrap().as_str());
      params.szSearch = static_string.as_ptr() as _;
      params.cbSearchLen = static_string.len();
      if let Err(error_code) = replace_string(&params) {
            utils::show_error_message_with_error_code("Static replacement failed with error code ", error_code);
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
            utils::show_error_message_with_error_code("Heap replacement failed with error code ", error_code);
      } else {
            utils::show_alert_message("Finished successfully. Result string: ", String::from_utf8(heap_string.clone()).unwrap().as_str());
      }
      let stack_string = [b'S', b'T', b'A', b'C', b'K', b'_', b'S', b'T', b'R', b'I', b'N', b'G', 0];
      utils::show_alert_message("Stack replacement. Origin string: ", String::from_utf8(stack_string.to_vec()).unwrap().as_str());
      params.szSearch = stack_string.as_ptr() as _;
      params.cbSearchLen = stack_string.len();
      if let Err(error_code) = replace_string(&params) {
            utils::show_error_message_with_error_code("Stack replacement failed with error code ", error_code);
      } else {
            utils::show_alert_message("Finished successfully. Result string: ", String::from_utf8(stack_string.to_vec()).unwrap().as_str());
      }
}

fn main() {
      run();
      // let params = FormParams::getDefaultParams();
      // let (window, hWindow) = Window::new("Cool game", "Cool game", params);
      // window.run(hWindow)
      //     .expect("Window running failed");
}
