#![no_std]

use alloc::string::String;
use alloc::vec::Vec;
use core::{mem, ptr, slice};
use wdk::println;
use wdk_sys::ntddk::{IoCreateNotificationEvent, KeClearEvent, KeSetEvent, PsSetCreateProcessNotifyRoutine, RtlInitUnicodeString, ZwClose};
use wdk_sys::{BOOLEAN, FALSE, HANDLE, IRP, NTSTATUS, PCREATE_PROCESS_NOTIFY_ROUTINE, PIO_STACK_LOCATION, PKEVENT, STATUS_UNEXPECTED_IO_ERROR, TRUE, UNICODE_STRING};

extern crate alloc;

pub struct KernelEvent {
    handle: HANDLE,
    event: PKEVENT,
}

impl KernelEvent {
    pub fn new(event_name: &String) -> Result<Self, NTSTATUS> {
        let mut handle: HANDLE = ptr::null_mut();
        let mut unicode_event_name = event_name.to_unicode();
        println!("The unicode len={} and buf_len={}", unicode_event_name.Length, unicode_event_name.MaximumLength);
        let event = unsafe {
            IoCreateNotificationEvent(&mut unicode_event_name, &mut handle)
        };
        if handle.is_null() || event.is_null() {
            println!("Event or handle is null");
            return Err(STATUS_UNEXPECTED_IO_ERROR);
        }
        println!("Event {event_name} is created");
        unsafe { KeClearEvent(event) };
        Ok(Self { handle, event })
    }
    pub fn raise(&mut self) {
        unsafe {
            KeSetEvent(self.event, 0, FALSE as BOOLEAN);
            KeClearEvent(self.event);
        }
    }
    pub unsafe fn free(&mut self) {
        let _ = ZwClose(self.handle);
    }
}

pub fn get_current_io_stack_location(irp: &IRP) -> PIO_STACK_LOCATION {
    unsafe {
        irp.Tail.Overlay.__bindgen_anon_2.__bindgen_anon_1.CurrentStackLocation
    }
}

pub fn add_notify_callback(callback: PCREATE_PROCESS_NOTIFY_ROUTINE) -> NTSTATUS {
    unsafe { PsSetCreateProcessNotifyRoutine(callback, FALSE as BOOLEAN) }
}

pub fn remove_notify_callback(callback: PCREATE_PROCESS_NOTIFY_ROUTINE) -> NTSTATUS {
    unsafe { PsSetCreateProcessNotifyRoutine(callback, TRUE as BOOLEAN) }
}
pub trait WindowsUnicode {
    fn to_unicode(&self) -> UNICODE_STRING;
    fn from_unicode(value: &UNICODE_STRING) -> Self;
}

impl WindowsUnicode for String {
    fn to_unicode(&self) -> UNICODE_STRING {
        let bytes = self.as_bytes();
        let mut buffer = Vec::<u16>::with_capacity(bytes.len() + 1);
        for byte in bytes {
            buffer.push(u16::from(*byte));
        }
        buffer.push(0u16);
        let mut result = UNICODE_STRING::default();
        unsafe {
            RtlInitUnicodeString(&mut result,
                                 Vec::leak(buffer).as_ptr());
        };
        result
    }

    fn from_unicode(unicode: &UNICODE_STRING) -> Self {
        let slice = unsafe {
            slice::from_raw_parts(
                unicode.Buffer,
                unicode.Length as usize / mem::size_of_val(&(*unicode.Buffer)))
        };
        String::from_utf16_lossy(slice)
    }
}
