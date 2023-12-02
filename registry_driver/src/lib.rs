#![no_std]

mod driver;

#[cfg(not(test))]
#[panic_handler]
pub fn panic(info: &core::panic::PanicInfo) -> ! {
    wdk::println!("Driver panics={info}");
    loop {}
}

extern crate alloc;

#[cfg(not(test))]
use wdk_alloc::WDKAllocator;


#[cfg(not(test))]
#[global_allocator]
static GLOBAL_ALLOCATOR: WDKAllocator = WDKAllocator;