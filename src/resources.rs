use std::collections::HashMap;
use winapi::ctypes::__uint8;
use winapi::shared::minwindef::{HMODULE, UINT, WORD};
use winapi::um::winnt::HANDLE;
use winapi::um::winuser::{IMAGE_BITMAP, IMAGE_ICON, LoadImageW, LR_LOADFROMFILE, MAKEINTRESOURCEW};
use crate::utils::WindowsString;


pub const TITLE_ICON: WORD = 100;
pub const BACKGROUND_BITMAP: WORD = 101;
pub const HERO_BITMAP: WORD = 102;

const RESOURCES_CNT: usize = 3;

pub fn load_resources(module_handle: HMODULE) -> HashMap<WORD, usize> {
      let mut map = HashMap::<WORD, usize>::new();
      let icon = load_image(module_handle, TITLE_ICON, IMAGE_ICON);
      let hero_bitmap = load_image(module_handle, HERO_BITMAP, IMAGE_BITMAP);
      let background_bitmap = load_image(module_handle, BACKGROUND_BITMAP, IMAGE_BITMAP);
      assert!(!icon.is_null() && !hero_bitmap.is_null() && !background_bitmap.is_null());
      map.insert(TITLE_ICON, icon as usize);
      map.insert(HERO_BITMAP, hero_bitmap as usize);
      map.insert(BACKGROUND_BITMAP, background_bitmap as usize);
      map
}

fn load_image(module_handle: HMODULE, resource_id: WORD, resource_type: UINT) -> HANDLE {
      unsafe {
            LoadImageW(module_handle,
                       MAKEINTRESOURCEW(resource_id),
                       resource_type,
                       0, 0, 0)
      }
}