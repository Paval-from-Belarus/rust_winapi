use std::{mem, ptr};
use std::ffi::CString;
use std::fmt::format;
use winapi::shared::minwindef::HMODULE;
use winapi::um::errhandlingapi::SetLastError;
use winapi::um::libloaderapi::{FreeLibrary, GetProcAddress, LoadLibraryW};
use winapi::um::processthreadsapi::GetCurrentProcess;
use winapi::um::winnt::INT;
use winapi_util::console::Intense::No;
use utils::{StringSearchParams, WindowsString};

extern crate core;
extern crate utils;

const DLL_PATH: &str = "string_replace.dll";
const FUNCTION_NAME: &str = "replace\0";


pub fn find_function(dll_module: HMODULE, function_name: &str) -> Option<SearchReplaceHandler> {
    let handler_ptr = unsafe { GetProcAddress(dll_module, function_name.as_ptr() as _) };
    let result;
    if !handler_ptr.is_null() {
        let handler: SearchReplaceHandler = unsafe { mem::transmute(handler_ptr) };
        result = Some(handler);
    } else {
        result = None;
    }
    return result;
}

pub type SearchReplaceHandler = fn(&StringSearchParams) -> INT;

fn replace_string(params: &StringSearchParams, handler: SearchReplaceHandler) -> Result<(), INT> {
    let result = unsafe {
        handler(params)
    };
    if result == 0 {
        Ok(())
    } else {
        Err(result)
    }
}

pub fn run(handler: SearchReplaceHandler) {
    let hacker_string = b"HACKER\0";
    let tail_string = b"_STRING\0";//common for each
    let mut params = StringSearchParams {
        szSearch: ptr::null_mut(),
        cbSearchLen: 0,
        szReplace: hacker_string.as_ptr() as _,
        cbReplaceLen: hacker_string.len(),
    };
    let static_string = b"STATIC_STRING\0";//pure CString
    utils::show_alert_message("Static replacement. Origin string: ", String::from_utf8(static_string.to_vec()).unwrap().as_str());
    params.szSearch = static_string.as_ptr() as _;
    params.cbSearchLen = static_string.len();
    if let Err(error_code) = replace_string(&params, handler) {
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
    if let Err(error_code) = replace_string(&params, handler) {
        utils::show_error_message_with_error_code("Heap replacement failed with error code ", error_code);
    } else {
        utils::show_alert_message("Finished successfully. Result string: ", String::from_utf8(heap_string.clone()).unwrap().as_str());
    }
    let stack_string = [b'S', b'T', b'A', b'C', b'K', b'_', b'S', b'T', b'R', b'I', b'N', b'G', 0];
    utils::show_alert_message("Stack replacement. Origin string: ", String::from_utf8(stack_string.to_vec()).unwrap().as_str());
    params.szSearch = stack_string.as_ptr() as _;
    params.cbSearchLen = stack_string.len();
    if let Err(error_code) = replace_string(&params, handler) {
        utils::show_error_message_with_error_code("Stack replacement failed with error code ", error_code);
    } else {
        utils::show_alert_message("Finished successfully. Result string: ", String::from_utf8(stack_string.to_vec()).unwrap().as_str());
    }
}

fn main() {
    let dll_module = utils::load_library(DLL_PATH);
    if !dll_module.is_null() {
        let handler = find_function(dll_module, FUNCTION_NAME);
        if handler.is_some() {
            run(handler.unwrap());
        } else {
            utils::show_error_message("function handler is not found");
            utils::free_library(dll_module);
        }
    } else {
        utils::show_error_message("string_replace.dll is not found");
        unsafe { SetLastError(1); }
    }
}
