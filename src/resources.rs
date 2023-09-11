use std::collections::HashMap;

use winapi::shared::minwindef::{HMODULE, UINT, WORD};
use winapi::um::winnt::{HANDLE, IMAGE_COR_MIH_EHRVA};
use winapi::um::winuser::{IMAGE_BITMAP, IMAGE_ICON, LoadImageW, LR_DEFAULTCOLOR, MAKEINTRESOURCEW};
use crate::utils::show_error_message;


pub const TITLE_ICON: WORD = 100;
pub const BACKGROUND_BITMAP: WORD = 101;
pub const HERO_FORE_BITMAP: WORD = 102;
pub const HERO_MASK_BITMAP: WORD = 103;

pub fn load_resources(module_handle: HMODULE) -> Result<HashMap<WORD, usize>, ()> {
      let mut map = HashMap::<WORD, usize>::new();
      let icon = load_image(module_handle, TITLE_ICON, IMAGE_ICON);
      let hero_fore_bitmap = load_image(module_handle, HERO_FORE_BITMAP, IMAGE_BITMAP);
      let hero_mask_bitmap = load_image(module_handle, HERO_MASK_BITMAP, IMAGE_BITMAP);
      let background_bitmap = load_image(module_handle, BACKGROUND_BITMAP, IMAGE_BITMAP);
      let result;
      if !icon.is_null() && !hero_fore_bitmap.is_null() && !background_bitmap.is_null() && !hero_mask_bitmap.is_null() {
            map.insert(TITLE_ICON, icon as usize);
            map.insert(HERO_FORE_BITMAP, hero_fore_bitmap as usize);
            map.insert(HERO_MASK_BITMAP, hero_mask_bitmap as usize);
            map.insert(BACKGROUND_BITMAP, background_bitmap as usize);
            result = Ok(map);
      } else {
            show_error_message("Failed to load resources");
            result = Err(());
      }
      result
}

fn load_image(module_handle: HMODULE, resource_id: WORD, resource_type: UINT) -> HANDLE {
      unsafe {
            LoadImageW(module_handle,
                       MAKEINTRESOURCEW(resource_id),
                       resource_type,
                       0, 0, LR_DEFAULTCOLOR)
      }
}