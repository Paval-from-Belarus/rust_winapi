extern crate alloc;
extern crate spin;

use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use core::{mem, ptr};
use core::mem::MaybeUninit;
use core::ptr::NonNull;
use wdk::{nt_success, paged_code, println};
use wdk_sys::{DEVICE_OBJECT, DRIVER_OBJECT, IO_NO_INCREMENT, LARGE_INTEGER, macros, NTSTATUS, PCUNICODE_STRING, PVOID, STATUS_NOT_SUPPORTED, STATUS_SUCCESS, ULONG, WDF_DRIVER_CONFIG, WDF_NO_HANDLE, WDFDRIVER};
use wdk_sys::ntddk::{KeGetCurrentIrql, CmRegisterCallback, CmUnRegisterCallback, IoCreateFile, IofCompleteRequest, ZwClose, ZwCreateFile, ZwWriteFile, IoCreateDevice, IoDeleteDevice, IoAllocateWorkItem, IoFreeWorkItem, IoQueueWorkItem};
use wdk_sys::{*};
use wdk_sys::_CREATE_FILE_TYPE::CreateFileTypeNone;
use wdk_sys::_REG_NOTIFY_CLASS::RegNtSetValueKey;
use wdk_sys::_WORK_QUEUE_TYPE::DelayedWorkQueue;
use utils::WindowsUnicode;

pub struct IoWorker {
    handle: PIO_WORKITEM,
    message: String,
    file: HANDLE,
}

impl IoWorker {
    pub fn new(logger: &mut RegisterLogger, message: String) -> Box<Self> {
        let handle = unsafe {
            IoAllocateWorkItem(logger.device.as_mut())
        };
        Box::new(Self { handle, message, file: logger.log_file })
    }
    fn write(&self) {
        let bytes = self.message.as_bytes();
        let mut io_status_block = MaybeUninit::<IO_STATUS_BLOCK>::uninit();
        let mut offset = _LARGE_INTEGER::default();
        unsafe {
            offset.u.HighPart = -1;
            offset.u.LowPart = FILE_WRITE_TO_END_OF_FILE;
        }
        let status = unsafe {
            ZwWriteFile(self.file,
                        ptr::null_mut(),
                        None,
                        ptr::null_mut(),
                        io_status_block.as_mut_ptr(),
                        bytes.as_ptr() as _,
                        bytes.len() as _,
                        &mut offset,
                        ptr::null_mut(),
            )
        };
        if !nt_success(status) {
            println!("Failed to append log file with status={status}");
        }
    }
    pub unsafe extern "C" fn dispatch(_device: *mut DEVICE_OBJECT, context: PVOID) {
        let worker = Box::from_raw(context.cast::<Self>());
        worker.write();
    }
}

impl Drop for IoWorker {
    fn drop(&mut self) {
        unsafe {
            IoFreeWorkItem(self.handle);
        }
    }
}

pub struct RegisterLogger {
    cookie: LARGE_INTEGER,
    log_file: HANDLE,
    device: NonNull<DEVICE_OBJECT>,
    dispatched_count: usize,
}

unsafe impl Sync for RegisterLogger {}

unsafe impl Send for RegisterLogger {}


impl RegisterLogger {
    pub fn new(driver: &mut DRIVER_OBJECT) -> Result<&'static mut Self, NTSTATUS> {
        let mut device: *mut DEVICE_OBJECT = ptr::null_mut();
        let nt_status = unsafe {
            IoCreateDevice(
                driver,
                0,
                ptr::null_mut(),
                FILE_DEVICE_UNKNOWN,
                0,
                FALSE as BOOLEAN,
                &mut device,
            )
        };
        if !nt_success(nt_status) {
            println!("Failed to create IoCreateDevice with code={nt_status}");
            return Err(nt_status);
        }
        println!("Device is created");
        let mut log_file: HANDLE = ptr::null_mut();
        let mut io_status_block: IO_STATUS_BLOCK = IO_STATUS_BLOCK::default();
        let mut file_name = "\\DosDevices\\C:\\register-log.dat".to_string().to_unicode();
        let mut attributes = OBJECT_ATTRIBUTES {
            Length: mem::size_of::<OBJECT_ATTRIBUTES>() as _,
            RootDirectory: ptr::null_mut(),
            ObjectName: &mut file_name,
            Attributes: OBJ_CASE_INSENSITIVE | OBJ_KERNEL_HANDLE,
            SecurityDescriptor: ptr::null_mut(),
            SecurityQualityOfService: ptr::null_mut(),
        };
        let status = unsafe {
            IoCreateFile(
                &mut log_file,
                GENERIC_WRITE,
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
            println!("Failed to create file for logger with status={status}");
            return Err(status);
        }
        let logger = Box::leak(Box::<Self>::new_uninit());
        let mut cookie: LARGE_INTEGER = LARGE_INTEGER::default();
        let status = unsafe { CmRegisterCallback(Some(Self::callback), logger.as_mut_ptr().cast(), &mut cookie as _) };
        if !nt_success(status) {
            println!("Failed to registry register callback");
            let _ = unsafe { ZwClose(log_file) };
            let _ = unsafe { Box::from_raw(logger) };
            return Err(status);
        }
        println!("Logger is contructed");
        unsafe {
            Ok(logger.write(Self {
                cookie,
                log_file,
                device: NonNull::new_unchecked(device),
                dispatched_count: 0,
            }))
        }
    }
    fn dispatch(&mut self, notify_class: REG_NOTIFY_CLASS, generic_info: PVOID) -> NTSTATUS {
        //we dispatches only first 1000 entries
        //to prevent bugs)
        if self.dispatched_count >= 1000 {
            return STATUS_SUCCESS;
        }
        let message: String;
        println!("We will log!");
        self.dispatched_count += 1;
        if notify_class == RegNtSetValueKey {
            let info = unsafe {
                &*(generic_info as *const REG_SET_VALUE_KEY_INFORMATION)
            };
            let entry_key = unsafe {
                String::from_unicode(&*info.ValueName)
            };
            message = format!("The entry {entry_key} will be changed\n");
        } else {
            message = "Let's log unknown info\n".to_string();
        }
        let worker = IoWorker::new(self, message);
        unsafe {
            IoQueueWorkItem(worker.handle,
                            Some(IoWorker::dispatch),
                            DelayedWorkQueue,
                            (Box::leak(worker) as *mut IoWorker).cast(),
            );
        }
        STATUS_SUCCESS
    }
    unsafe extern "C" fn callback(context: PVOID, first: PVOID, second: PVOID) -> NTSTATUS {
        let logger = context.cast::<Self>();
        (*logger).dispatch(first as _, second)
    }
    pub unsafe fn free(&mut self) {
        unsafe {
            let _ = CmUnRegisterCallback(self.cookie);
            let _ = ZwClose(self.log_file);
            IoDeleteDevice(self.device.as_mut());
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
        EvtDriverDeviceAdd: Some(echo_evt_device_add),
        ..WDF_DRIVER_CONFIG::default()
    };
    let driver_handle_output = WDF_NO_HANDLE.cast::<WDFDRIVER>();
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
    let logger_result = RegisterLogger::new(driver);
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
    for (index, function) in driver.MajorFunction.iter_mut().enumerate() {
        let proc_index = index as u32;
        if proc_index == IRP_MJ_CREATE || proc_index == IRP_MJ_CLOSE {
            *function = Some(create_close_function);
        } else {
            *function = Some(unsupported_function);
        }
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

extern "C" fn create_close_function(_device: *mut DEVICE_OBJECT, irp: *mut IRP) -> NTSTATUS {
    println!("Create-Close handler was invoked");
    let request = unsafe { &mut *irp };
    let status_block = &mut request.IoStatus;
    status_block.__bindgen_anon_1.Status = STATUS_SUCCESS;
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

#[link_section = "PAGE"]
extern "C" fn echo_evt_device_add(_driver: WDFDRIVER, _device_init: PWDFDEVICE_INIT) -> NTSTATUS {
    paged_code!();

    println!("Enter  EchoEvtDeviceAdd");
    STATUS_SUCCESS
}

