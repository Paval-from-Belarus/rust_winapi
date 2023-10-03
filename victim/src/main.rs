extern crate utils;

use std::thread::sleep;
use std::time::Duration;

fn main() {
    let victim_string = b"HEAP_STRING\0".to_vec();
    utils::show_alert_message("First appearance: ", String::from_utf8(victim_string.to_vec()).unwrap().as_str());
    utils::show_alert_message("Process...", "Our program is too busy...");
    sleep(Duration::from_millis(100));
    utils::show_alert_message("Second appearance: ", String::from_utf8(victim_string.to_vec()).unwrap().as_str());
}
