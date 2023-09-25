#![feature(pointer_byte_offsets)]
#![feature(let_chains)]
#![feature(slice_pattern)]

use std::arch::x86_64::_mm256_fmsubadd_pd;
use std::ffi::{c_char, CStr, CString};
use std::{mem, ptr};
use std::mem::MaybeUninit;
use std::ops::Index;
use num_enum::IntoPrimitive;
use winapi::shared::minwindef::{DWORD, HINSTANCE, LPVOID};
use winapi::um::sysinfoapi::{GetSystemInfo, SYSTEM_INFO};
use winapi::um::winbase::lstrlenA;
use winapi::um::memoryapi::{VirtualQueryEx};
use winapi::um::winnt::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH, HANDLE, INT, MEM_COMMIT, MEMORY_BASIC_INFORMATION, PAGE_EXECUTE_READWRITE, PAGE_GUARD, PAGE_READWRITE};
use winapi_util::console::Intense::No;
use utils::StringSearchParams;
use crate::SearchResult::{Found, InvalidParams, InvalidStringValues, NotFound};

#[allow(non_snake_case, unused_variables)]
extern crate utils;
extern crate core;

#[no_mangle]
extern "system" fn DllMain(dll_module: HINSTANCE, call_reason: u32, _: *mut ()) -> bool {
      match call_reason {
            DLL_PROCESS_ATTACH => attach(),
            DLL_PROCESS_DETACH => detach(),
            _ => ()
      }
      true
}

#[derive(Copy, Clone, IntoPrimitive)]
#[repr(usize)]
pub enum SearchResult {
      Found = 0,
      NotFound = 1,
      InvalidParams = 2,
      InvalidStringValues = 3,
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

fn replace_as_pattern(replaceable: &mut [u8], pattern: &[u8]) {
      let min_length = usize::min(replaceable.len(), pattern.len());
      for i in 0..min_length {
            replaceable[i] = pattern[i];
      }
}

fn is_accessible_memory(memory_info: MEMORY_BASIC_INFORMATION) -> bool {
      let protect = memory_info.Protect;
      let is_read_write_accessible =
          (protect & PAGE_READWRITE) == PAGE_READWRITE ||
          (protect & PAGE_EXECUTE_READWRITE) == PAGE_EXECUTE_READWRITE;
      let is_not_guard_page = (protect & PAGE_GUARD) != PAGE_GUARD;
      return is_read_write_accessible && memory_info.State == MEM_COMMIT && is_not_guard_page;
}

fn find_string_in_range(pattern: &[u8], base_offset: *const u8, limit_offset: *const u8) -> Option<&mut [u8]> {
      debug_assert!(!base_offset.is_null() && !limit_offset.is_null());
      let mut byte_offset = base_offset;
      let mut pivot = 0;
      while byte_offset < limit_offset && pivot < pattern.len() {
            // if ptr::eq(pattern.as_ptr(), byte_offset) {
            //     byte_offset = unsafe {byte_offset.byte_add(pattern.len())};
            //     pivot = 0;
            //     continue;
            // }
            let letter = unsafe { byte_offset.read() };
            if letter == pattern[pivot] {
                  pivot += 1;
            } else {
                  pivot = 0;
            }
            byte_offset = unsafe { byte_offset.byte_add(1) };
      }
      let result;
      if byte_offset < limit_offset && pivot == pattern.len() {
            let byte_offset = unsafe { byte_offset.offset(-(pattern.len() as isize)) } as *mut u8;
            let bytes = unsafe { &mut *ptr::slice_from_raw_parts_mut(byte_offset, pattern.len()) };
            result = Some(bytes);
      } else {
            result = None;
      }
      return result;
}

fn find_string(pattern: &[u8], process_handle: HANDLE) -> Option<&mut [u8]> { //return found CString
      let system_info = get_system_info();
      let mut memory_info = MaybeUninit::<MEMORY_BASIC_INFORMATION>::uninit();
      let mut base_offset = system_info.lpMinimumApplicationAddress;
      let mut search_result = None;
      while search_result.is_none() && base_offset < system_info.lpMaximumApplicationAddress {
            let query_result: usize = unsafe {
                  VirtualQueryEx(process_handle,
                                 base_offset,
                                 memory_info.as_mut_ptr(),
                                 mem::size_of::<MEMORY_BASIC_INFORMATION>())
            };
            if query_result == mem::size_of::<MEMORY_BASIC_INFORMATION>() {
                  let memory_info = unsafe { memory_info.assume_init() };
                  let limit_offset = unsafe { base_offset.byte_add(memory_info.RegionSize) };
                  if is_accessible_memory(memory_info) && let Some(found_string) = find_string_in_range(&pattern, base_offset as _, limit_offset as _) {
                        search_result = Some(found_string);
                  }
                  base_offset = limit_offset;
            } else {
                  return None;//we already exceed impossible conditions
            }
      }
      return search_result;
}

const MAX_STRING_SIZE: usize = 255;

fn is_valid_params(params: &StringSearchParams) -> bool {
      let is_valid;
      if !params.szReplace.is_null() && !params.szSearch.is_null() && !params.hProcess.is_null() {
            is_valid = params.cbReplaceLen <= MAX_STRING_SIZE && params.cbSearchLen <= MAX_STRING_SIZE;
      } else {
            is_valid = false;
      }
      return is_valid;
}

#[no_mangle]
pub extern fn replace(params: *const StringSearchParams) -> INT {
      if params.is_null() {
            return InvalidParams as INT;
      }
      let params = unsafe { &*params };
      let mut result_code: SearchResult;
      if is_valid_params(params) {
            let search_pattern = unsafe { CStr::from_ptr(params.szSearch) }.to_bytes();
            let replace_pattern = unsafe { CStr::from_ptr(params.szReplace) }.to_bytes();
            if search_pattern.len() > 0 && replace_pattern.len() > 0 {
                  let found_pattern = find_string(search_pattern, params.hProcess);
                  if let Some(replaceable) = found_pattern {
                        result_code = Found;
                        replace_as_pattern(replaceable, replace_pattern);
                  } else {
                        result_code = NotFound;
                  }
            } else {
                  result_code = InvalidStringValues;
            }
      } else {
            result_code = InvalidParams;
      }
      return result_code as INT;
}

fn attach() {
      utils::show_error_message("Attached");
}

fn detach() {
      utils::show_error_message("Dettached");
}

#[cfg(test)]
mod tests {
      use core::slice::{SlicePattern};
      use super::*;

      #[test]
      fn find_string_in_range_test() {
            let bytes = [1, 2, 3, 4, 5, 1, 2, 4, 5];
            let pattern: [u8; 3] = [1, 2, 4];
            let found_pattern = find_string_in_range(
                  pattern.as_slice(), bytes.as_ptr(), unsafe { bytes.as_ptr().add(bytes.len()) },
            );
            assert_eq!(found_pattern.is_some() && found_pattern.unwrap().eq(pattern.as_slice()));
            let pattern = [1, 4, 6];
            let found_pattern = find_string_in_range(
                  pattern.as_slice(), bytes.as_ptr(), unsafe { bytes.as_ptr().add(bytes.len()) },
            );
            assert_eq!(found_pattern.is_none());
      }
}
