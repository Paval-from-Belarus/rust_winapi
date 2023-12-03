use std::collections::VecDeque;
use std::ffi::CStr;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::{mem, ptr};
use winapi::shared::minwindef::{DWORD, FALSE, LPVOID};
use winapi::um::errhandlingapi::GetLastError;
use winapi::um::handleapi::CloseHandle;
use winapi::um::processthreadsapi::{
    CreateProcessA, CreateThread, TerminateProcess, TerminateThread, PROCESS_INFORMATION,
    STARTUPINFOA,
};
use winapi::um::synchapi::{OpenEventA, WaitForSingleObject};
use winapi::um::winbase::{CREATE_UNICODE_ENVIRONMENT, WAIT_OBJECT_0};
use winapi::um::winnt::{HANDLE, SYNCHRONIZE};

const CREATE_EVENT_NAME: &'static [u8] = b"Global\\RustProcessSpyCreateEvent\0";
const EXIT_EVENT_NAME: &'static [u8] = b"Global\\RustProcessSpyExitEvent\0";

static IS_ALIVE: AtomicBool = AtomicBool::new(true);

pub struct Process {
    handle: HANDLE,
}

unsafe impl Send for Process {}

unsafe impl Sync for Process {}

static RUNNING: Mutex<VecDeque<Process>> = Mutex::new(VecDeque::new());

pub extern "system" fn create_process_task(_context: LPVOID) -> DWORD {
    let event = unsafe { OpenEventA(SYNCHRONIZE, FALSE, CREATE_EVENT_NAME.as_ptr() as _) };
    if event.is_null() {
        let code = unsafe { GetLastError() };
        println!("The create event is null. error_code={code}");
        IS_ALIVE.store(false, Ordering::SeqCst);
        return 1;
    }
    while IS_ALIVE.load(Ordering::SeqCst) {
        let status = unsafe { WaitForSingleObject(event, 1000) };
        if IS_ALIVE.load(Ordering::SeqCst) && status == WAIT_OBJECT_0 {
            let mut startup_info = unsafe { MaybeUninit::<STARTUPINFOA>::zeroed().assume_init() };
            startup_info.cb = mem::size_of::<STARTUPINFOA>() as DWORD;
            let file_name = b"c:\\drivers\\ultimate.exe\0";
            let mut process_info = MaybeUninit::<PROCESS_INFORMATION>::uninit();
            let result = unsafe {
                CreateProcessA(
                    file_name.as_ptr() as _,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    FALSE,
                    CREATE_UNICODE_ENVIRONMENT,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    &mut startup_info,
                    process_info.as_mut_ptr(),
                )
            };
            let process_info = unsafe { process_info.assume_init() };
            if result == FALSE {
                println!("Failed to start parallel process...(")
            } else {
                println!("Ultimate started!");
                let mut handles = RUNNING.lock().expect("No one can panic here!");
                handles.push_back(Process {
                    handle: process_info.hProcess,
                });
            }
        }
    }
    unsafe { CloseHandle(event) };
    0
}

pub extern "system" fn exit_process_task(_context: LPVOID) -> DWORD {
    let event = unsafe { OpenEventA(SYNCHRONIZE, FALSE, EXIT_EVENT_NAME.as_ptr() as _) };
    if event.is_null() {
        let code = unsafe { GetLastError() };
        println!("exit event is null. error_code={code}");
        IS_ALIVE.store(false, Ordering::SeqCst);
        return 1;
    }
    while IS_ALIVE.load(Ordering::SeqCst) {
        let status = unsafe { WaitForSingleObject(event, 1000) };
        if IS_ALIVE.load(Ordering::SeqCst) && status == WAIT_OBJECT_0 {
            let mut handles = RUNNING.lock().expect("We no panic too!");
            let removable_option = handles.pop_front();
            if let Some(removed) = removable_option {
                let _ = unsafe { TerminateProcess(removed.handle, 42) };
                unsafe { CloseHandle(removed.handle) };
            } else {
                println!("We too late too remove anything from deque...(")
            }
        }
    }
    unsafe { CloseHandle(event) };
    0
}

fn main() {
    println!("Not New");
    let create_task_thread = unsafe {
        CreateThread(
            ptr::null_mut(),
            0,
            Some(create_process_task),
            ptr::null_mut(),
            0,
            ptr::null_mut(),
        )
    };
    let exit_task_thread = unsafe {
        CreateThread(
            ptr::null_mut(),
            0,
            Some(exit_process_task),
            ptr::null_mut(),
            0,
            ptr::null_mut(),
        )
    };
    let _: String = text_io::read!();
    IS_ALIVE.store(false, Ordering::SeqCst);
    if unsafe { WaitForSingleObject(create_task_thread, 2000) } != WAIT_OBJECT_0 {
        println!("The create thread will be suppressed");
        unsafe {
            TerminateThread(create_task_thread, 1);
        }
    }
    if unsafe { WaitForSingleObject(exit_task_thread, 2000) } != WAIT_OBJECT_0 {
        println!("The exit thread will be suppressed");
        unsafe { TerminateThread(exit_task_thread, 1) };
    }
    println!("Thanks for working with us!)");
}
