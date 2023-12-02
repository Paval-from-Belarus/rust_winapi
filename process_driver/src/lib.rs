#![feature(ascii_char)]
// Copyright (c) Microsoft Corporation.
// License: MIT OR Apache-2.0

//! # Abstract
//!
//!    This driver demonstrates use of a default I/O Queue, its
//!    request start events, cancellation event, and a synchronized DPC.
//!
//!    To demonstrate asynchronous operation, the I/O requests are not completed
//!    immediately, but stored in the drivers private data structure, and a
//!    timer DPC will complete it next time the DPC runs.
//!
//!    During the time the request is waiting for the DPC to run, it is
//!    made cancellable by the call WdfRequestMarkCancelable. This
//!    allows the test program to cancel the request and exit instantly.
//!
//!    This rather complicated set of events is designed to demonstrate
//!    the driver frameworks synchronization of access to a device driver
//!    data structure, and a pointer which can be a proxy for device hardware
//!    registers or resources.
//!
//!    This common data structure, or resource is accessed by new request
//!    events arriving, the DPC that completes it, and cancel processing.
//!
//!    Notice the lack of specific lock/unlock operations.
//!
//!    Even though this example utilizes a serial queue, a parallel queue
//!    would not need any additional explicit synchronization, just a
//!    strategy for managing multiple requests outstanding.

#![no_std]
#![cfg_attr(feature = "nightly", feature(hint_must_use))]
// #![deny(warnings)]
// #![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]
#![allow(clippy::missing_safety_doc)]

mod driver;

#[cfg(not(test))]
#[panic_handler]
pub fn panic(info: &core::panic::PanicInfo) -> ! {
    wdk::println!("Driver panics={info}");
    loop {}
}

extern crate alloc;
extern crate utils;

#[cfg(not(test))]
use wdk_alloc::WDKAllocator;


#[cfg(not(test))]
#[global_allocator]
static GLOBAL_ALLOCATOR: WDKAllocator = WDKAllocator;