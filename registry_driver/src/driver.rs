extern crate alloc;

use wdk_sys::{DEVICE_OBJECT, DRIVER_OBJECT, NTSTATUS, PVOID, STATUS_SUCCESS};
use wdk_sys::ntddk::IoDeleteDevice;

pub struct RegistryLogger {
    device: DEVICE_OBJECT
}

impl RegistryLogger {
    pub fn new(driver: DRIVER_OBJECT) -> Result<Self, NTSTATUS> {
        todo!()
    }
    pub unsafe fn free(&mut self) {
        IoDeleteDevice(self.device);
    }
}

extern "C" fn registry_callback(context: PVOID, first: PVOID, second: PVOID) -> NTSTATUS {
    STATUS_SUCCESS
}

fn init() {}