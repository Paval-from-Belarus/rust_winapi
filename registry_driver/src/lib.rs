#![feature(new_uninit)]
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