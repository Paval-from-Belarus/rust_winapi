extern crate core;
extern crate text_io;

use std::{mem, ptr};
use std::ffi::{c_int, c_void};
use std::fs::File;
use std::io::Write;
use std::mem::MaybeUninit;
use std::path::Path;
use winapi::shared::minwindef::{DWORD, FALSE, FARPROC, HMODULE, LPVOID, TRUE, WORD};
use winapi::um::handleapi::CloseHandle;
use winapi::um::libloaderapi::{GetModuleHandleA, GetProcAddress, LoadLibraryW};
use winapi::um::memoryapi::{VirtualAllocEx, VirtualFree, WriteProcessMemory};
use winapi::um::processthreadsapi::{CreateRemoteThread, OpenProcess};
use winapi::um::tlhelp32::{CreateToolhelp32Snapshot, Process32First, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS};
use winapi::um::winbase::lstrlenW;
use winapi::um::winnt::{INT, LPCWSTR, MEM_COMMIT, MEM_FREE, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE, PROCESS_ALL_ACCESS};
use winapi::um::winuser::{CharLowerBuffW, MB_ICONEXCLAMATION, MB_OK, MessageBoxW};
use utils::WindowsString;


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

fn main() {
        loop {
                println!("The process name: ");
                let process_name: String = text_io::read!();
                println!("The injectable dll name: ");
                let dll_name: String = text_io::read!();
                let pid_option = find_pid_by_name(process_name.as_str());
                if let Some(pid) = pid_option {
                        let injection_result = inject_dll(pid, dll_name.as_os_str().as_slice());
                        if injection_result.is_ok() {
                                println!("Injected successfully!");
                        } else {
                                println!("Injection failed");
                        }
                } else {
                        println!("Failed to get process pid");
                }
        }
}
