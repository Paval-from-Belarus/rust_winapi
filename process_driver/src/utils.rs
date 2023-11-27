use alloc::string::String;
use alloc::vec::Vec;
use core::{mem, ptr, slice};
use wdk_sys::{BOOLEAN, FALSE, HANDLE, IRP, KEVENT, NTSTATUS, PCREATE_PROCESS_NOTIFY_ROUTINE, PIO_STACK_LOCATION, PKEVENT, TRUE, UNICODE_STRING};
use wdk_sys::ntddk::{IoCreateNotificationEvent, KeClearEvent, KeSetEvent, PsSetCreateProcessNotifyRoutine, ZwClose};

pub trait WindowsUnicode {
    fn to_unicode(&self) -> UNICODE_STRING;
    fn from_unicode(value: &UNICODE_STRING) -> Self;
}

pub struct KernelEvent {
    handle: HANDLE,
    event: PKEVENT,
}

impl KernelEvent {
    pub fn new(event_name: &String) -> Self {
        let mut handle: HANDLE = ptr::null_mut();
        let mut unicode_event_name = event_name.to_unicode();
        let event = unsafe {
            IoCreateNotificationEvent(&mut unicode_event_name, &mut handle)
        };
        debug_assert!(!handle.is_null(), "Handle event is still null");
        unsafe { KeClearEvent(event) };
        Self { handle, event }
    }
    pub fn raise(&mut self) {
        unsafe {
            KeSetEvent(self.event, 0, FALSE as BOOLEAN);
            KeClearEvent(self.event);
        }
    }
    pub unsafe fn free(&mut self) {
        ZwClose(self.handle);
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

impl WindowsUnicode for String {
    fn to_unicode(&self) -> UNICODE_STRING {
        let bytes = self.as_bytes();
        let mut buffer = Vec::<u16>::with_capacity(bytes.len() + 1);
        for byte in bytes.iter() {
            buffer.push(*byte as u16);
        }
        buffer.push(0u16);
        UNICODE_STRING {
            Length: (buffer.len() - 1) as u16,
            MaximumLength: buffer.len() as u16,
            Buffer: buffer.leak().as_mut_ptr(),
        }
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
