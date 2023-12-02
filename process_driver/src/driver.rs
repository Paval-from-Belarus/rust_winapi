use wdk::{nt_success, paged_code, println};
use wdk_sys::{DRIVER_OBJECT, HANDLE, macros, ntddk::KeGetCurrentIrql, NTSTATUS, *};

extern crate alloc;

use alloc::{slice, string::String};
use alloc::boxed::Box;

use alloc::string::ToString;

use core::{mem, ptr};
use core::ffi::CStr;

use core::ptr::NonNull;
use no_panic::no_panic;

use wdk_sys::_WORK_QUEUE_TYPE::DelayedWorkQueue;
use wdk_sys::ntddk::{IoAllocateWorkItem, IoCreateDevice, IoDeleteDevice, IofCompleteRequest, IoFreeWorkItem, IoQueueWorkItem, MmGetSystemRoutineAddress, PsLookupProcessByProcessId};
use crate::utils::{add_notify_callback, KernelEvent, remove_notify_callback, WindowsUnicode};

pub struct StringObject {
    handle: WDFSTRING,
}

impl StringObject {
    pub fn new() -> Self {
        let mut handle: WDFSTRING = ptr::null_mut();
        let nt_status = unsafe {
            macros::call_unsafe_wdf_function_binding!(
                WdfStringCreate,
                ptr::null_mut(),
                WDF_NO_OBJECT_ATTRIBUTES,
                &mut handle
            )
        };
        if !nt_success(nt_status) {
            panic!("WdfStringCreate failed {nt_status:#010X}");
        }
        Self { handle }
    }
    pub fn wrap(string: &UNICODE_STRING) -> Self {
        let mut handle: WDFSTRING = ptr::null_mut();
        let nt_status = unsafe {
            macros::call_unsafe_wdf_function_binding!(
                WdfStringCreate,
                string,
                WDF_NO_OBJECT_ATTRIBUTES,
                &mut handle
            )
        };
        assert!(nt_success(nt_status), "WdfStringCreate with wrapper failed {nt_status:#010X}");
        Self { handle }
    }
    pub fn as_unicode(&self) -> UNICODE_STRING {
        let mut string = UNICODE_STRING::default();
        let [_] = [unsafe {
            macros::call_unsafe_wdf_function_binding!(
                WdfStringGetUnicodeString,
                self.handle,
                &mut string
            );
        }];
        string
    }
    pub fn as_kernel_handle(&self) -> WDFSTRING {
        self.handle
    }
}

impl Drop for StringObject {
    fn drop(&mut self) {
        unsafe {
            let [_] = [macros::call_unsafe_wdf_function_binding!(
                WdfObjectDelete,
                self.handle as WDFOBJECT
            )];
        }
    }
}

fn find_pid_resolver() -> Result<ProcessNameResolver, NTSTATUS> {
    let mut resolver_name = "PsGetProcessImageFileName".to_string().to_unicode();
    let resolver = unsafe {
        let address: PVOID = MmGetSystemRoutineAddress(&mut resolver_name);
        if address.is_null() {
            println!("Failed to find GetProcessImageFileName");
            Err(STATUS_NO_SUCH_MEMBER)
        } else {
            Ok(mem::transmute::<PVOID, ProcessNameResolver>(address))
        }
    };
    resolver
}

pub struct SpyWorker {
    handle: PIO_WORKITEM,
    proc: HANDLE,
    //the process with which interact
    is_created: BOOLEAN,
    //the process state
    spy: NonNull<ProcessSpy>,
}

impl SpyWorker {
    pub fn new(mut spy: NonNull<ProcessSpy>, proc: HANDLE, is_created: BOOLEAN) -> Box<SpyWorker> {
        let device = unsafe { spy.as_mut().device() };
        let handle = unsafe { IoAllocateWorkItem(device) };
        Box::new(Self { handle, proc, is_created, spy })
    }
    pub const fn handle(&self) -> PIO_WORKITEM {
        self.handle
    }
    pub unsafe extern "C" fn dispatch_wrapper(_device: *mut DEVICE_OBJECT, context: PVOID) {
        println!("Starting worker dispatching");
        let mut worker = Box::from_raw(context.cast::<Self>());
        let spy = worker.spy.as_mut();
        spy.dispatch(worker.proc, worker.is_created);
    }
}

impl Drop for SpyWorker {
    fn drop(&mut self) {
        unsafe { IoFreeWorkItem(self.handle) }
    }
}

///the main struct that control situation
pub struct ProcessSpy {
    device: NonNull<DEVICE_OBJECT>,
    //events to communicate with user-mode manager that should start/close corresponding process
    create_event: KernelEvent,
    exit_event: KernelEvent,
    //the function pointer
    pid_resolver: ProcessNameResolver,
}

unsafe impl Send for ProcessSpy {}

unsafe impl Sync for ProcessSpy {}

type ProcessNameResolver = fn(PEPROCESS) -> PCHAR;


impl ProcessSpy {
    const TRACKABLE_PROCESS_NAME: &'static CStr = unsafe {
        CStr::from_bytes_with_nul_unchecked(
            b"firefox.exe\0"
        )
    };
    pub fn new(driver: &mut DRIVER_OBJECT) -> Result<&'static mut Self, NTSTATUS> {
        let mut device: *mut DEVICE_OBJECT = ptr::null_mut();
        let nt_status = unsafe {
            IoCreateDevice(
                driver,
                mem::size_of::<ProcessSpy>() as ULONG,
                ptr::null_mut(),
                FILE_DEVICE_UNKNOWN,
                0,
                FALSE as BOOLEAN,
                &mut device,
            )
        };
        if !nt_success(nt_status) {
            return Err(nt_status);
        }
        if device.is_null() {
            println!("Device object still null");
            return Err(STATUS_UNEXPECTED_IO_ERROR);
        }
        let create_event_name = concat!("\\BaseNamedObjects\\", "RustProcessSpyCreate")
            .to_string();
        let exit_event_name = concat!("\\BaseNamedObjects\\", "RustProcessSpyExit")
            .to_string();
        let create_event = KernelEvent::new(&create_event_name)?;
        let exit_event = KernelEvent::new(&exit_event_name)?;
        let pid_resolver = find_pid_resolver()?;
        let spy = unsafe {
            let spy_layout = (*device).DeviceExtension.cast::<Self>();
            spy_layout.write(Self {
                device: NonNull::new_unchecked(device),
                create_event,
                exit_event,
                pid_resolver,
            });
            &mut *spy_layout
        };
        println!("New spy is created");
        Ok(spy)
    }

    pub fn dispatch(&mut self, pid: HANDLE, is_created: BOOLEAN) {
        let mut process_info: PEPROCESS = ptr::null_mut();
        let nt_status = unsafe { PsLookupProcessByProcessId(pid, &mut process_info) };
        if !nt_success(nt_status) {
            println!("Failed to lookup process by id {nt_status:#010X}");
            return;
        }
        let pid_resolver = self.pid_resolver;
        let process_name = unsafe {
            let pid_name = pid_resolver(process_info);
            CStr::from_ptr(pid_name)
        };

        match process_name.to_str() {
            Ok(rust_string) => {
                println!("Process {rust_string} is catched");
            }
            Err(_) => {
                println!("Failed to parse UTF-8 from process name");
            }
        }

        if !Self::same_with_trackable(process_name) {
            return;
        }
        if is_created == TRUE as BOOLEAN {
            self.create_event.raise();
        } else {
            self.exit_event.raise();
        }
    }
    fn same_with_trackable(process_name: &CStr) -> bool {
        process_name.eq(Self::TRACKABLE_PROCESS_NAME)
    }
    pub fn device(&mut self) -> &mut DEVICE_OBJECT {
        unsafe { self.device.as_mut() }
    }
    pub unsafe fn free(&mut self) {
        IoDeleteDevice(self.device.as_mut());
        self.create_event.free();
        self.exit_event.free();
    }
}


static CURRENT_SPY: spin::Mutex<Option<&'static mut ProcessSpy>> = spin::Mutex::new(None);

fn current_spy() -> NonNull<ProcessSpy> {
    let mut guard = CURRENT_SPY.lock();
    let spy = guard.as_deref_mut().expect("The Process Spy should be initialized");
    NonNull::from(spy)
}

fn replace_current_spy(spy: &'static mut ProcessSpy) -> Option<&'static mut ProcessSpy> {
    CURRENT_SPY.lock().replace(spy)
}

///the process callback that will be invoked each time when new process is created
pub unsafe extern "C" fn notify_callback(_parent: HANDLE, child: HANDLE, is_created: BOOLEAN) {
    println!("Notify callback is started");
    let spy = current_spy();
    let worker = SpyWorker::new(spy, child, is_created);
    IoQueueWorkItem(
        worker.handle(),
        Some(SpyWorker::dispatch_wrapper),
        DelayedWorkQueue,
        (Box::leak(worker) as *mut SpyWorker).cast(),
    );
}

/// DriverEntry initializes the driver and is the first routine called by the
/// system after the driver is loaded. DriverEntry specifies the other entry
/// points in the function driver, such as EvtDevice and DriverUnload.
///
/// # Arguments
///
/// * `driver` - represents the instance of the function driver that is loaded
///   into memory. DriverEntry must initialize members of DriverObject before it
///   returns to the caller. DriverObject is allocated by the system before the
///   driver is loaded, and it is released by the system after the system
///   unloads the function driver from memory.
/// * `registry_path` - represents the driver specific path in the Registry. The
///   function driver can use the path to store driver related data between
///   reboots. The path does not store hardware instance specific data.
///
/// # Return value:
///
/// * `STATUS_SUCCESS` - if successful,
/// * `STATUS_UNSUCCESSFUL` - otherwise.
#[link_section = "INIT"]
#[export_name = "DriverEntry"] // WDF expects a symbol with the name DriverEntry
#[no_panic]
extern "system" fn driver_entry(
    driver: &mut DRIVER_OBJECT,
    registry_path: PCUNICODE_STRING,
) -> NTSTATUS {
    let mut driver_config = WDF_DRIVER_CONFIG {
        Size: mem::size_of::<WDF_DRIVER_CONFIG>() as ULONG,
        EvtDriverDeviceAdd: Some(echo_evt_device_add),
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
    init_driver_functions(driver);
    let spy_result = ProcessSpy::new(driver);
    match spy_result {
        Ok(spy) => {
            let old = replace_current_spy(spy);
            debug_assert!(old.is_none());
        }
        Err(code) => {
            return code;
        }
    }
    echo_print_driver_version();
    add_notify_callback(Some(notify_callback));
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

#[link_section = "PAGE"]
extern "C" fn unload_driver(_driver: *mut DRIVER_OBJECT) {
    println!("Driver unloading is started");
    unsafe {
        let spy = current_spy().as_mut();
        spy.free();
        remove_notify_callback(Some(notify_callback));
    }
    println!("Driver is unloaded");
}

/// EvtDeviceAdd is called by the framework in response to AddDevice
/// call from the PnP manager. We create and initialize a device object to
/// represent a new instance of the device.
///
/// # Arguments:
///
/// * `_driver` - Handle to a framework driver object created in DriverEntry
/// * `device_init` - Pointer to a framework-allocated WDFDEVICE_INIT structure.
///
/// # Return value:
///
///   * `NTSTATUS`
#[link_section = "PAGE"]
extern "C" fn echo_evt_device_add(_driver: WDFDRIVER, _device_init: PWDFDEVICE_INIT) -> NTSTATUS {
    paged_code!();

    println!("Enter  EchoEvtDeviceAdd");
    STATUS_SUCCESS
}


/// This routine shows how to retrieve framework version string and
/// also how to find out to which version of framework library the
/// client driver is bound to.
///
/// # Arguments:
///
/// # Return value:
///
///   * `NTSTATUS`
#[link_section = "INIT"]
fn echo_print_driver_version() -> NTSTATUS {
    // 1) Retreive version string and print that in the debugger.
    //
    let mut string: WDFSTRING = core::ptr::null_mut();
    let mut us: UNICODE_STRING = UNICODE_STRING::default();
    let mut nt_status = unsafe {
        macros::call_unsafe_wdf_function_binding!(
            WdfStringCreate,
            core::ptr::null_mut(),
            WDF_NO_OBJECT_ATTRIBUTES,
            &mut string
        )
    };
    if !nt_success(nt_status) {
        println!("Error: WdfStringCreate failed {nt_status:#010X}");
        return nt_status;
    }

    // driver = unsafe{macros::call_unsafe_wdf_function_binding!(WdfGetDriver)};
    let driver = unsafe { (*wdk_sys::WdfDriverGlobals).Driver };
    nt_status = unsafe {
        macros::call_unsafe_wdf_function_binding!(WdfDriverRetrieveVersionString, driver, string)
    };
    if !nt_success(nt_status) {
        // No need to worry about delete the string object because
        // by default it's parented to the driver and it will be
        // deleted when the driverobject is deleted when the DriverEntry
        // returns a failure status.
        //
        println!("Error: WdfDriverRetrieveVersionString failed {nt_status:#010X}");
        return nt_status;
    }

    let [_] = [unsafe {
        macros::call_unsafe_wdf_function_binding!(WdfStringGetUnicodeString, string, &mut us)
    }];
    let driver_version = String::from_utf16_lossy(unsafe {
        slice::from_raw_parts(
            us.Buffer,
            us.Length as usize / core::mem::size_of_val(&(*us.Buffer)),
        )
    });
    println!("Echo Sample {driver_version}");

    let [_] = [unsafe {
        macros::call_unsafe_wdf_function_binding!(WdfObjectDelete, string as WDFOBJECT)
    }];
    // string = core::ptr::null_mut();

    // 2) Find out to which version of framework this driver is bound to.
    //
    let mut ver = WDF_DRIVER_VERSION_AVAILABLE_PARAMS {
        Size: mem::size_of::<WDF_DRIVER_VERSION_AVAILABLE_PARAMS>() as ULONG,
        MajorVersion: 1,
        MinorVersion: 0,
    };

    if unsafe {
        macros::call_unsafe_wdf_function_binding!(WdfDriverIsVersionAvailable, driver, &mut ver)
    } > 0
    {
        println!("Yes, framework version is 1.0");
    } else {
        println!("No, framework version is not 1.0");
    }

    STATUS_SUCCESS
}