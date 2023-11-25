#![no_std]
extern crate alloc;
#[cfg(not(test))]
extern crate wdk_panic;

use alloc::ffi::CString;
use alloc::string::String;
use core::mem::MaybeUninit;
use core::{mem, slice};
use static_assertions::const_assert;
use wdk::println;
#[cfg(not(test))]
use wdk_alloc::WDKAllocator;

#[cfg(not(test))]
#[global_allocator]
static GLOBAL_ALLOCATOR: WDKAllocator = WDKAllocator;

use wdk_sys::ntddk::DbgPrint;
use wdk_sys::{
    DPFLTR_INFO_LEVEL, DRIVER_OBJECT, NTSTATUS, PCUNICODE_STRING, STATUS_SUCCESS, ULONG, WDFDEVICE,
    WDFDEVICE_INIT, WDFDRIVER, WDF_DRIVER_CONFIG, WDF_NO_HANDLE, WDF_NO_OBJECT_ATTRIBUTES,
    WDF_RELEASE_HARDWARE_ORDER_ON_FAILURE,
};

fn dbg_print(message: &str) {
    let message = CString::new(message).unwrap();
    unsafe {
        DbgPrint(message.as_ptr());
    }
}

#[export_name = "DriverEntry"] // WDF expects a symbol with the name DriverEntry
pub unsafe extern "system" fn driver_entry(
    driver: &mut DRIVER_OBJECT,
    registry_path: PCUNICODE_STRING,
) -> NTSTATUS {
    let status: NTSTATUS = STATUS_SUCCESS;
    let mut driver_config = unsafe {
        const CONFIG_SIZE: usize = mem::size_of::<WDF_DRIVER_CONFIG>();
        const_assert!(CONFIG_SIZE <= ULONG::MAX as usize);
        WDF_DRIVER_CONFIG {
            Size: CONFIG_SIZE as ULONG,
            EvtDriverDeviceAdd: Some(evt_driver_device_add),
            ..WDF_DRIVER_CONFIG::default()
        }
    };
    let driver_attributes = WDF_NO_OBJECT_ATTRIBUTES;
    let driver_handle_output = WDF_NO_HANDLE.cast::<*mut wdk_sys::WDFDRIVER__>();
    let wdf_driver_create_status = unsafe {
        wdk_macros::call_unsafe_wdf_function_binding!(
            WdfDriverCreate,
            driver as wdk_sys::PDRIVER_OBJECT,
            registry_path,
            driver_attributes,
            &mut driver_config,
            driver_handle_output,
        )
    };
    let registry_path = String::from_utf16_lossy(unsafe {
        slice::from_raw_parts(
            (*registry_path).Buffer,
            (*registry_path).Length as usize / core::mem::size_of_val(&(*(*registry_path).Buffer)),
        )
    });
    println!("KMDF Driver Entry Complete! Driver Registry Parameter Key: {registry_path}");
    status
}

extern "C" fn evt_driver_device_add(
    driver: WDFDRIVER,
    mut device_init: *mut WDFDEVICE_INIT,
) -> NTSTATUS {
    println!("Evt driver Entered!");
    let mut device_handle_output: WDFDEVICE = WDF_NO_HANDLE.cast();
    let status = unsafe {
        wdk_macros::call_unsafe_wdf_function_binding!(
            WdfDeviceCreate,
            &mut device_init,
            WDF_NO_OBJECT_ATTRIBUTES,
            &mut device_handle_output,
        )
    };
    println!("WdfDeviceCreate NTSTATUS: {status:#02x}");
    status
}

extern "C" fn driver_exit(driver: *mut DRIVER_OBJECT) {
    println!("Goodbye World!");
    println!("Driver Exit Complete!");
}
// use wdk_alloc::WDKAllocator;
//
// #[cfg(not(test))]
// #[global_allocator]
// static GLOBAL_ALLOCATOR: WDKAllocator = WDKAllocator;
// use wdk_sys::{
//     DRIVER_OBJECT,
//     NTSTATUS,
//     PCUNICODE_STRING,
// };
//
// // use wdk_sys::{DRIVER_OBJECT, NTSTATUS, PCUNICODE_STRING, WDF_NO_OBJECT_ATTRIBUTES};
// //
// #[export_name = "DriverEntry"] // WDF expects a symbol with the name DriverEntry
// pub unsafe extern "system" fn driver_entry(
//     driver: &mut DRIVER_OBJECT,
//     registry_path: PCUNICODE_STRING,
// ) -> NTSTATUS {
// //     // let status: NTSTATUS;
// //     // let config = MaybeUninit::<WDF_DRIVER_CONFIG >::uninit();
// //     // WDF_DRIVER_CONFIG_INIT(config.as_mut_ptr(), )
// //     // status = WdfDriverCreate(driver,
// //     //                          registry_path,
// //     //                          WDF_NO_OBJECT_ATTRIBUTES,
// //     //
// //     // )
// //
//     0
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {}
}
