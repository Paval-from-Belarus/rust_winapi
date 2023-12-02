extern crate alloc;
extern crate spin;

use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use core::{mem, ptr};
use core::mem::MaybeUninit;
use wdk::{nt_success, println};
use wdk_sys::{DEVICE_OBJECT, DRIVER_OBJECT, IO_NO_INCREMENT, LARGE_INTEGER, macros, NTSTATUS, PCUNICODE_STRING, PVOID, STATUS_NOT_SUPPORTED, STATUS_SUCCESS, ULONG, WDF_DRIVER_CONFIG, WDF_NO_HANDLE, WDFDRIVER};
use wdk_sys::ntddk::{CmRegisterCallback, CmUnRegisterCallback, IoCreateFile, IofCompleteRequest, ZwClose, ZwWriteFile};
use wdk_sys::{*};
use wdk_sys::_CREATE_FILE_TYPE::CreateFileTypeNone;
use wdk_sys::_REG_NOTIFY_CLASS::RegNtSetValueKey;
use utils::WindowsUnicode;

pub struct RegisterLogger {
    cookie: LARGE_INTEGER,
    log_file: HANDLE,
}

unsafe impl Sync for RegisterLogger {}

unsafe impl Send for RegisterLogger {}

impl RegisterLogger {
    pub fn new() -> Result<&'static mut Self, NTSTATUS> {
        let mut log_file: HANDLE = ptr::null_mut();
        let mut io_status_block: IO_STATUS_BLOCK = IO_STATUS_BLOCK::default();
        let mut file_name = "??\\c:\\log.dat".to_string().to_unicode();
        let mut attributes = OBJECT_ATTRIBUTES {
            Length: mem::size_of::<OBJECT_ATTRIBUTES>() as _,
            RootDirectory: ptr::null_mut(),
            ObjectName: &mut file_name,
            Attributes: 0,
            SecurityDescriptor: ptr::null_mut(),
            SecurityQualityOfService: ptr::null_mut(),
        };
        let status = unsafe {
            IoCreateFile(
                &mut log_file,
                FILE_APPEND_DATA,
                &mut attributes,
                &mut io_status_block,
                ptr::null_mut(),
                FILE_ATTRIBUTE_NORMAL,
                FILE_SHARE_READ,
                FILE_OPEN_IF,
                FILE_SEQUENTIAL_ONLY,
                ptr::null_mut(),
                0,
                CreateFileTypeNone,
                ptr::null_mut(),
                0,
            )
        };
        if !nt_success(status) {
            println!("Failed to create file for logging");
            return Err(status);
        }
        let mut logger = Box::leak(Box::<RegisterLogger>::new_uninit());
        let mut cookie: LARGE_INTEGER = LARGE_INTEGER::default();
        let status = unsafe { CmRegisterCallback(Some(Self::callback), logger.as_mut_ptr() as _, &mut cookie as _) };
        if !nt_success(status) {
            println!("Failed to registry register callback");
            let _ = unsafe { ZwClose(log_file) };
            let _ = unsafe { Box::from_raw(logger) };
            return Err(status);
        }
        Ok(logger.write(Self { cookie, log_file }))
    }
    fn write(&self, bytes: &[u8]) {
        let mut io_status_block = MaybeUninit::<IO_STATUS_BLOCK>::uninit();
        let status = unsafe {
            ZwWriteFile(self.log_file,
                        ptr::null_mut(),
                        None,
                        ptr::null_mut(),
                        io_status_block.as_mut_ptr(),
                        bytes.as_ptr() as _,
                        bytes.len() as _,
                        ptr::null_mut(),//ignored because appending to the end
                        ptr::null_mut(),
            )
        };
        if !nt_success(status) {
            println!("Failed to append log file with status={status}");
        }
    }
    fn dispatch(&self, notify_class: REG_NOTIFY_CLASS, generic_info: PVOID) -> NTSTATUS {
        if notify_class == RegNtSetValueKey {
            let info = unsafe {
                &*(generic_info as *const REG_SET_VALUE_KEY_INFORMATION)
            };
            let entry_key = unsafe {
                String::from_unicode(&*info.ValueName)
            };
            let message = format!("The entry {entry_key} will be changed\n");
            self.write(message.as_bytes());
        }
        STATUS_SUCCESS
    }
    unsafe extern "C" fn callback(context: PVOID, first: PVOID, second: PVOID) -> NTSTATUS {
        let logger = context as *const RegisterLogger;
        (*logger).dispatch(first as _, second)
    }
    pub unsafe fn free(&mut self) {
        unsafe {
            let _ = CmUnRegisterCallback(self.cookie);
            let _ = ZwClose(self.log_file);
        }
    }
}

static LOGGER: spin::Mutex<Option<&'static mut RegisterLogger>> = spin::Mutex::new(None);

#[link_section = "INIT"]
#[export_name = "DriverEntry"] // WDF expects a symbol with the name DriverEntry
extern "system" fn driver_entry(
    driver: &mut DRIVER_OBJECT,
    registry_path: PCUNICODE_STRING,
) -> NTSTATUS {
    let mut driver_config = WDF_DRIVER_CONFIG {
        Size: mem::size_of::<WDF_DRIVER_CONFIG>() as ULONG,
        ..WDF_DRIVER_CONFIG::default()
    };
    let driver_handle_output = WDF_NO_HANDLE as *mut WDFDRIVER;
    let nt_status = unsafe {
        macros::call_unsafe_wdf_function_binding!(
            WdfDriverCreate,
            driver as PDRIVER_OBJECT,
            registry_path,
            WDF_NO_OBJECT_ATTRIBUTES,
            &mut driver_config,
            driver_handle_output,
        )
    };
    if !nt_success(nt_status) {
        println!("Error: WdfDriverCreate failed {nt_status:#010X}");
        return nt_status;
    }
    let logger_result = RegisterLogger::new();
    match logger_result {
        Ok(logger) => {
            let _ = LOGGER.lock().replace(logger);
        }
        Err(code) => {
            return code;
        }
    }
    init_driver_functions(driver);
    nt_status
}


#[link_section = "INIT"]
fn init_driver_functions(driver: &mut DRIVER_OBJECT) {
    for function in driver.MajorFunction.iter_mut() {
        *function = Some(unsupported_function);
    }
    driver.DriverUnload = Some(unload_driver);
    println!("Driver functions are initialized");
}

extern "C" fn unsupported_function(_device: *mut DEVICE_OBJECT, irp: *mut IRP) -> NTSTATUS {
    println!("Unsupported handler was invoked");
    let request = unsafe { &mut *irp };
    let status_block = &mut request.IoStatus;
    status_block.__bindgen_anon_1.Status = STATUS_NOT_SUPPORTED;
    status_block.Information = 0;
    unsafe { IofCompleteRequest(irp, IO_NO_INCREMENT as _) };
    STATUS_SUCCESS
}

extern "C" fn unload_driver(_driver: *mut DRIVER_OBJECT) {
    println!("Driver unloading is started");
    unsafe {
        let mut guard = LOGGER.lock();
        let logger_option = guard.as_deref_mut();
        if let Some(logger) = logger_option {
            logger.free();
        }
    }
    println!("Driver is unloaded");
}
