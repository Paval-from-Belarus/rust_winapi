use std::ffi::CStr;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::{mem, ptr};
use winapi::shared::minwindef::{DWORD, FALSE, LPVOID};
use winapi::um::processthreadsapi::{
    CreateProcessA, CreateThread, TerminateThread, PROCESS_INFORMATION, STARTUPINFOA,
};
use winapi::um::synchapi::{OpenEventA, WaitForSingleObject};
use winapi::um::winbase::{CREATE_UNICODE_ENVIRONMENT, WAIT_OBJECT_0};
use winapi::um::winnt::{HANDLE, SYNCHRONIZE};
use winapi::um::winuser::SENDASYNCPROC;

const CREATE_EVENT_NAME: &'static CStr =
    unsafe { CStr::from_bytes_with_nul_unchecked(b"RustProcessSpyCreateEvent\0") };
const EXIT_EVENT_NAME: &'static CStr =
    unsafe { CStr::from_bytes_with_nul_unchecked(b"RustProcessSpyExitEvent\0") };

static IS_ALIVE: AtomicBool = AtomicBool::new(true);

pub struct Process {
    handle: HANDLE,
}

unsafe impl Send for Process {}

unsafe impl Sync for Process {}

static RUNNING: Mutex<Vec<Process>> = Mutex::new(Vec::new());

pub extern "system" fn create_process_task(_context: LPVOID) -> DWORD {
    let event = unsafe { OpenEventA(SYNCHRONIZE, FALSE, CREATE_EVENT_NAME.as_ptr()) };
    while IS_ALIVE.load(Ordering::SeqCst) {
        let status = unsafe { WaitForSingleObject(event, 1000) };
        if IS_ALIVE.load(Ordering::SeqCst) && status == WAIT_OBJECT_0 {
            let mut startup_info = unsafe { MaybeUninit::<STARTUPINFOA>::zeroed().assume_init() };
            startup_info.cb = mem::size_of::<STARTUPINFOA>() as DWORD;
            let file_name = b"C:\\drivers\\ultimate.exe";
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
            unsafe { process_info.assume_init() };
        }
    }
    0
}

pub extern "system" fn exit_process_task(_context: LPVOID) -> DWORD {
    0
}

fn main() {
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
}
