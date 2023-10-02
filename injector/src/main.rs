extern crate core;
extern crate text_io;

use std::{mem, ptr};
use std::ffi::{c_void, CStr, CString};
use std::io::Write;
use std::mem::{align_of, MaybeUninit};

use winapi::shared::minwindef::{DWORD, FALSE, FARPROC, HMODULE, INT, LPVOID, TRUE, WORD};
use winapi::um::handleapi::CloseHandle;
use winapi::um::libloaderapi::{GetModuleHandleA, GetModuleHandleW, GetProcAddress};
use winapi::um::memoryapi::{VirtualAllocEx, WriteProcessMemory};
use winapi::um::processthreadsapi::{CreateRemoteThread, OpenProcess};
use winapi::um::tlhelp32::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS};
use winapi::um::winbase::lstrlenW;
use winapi::um::winnt::{HANDLE, MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READWRITE, PAGE_READWRITE, PROCESS_ALL_ACCESS};
use winapi::um::winuser::CharLowerBuffW;

use utils::{StringSearchParams, WindowsString};

fn equals_string(first: &[u16], second: &[u16]) -> bool {
    let min_length = usize::min(first.len(), second.len());
    for i in 0..min_length {
        if first[i] != second[i] {
            return false;
        }
    }
    return true;
}

fn find_pid_by_name(process_name: &str) -> Option<DWORD> {
    let process_name = process_name.as_os_str();
    let snapshot_handle = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot_handle.is_null() {
        println!("Failed to load snapshot");
        return None;
    }
    let mut process_entry = unsafe {
        MaybeUninit::<PROCESSENTRY32W>::zeroed().assume_init()
    };
    process_entry.dwSize = mem::size_of::<PROCESSENTRY32W>() as DWORD;
    let mut has_next = unsafe { Process32FirstW(snapshot_handle, &mut process_entry) };
    let mut pid: Option<DWORD> = None;
    while pid.is_none() && has_next == TRUE {
        unsafe {
            CharLowerBuffW(
                &mut process_entry.szExeFile as _, lstrlenW(&process_entry.szExeFile as _) as DWORD)
        };
        // let mut vec = process_entry.szExeFile.to_vec();
        // vec.push(0);
        // unsafe {
        //         MessageBoxW(ptr::null_mut(), vec.as_ptr(), "Error".as_os_str().as_ptr(), MB_ICONEXCLAMATION | MB_OK);
        // }
        if equals_string(process_name.as_slice(), process_entry.szExeFile.as_slice()) {
            pid = Some(process_entry.th32ProcessID);
        } else {
            has_next = unsafe { Process32NextW(snapshot_handle, &mut process_entry) };
        }
    }
    return pid;
}

fn align_pointer(pointer: LPVOID) -> LPVOID {
    let offset = pointer.align_offset(mem::align_of::<usize>());
    unsafe { pointer.add(offset) }
}

fn copy_to_process(process_handle: HANDLE, params: &StringSearchParams) -> Result<LPVOID, &str> {
    let align_offset = mem::size_of::<usize>();
    let params_size = mem::size_of::<StringSearchParams>() + params.cbSearchLen + params.cbReplaceLen;
    let offset = unsafe {
        VirtualAllocEx(process_handle, ptr::null_mut(), params_size + 3 * align_offset,
                       MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE)
    };
    if offset.is_null() {
        return Err("Failed to allocate memory in foreign process");
    }
    let other_params_ptr = align_pointer(offset);
    let search_pattern = unsafe {
        align_pointer(other_params_ptr.add(mem::size_of::<StringSearchParams>()))
    };
    let replace_pattern = unsafe {
        align_pointer(search_pattern.add(params.cbSearchLen))
    };
    let other_params_ptr = unsafe {
        align_pointer(search_pattern.add(params.cbReplaceLen))
    };
    let other_params = StringSearchParams {
        szSearch: search_pattern as _,
        cbSearchLen: params.cbSearchLen,
        szReplace: replace_pattern as _,
        cbReplaceLen: params.cbReplaceLen,
    };
    unsafe {
        let mut cb_written = 0;
        let copy_result = WriteProcessMemory(process_handle, search_pattern,
                                             params.szSearch as _, params.cbSearchLen, &mut cb_written);
        debug_assert!(copy_result == TRUE);
        let copy_result = WriteProcessMemory(process_handle, replace_pattern,
                                             params.szReplace as _, params.cbReplaceLen, &mut cb_written);
        debug_assert!(copy_result == TRUE);
        let source_params_ptr = (&other_params as *const StringSearchParams) as LPVOID;
        let copy_result = WriteProcessMemory(process_handle, other_params_ptr,
                                             source_params_ptr, mem::size_of::<StringSearchParams>(), ptr::null_mut());
        debug_assert!(copy_result == TRUE);
    };
    Ok(other_params_ptr)
}

//this function assume that dll is already loaded
fn invoke_function(pid: DWORD, dll_name: &str, function_name: &str, params: &StringSearchParams) {
    let process_handle = unsafe { OpenProcess(PROCESS_ALL_ACCESS, FALSE, pid) };
    if process_handle.is_null() {
        println!("Failed to open process");
        return;
    }
    let function_handler = unsafe {
        let dll_handle = utils::load_library(dll_name);
        // let dll_handle = 0x7fff09be0000 as HMODULE;
        // let dll_handle = GetModuleHandleA(dll_name.as_ptr() as _);
        let offset = GetProcAddress(dll_handle, function_name.as_ptr() as _);
        if offset.is_null() {
            println!("Failed to load dll function");
            return;
        }
        // let offset = 0x7fff00c41c90 as FARPROC;
        mem::transmute::<FARPROC, unsafe extern "system" fn(*mut c_void) -> u32>(offset)
    };
    let copy_params_result = copy_to_process(process_handle, params);
    if copy_params_result.is_err() {
        print!("Failed to copy params to another proc");
        return;
    }
    let params_ptr = copy_params_result.unwrap();
    let thread_handle = unsafe {
        CreateRemoteThread(process_handle, ptr::null_mut(),
                           0, Some(function_handler as _), params_ptr, 0, ptr::null_mut(),
        )
    };
    if thread_handle.is_null() {
        println!("Failed to invoke remote thread function");
        return;
    }
    unsafe { CloseHandle(process_handle); }
}

fn inject_dll(pid: DWORD, dll_name: &[u16]) -> Result<(), ()> {
    let process_handle = unsafe { OpenProcess(PROCESS_ALL_ACCESS, FALSE, pid) };
    if process_handle.is_null() {
        println!("Failed to open process");
        return Err(());
    }
    let dll_name_buffer_size = dll_name.len() * 2;//byte size
    let init_handler = unsafe {
        let offset = GetProcAddress(GetModuleHandleA(b"kernel32.dll\0".as_ptr() as _), "LoadLibraryW\0".as_ptr() as _);
        mem::transmute::<FARPROC, unsafe extern "system" fn(*mut c_void) -> u32>(offset as FARPROC)
    };
    let memory_ptr = unsafe {
        VirtualAllocEx(process_handle, ptr::null_mut(), dll_name_buffer_size,
                       MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE)
    };
    if memory_ptr.is_null() {
        println!("Failed to allocated memory in foreign process");
        return Err(());
    }
    let was_written = unsafe {
        WriteProcessMemory(process_handle, memory_ptr,
                           dll_name.as_ptr() as _, dll_name_buffer_size, ptr::null_mut())
    };
    if was_written == FALSE {
        print!("Failed to write to allocated memory");
        return Err(());
    }
    let thread_handle = unsafe {
        CreateRemoteThread(process_handle, ptr::null_mut(),
                           0, Some(init_handler as _), memory_ptr, 0, ptr::null_mut(),
        )
    };
    if thread_handle.is_null() {
        return Err(());
    }
    unsafe {
        CloseHandle(process_handle);
    }
    return Ok(());
}

const DLL_NAME: &str = "string_replace.dll";
const FUNCTION_NAME: &str = "replace\0";
const SEARCH_PATTERN: &[u8] = b"HEAP_STRING\0";
const REPLACE_PATTERN: &[u8] = b"HACKER\0";

pub type DllFunction = fn(&StringSearchParams) -> INT;

fn main() {
    loop {
        println!("The process name: ");
        let process_name: String = text_io::read!();
        let pid_option = find_pid_by_name(process_name.as_str());
        if let Some(pid) = pid_option {
            let injection_result = inject_dll(pid, DLL_NAME.as_os_str().as_slice());
            if injection_result.is_err() {
                println!("Injection failed");
                continue;
            }
            println!("Injected successfully!");
            let params = StringSearchParams {
                szSearch: SEARCH_PATTERN.as_ptr() as _,
                cbSearchLen: SEARCH_PATTERN.len(),
                szReplace: REPLACE_PATTERN.as_ptr() as _,
                cbReplaceLen: REPLACE_PATTERN.len(),
            };
            invoke_function(pid, DLL_NAME, FUNCTION_NAME, &params);
        } else {
            println!("Failed to get process pid");
        }
    }
}
