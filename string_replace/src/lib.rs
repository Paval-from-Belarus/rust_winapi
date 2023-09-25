use std::ffi::{c_char, CString};
use std::mem::MaybeUninit;
use winapi::shared::minwindef::HINSTANCE;
use winapi::um::sysinfoapi::{GetSystemInfo, SYSTEM_INFO};
use winapi::um::winnt::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH, HANDLE, INT};
use utils::StringSearchParams;

#[allow(non_snake_case, unused_variables)]
extern crate utils;

#[no_mangle]
extern "system" fn DllMain(dll_module: HINSTANCE, call_reason: u32, _: *mut ()) -> bool {
      match call_reason {
            DLL_PROCESS_ATTACH => attach(),
            DLL_PROCESS_DETACH => detach(),
            _ => ()
      }
      true
}

fn get_system_info() -> SYSTEM_INFO {
      let mut buffer = MaybeUninit::<SYSTEM_INFO>::uninit();
      unsafe {
            GetSystemInfo(buffer.as_mut_ptr())
      };
      return unsafe {
            buffer.assume_init()
      };
}

#[no_mangle]
pub extern fn replace(params: *const StringSearchParams) -> INT {
      let system_info = get_system_info();
      0
}

fn attach() {
      utils::show_error_message("Attached");
}

fn detach() {
      utils::show_error_message("Dettached");
}

#[cfg(test)]
mod tests {
      use super::*;

      #[test]
      fn it_works() {
            // let result = add(2, 2);
            // assert_eq!(result, 4);
      }
}
