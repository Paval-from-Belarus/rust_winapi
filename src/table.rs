use std::cell::{Cell, RefCell, RefMut};
use std::ffi::c_char;
use std::{isize, ptr, usize};
use std::rc::Rc;
use winapi::ctypes::__uint8;
use winapi::shared::minwindef::{DWORD, INT, LPINT, UINT};
use winapi::shared::windef::{COLORREF, HBRUSH, HDC, HGDIOBJ, HPEN, POINT, RECT};
use winapi::um::winbase::lstrcatA;
use winapi::um::wingdi::{DeleteObject, DT_RASCAMERA, GetCharWidth32W, PS_SOLID, RC_GDI20_OUTPUT, Rectangle, RGB, TextOutW};
use winapi::um::winnt::{LONG, LPCWSTR, LPSTR, LPWSTR, WCHAR};
use winapi::um::winuser::{DrawTextW, DT_BOTTOM, DT_CENTER, DT_LEFT, DT_RIGHT, DT_TOP, DT_VCENTER, DT_WORDBREAK, FillRect, FrameRect, VK_BACK, VK_DELETE, VK_ESCAPE};
use winapi_util::console::Color;
use winapi_util::console::Intense::No;
use crate::table::CeilPropertiesType::{Pressed, Released};
use crate::utils;
use crate::utils::{get_char_width, WindowsString};

pub struct TextTable {
      rows: Vec<TextRow>,
      chosen_ceil: Option<*mut TextCeil>,
      chosen_row: Option<(*mut TextRow, usize)>,
      row_width: usize,
      row_height: usize,
      column_cnt: usize,
      table_width: usize,
      table_height: usize,
      released_properties: Box<CeilProperties>,
      pressed_properties: Box<CeilProperties>,
}

pub struct TextRow {
      row: Vec<TextCeil>,
      max_height: usize,
      min_height: usize,
      ceil_width: usize,
      rect: RECT,
      // start_x: usize,
      // //the upper lower bound of row
      // start_y: usize,
}

pub struct TextCeil {
      text: Vec<u16>,
      rect: RECT,
      //to draw
      properties: *const CeilProperties,
}

pub enum CeilPropertiesType {
      Pressed,
      Released,
}

pub struct CeilProperties {
      border: HBRUSH,
      fill: HBRUSH,
      char_width: LONG,
      char_height: LONG,
}

impl CeilProperties {
      pub fn finalize(&mut self) {
            unsafe {
                  DeleteObject(self.border as HGDIOBJ);
                  DeleteObject(self.fill as HGDIOBJ);
            }
      }
}

impl TextTable {
      ///dimensions are width and height
      pub fn new(client_window: &RECT, row_cnt: usize, column_cnt: usize) -> TextTable {
            let mut rows = Vec::<TextRow>::with_capacity(row_cnt);
            let table_width = utils::rect_width(client_window) as usize;
            let table_height = utils::rect_height(client_window) as usize;
            let row_height = table_height / row_cnt;
            let row_width = table_width;
            let mut start_y = client_window.top as usize;
            let start_x = client_window.left as usize;
            let pressed_properties = Box::new(TextTable::generate_properties(Pressed));
            let released_properties = Box::new(TextTable::generate_properties(Released));
            for _ in 0..row_cnt {
                  let mut row = TextRow::new((row_width, row_height), start_x, start_y, column_cnt);
                  row.set_properties(released_properties.as_ref());
                  rows.push(row);
                  start_y += row_height;
            }
            TextTable {
                  rows,
                  column_cnt,
                  row_height,
                  row_width,
                  table_width,
                  table_height,
                  chosen_ceil: None,
                  chosen_row: None,
                  pressed_properties,
                  released_properties,
            }
      }
      pub fn draw(&mut self, hdc: HDC) {
            self.rows.iter_mut().for_each(|row| row.draw(hdc));
      }
      pub fn set_char_properties(&mut self, char_width: LONG, char_height: LONG) {
            self.released_properties.char_width = char_width;
            self.released_properties.char_height = char_height;
            self.pressed_properties.char_width = char_width;
            self.pressed_properties.char_height = char_height;
      }
      pub fn get_caret_pos(&self) -> Option<POINT> {
            Some(POINT { x: 300, y: 150 })
      }

      pub fn resize(&mut self, hdc: HDC, client_window: &RECT) {
            let table_height = utils::rect_height(client_window) as usize;
            let table_width = utils::rect_width(client_window) as usize;
            let row_height = table_height / self.rows.len();
            let row_width = table_width;
            let mut start_y = client_window.top as usize;
            let start_x = client_window.left as usize;
            for row in self.rows.iter_mut() {
                  row.resize((row_width, row_height), start_x, start_y);
                  let shrunk_height = row.shrink(hdc, 0);
                  start_y += shrunk_height;
                  // start_y += row_height;
            }
            self.row_height = row_height;
            self.row_width = row_width;
            self.table_height = table_height;
            self.table_width = table_width;
      }
      pub fn handle_click(&mut self, x: LONG, y: LONG) {
            if x < 0 || x > (self.table_width as LONG) || y < 0 || y > (self.table_height as LONG) {
                  return;
            }
            debug_assert!(self.rows.len() > 0);
            // let row_index = usize::min(y as usize / self.row_height, self.rows.len() - 1);
            let mut chosen_row: *mut TextRow = ptr::null_mut();
            let mut row_index = self.rows.len();
            for (index, row) in self.rows.iter_mut().enumerate() {
                  if utils::point_in_rect(&row.rect, x, y) {
                        chosen_row = row;
                        row_index = index;
                        break;
                  }
            }
            if chosen_row.is_null() { //hard-coded
                  row_index = self.rows.len() - 1;
                  chosen_row = self.rows.get_mut(row_index).unwrap();
            }
            self.release_old_ceil();
            // let row = self.rows.get_mut(row_index).unwrap();
            let ceil = unsafe { (*chosen_row).ceil(x as usize) };
            ceil.set_properties(self.pressed_properties.as_ref());
            self.chosen_ceil = Some(ceil);
            self.chosen_row = Some((chosen_row, row_index));
      }
      pub fn handle_type(&mut self, hdc: HDC, key: INT) {
            if self.chosen_ceil.is_none() {
                  return; //exit if no ceil is chosen
            }
            let ceil = unsafe { &mut (*self.chosen_ceil.unwrap()) };
            let (row, row_index) = unsafe {
                  let (row, row_index) = self.chosen_row.unwrap();
                  (&mut *row, row_index)
            };
            if key == VK_ESCAPE {
                  self.release_old_ceil();
                  return;
            }
            let old_height = ceil.text_height(hdc);
            if key != VK_BACK {
                  ceil.append_letter(key as u16);
            } else {
                  ceil.erase_letter();
            }
            let new_height = ceil.text_height(hdc);
            if old_height != new_height {
                  let old_row_height = row.max_height as isize;
                  let new_row_height = row.shrink(hdc, new_height) as isize;
                  let shift_size = new_row_height - old_row_height;
                  for i in (row_index + 1)..self.rows.len() {
                        self.rows.get_mut(i).unwrap().shift(0, shift_size);
                  }
            }
      }
      pub fn scroll(&mut self, delta: INT) {

      }
      fn release_old_ceil(&mut self) {
            if let Some(last_chosen) = self.chosen_ceil {
                  unsafe {
                        (*last_chosen).set_properties(self.released_properties.as_ref());
                  }
                  self.chosen_row = None;
                  self.chosen_ceil = None;
            }
      }
      fn generate_properties(properties_type: CeilPropertiesType) -> CeilProperties {
            let pressed_border: COLORREF = RGB(41, 41, 82);
            let released_border: COLORREF = RGB(54, 61, 61);
            let pressed_fill: COLORREF = RGB(61, 193, 219);
            let released_fill: COLORREF = RGB(211, 240, 238);
            match properties_type {
                  Pressed => {
                        CeilProperties {
                              border: utils::create_solid_brush(pressed_border),
                              fill: utils::create_solid_brush(pressed_fill),
                              char_width: 1,
                              char_height: 1,
                        }
                  }
                  Released => {
                        CeilProperties {
                              border: utils::create_solid_brush(released_border),
                              fill: utils::create_solid_brush(released_fill),
                              char_width: 1,
                              char_height: 1,
                        }
                  }
            }
      }
      pub fn finalize(&mut self) {
            self.chosen_ceil = None;
            self.released_properties.finalize();
            self.pressed_properties.finalize();
      }
}

impl TextRow {
      const DEFAULT_CEIL_FORMAT: UINT = DT_CENTER;
      const DEFAULT_CEIL_RELEASED_COLOR: COLORREF = Color::Green as COLORREF;
      const DEFAULT_CEIL_PRESSED_COLOR: COLORREF = Color::Red as COLORREF;
      const DEFAULT_BORDER_WIDTH: DWORD = 3;
      ///dimension are width and height corresponding
      pub fn new(dimensions: (usize, usize), start_x: usize, start_y: usize, column_cnt: usize) -> TextRow {
            debug_assert!(column_cnt >= 1);
            let ceil_width = (dimensions.0 / column_cnt);
            let ceil_height = (dimensions.1);
            let mut ceil_rect = RECT {
                  left: start_x as LONG,
                  top: start_y as LONG,
                  right: (start_x + ceil_width) as LONG,
                  bottom: (start_y + ceil_height) as LONG,
            };
            let mut row = Vec::<TextCeil>::with_capacity(column_cnt);
            for _ in 0..column_cnt {
                  let ceil = TextCeil::new(ceil_rect.clone(), ptr::null());
                  row.push(ceil);
                  utils::offset_rect(&mut ceil_rect, ceil_width as INT, 0);
            }
            let rect = RECT { left: start_x as LONG, top: start_y as LONG, right: (start_x + dimensions.0) as LONG, bottom: (start_y + dimensions.1) as LONG };
            TextRow {
                  row,
                  max_height: ceil_height,
                  min_height: ceil_height,
                  ceil_width,
                  rect,
            }
      }
      // pub fn get_properties(&self, properties_type: CeilPropertiesType) -> *const CeilProperties {
      //       match properties_type {
      //             Pressed => { self.pressed_properties.as_ref() }
      //             Released => { self.released_properties.as_ref() }
      //       }
      // }
      fn set_properties(&mut self, properties: *const CeilProperties) {
            self.row.iter_mut().for_each(|ceil| ceil.set_properties(properties));
      }
      // pub fn set_char_properties(&mut self, char_width: LONG, char_height: LONG) {
      //
      //       //ceils holds only reference to properties
      // }
      ///as always the first element of dimension is width, the second is height
      pub fn resize(&mut self, dimensions: (usize, usize), start_x: usize, start_y: usize) {
            let ceil_width = (dimensions.0 / self.row.len());
            let ceil_height = dimensions.1;
            let mut ceil_rect = RECT {
                  left: start_x as LONG,
                  right: (start_x + ceil_width) as LONG,
                  top: start_y as LONG,
                  bottom: (start_y + ceil_height) as LONG,
            };
            for ceil in self.row.iter_mut() {
                  utils::copy_rect(&mut ceil.rect, &ceil_rect);
                  utils::offset_rect(&mut ceil_rect, ceil_width as INT, 0);
            }
            self.rect = RECT { left: start_x as LONG, top: start_y as LONG, right: (start_x + dimensions.0) as LONG, bottom: (start_y + dimensions.1) as LONG };
            self.ceil_width = ceil_width;
            self.max_height = ceil_height;
            self.min_height = ceil_height;
      }
      pub fn shift(&mut self, delta_x: isize, delta_y: isize) {
            // self.start_x += delta_x;
            // self.start_y += delta_y;
            utils::offset_rect(&mut self.rect, delta_x as INT, delta_y as INT);
            for ceil in self.row.iter_mut() {
                  utils::offset_rect(&mut ceil.rect, delta_x as INT, delta_y as INT);
            }
      }
      ///return current row height
      pub fn shrink(&mut self, hdc: HDC, height: usize) -> usize {
            if self.max_height == height { //do nothing where row is already huge
                  return self.max_height;
            }
            let max_height = self.row.iter()
                .map(|ceil| ceil.text_height(hdc))
                .max().unwrap();
            self.max_height = usize::max(max_height, self.min_height);
            self.rect.bottom = self.rect.top + self.max_height as LONG;
            self.row.iter_mut()
                .for_each(|ceil| ceil.set_height(self.max_height));
            self.max_height
      }
      //todo: repleace to change properties
      // pub fn set_format(&mut self, format: UINT) {
      //       self.row.iter_mut().for_each(|ceil| ceil.set_format(format));
      // }
      pub fn draw(&mut self, hdc: HDC) {
            for ceil in self.row.iter_mut() {
                  ceil.draw(hdc);
            }
      }
      pub fn ceil(&mut self, column_offset: usize) -> &mut TextCeil {
            debug_assert!(self.row.len() > 0);
            let ceil_index = usize::min(column_offset / self.ceil_width, self.row.len() - 1);
            // RefCell::clone(&self.row.get_mut(ceil_index))
            self.row.get_mut(ceil_index).unwrap()
      }
}

impl TextCeil {
      pub fn new(rect: RECT, properties: *const CeilProperties) -> TextCeil {
            let text = Vec::<u16>::new();
            TextCeil { rect, text, properties }
      }
      pub fn draw(&self, hdc: HDC) {
            let properties = unsafe { &(*self.properties) };
            unsafe {
                  FillRect(hdc, &self.rect, properties.fill);
                  FrameRect(hdc, &self.rect, properties.border);
            }
            // let chars_per_line = (utils::rect_width(&self.rect) / properties.char_width) as usize;
            // let mut lines_cnt = self.text.len() / chars_per_line;
            // if self.text.len() % chars_per_line != 0 {
            //       lines_cnt += 1;
            // }
            // let mut char_offset = 0;
            let mut line_offset_x = self.rect.left;
            let mut line_offset_y = self.rect.top;
            // let mut rest_chars_cnt = self.text.len();
            let line_width = utils::rect_width(&self.rect);
            let mut rest_width = line_width;
            for letter in self.text.iter() {
                  let width = get_char_width(hdc, *letter);
                  if rest_width - width < 0 {
                        line_offset_x = self.rect.left;
                        line_offset_y += properties.char_height;
                        rest_width = line_width;
                  }
                  unsafe {
                        TextOutW(hdc, line_offset_x, line_offset_y, letter, 1);
                  }
                  line_offset_x += width;
                  rest_width -= width;
                  // if rest_width - width >= 0 { //can proceed storing
                  //
                  // } else {
                  //       unsafe {
                  //             TextOutW(hdc, line_offset_x, line_offset_y, letter, 1);
                  //       }
                  // }
            }
            // for i in 0..lines_cnt {
            //       let chars_cnt = usize::min(chars_per_line, rest_chars_cnt);
            //       // let slice = &self.text[char_offset..(chars_cnt + chars_cnt)];
            //       unsafe {
            //             TextOutW(hdc, line_offset_x, line_offset_y, self.text[char_offset..(char_offset + chars_cnt)].as_ptr() as LPWSTR, chars_cnt as INT);
            //       }
            //       line_offset_y += properties.char_height;
            //       rest_chars_cnt -= chars_cnt;
            //       char_offset += chars_per_line;
            // }
      }
      // TextOutW(hdc, self.rect.left, self.rect.top, self.text.as_ptr() as LPCWSTR, self.text.len() as INT);
      // DrawTextW(hdc, self.text.as_ptr() as LPCWSTR, self.text.len() as INT, &mut self.rect as _, self.properties.text_format);

      pub fn erase_letter(&mut self) {
            // let properties = unsafe { &*self.properties };
            // let char_height = properties.char_width as usize;
            let len = self.text.len().saturating_sub(1);
            self.text.truncate(len);
      }

      pub fn append_letter(&mut self, letter: u16) {
            self.text.push(letter);
      }

      fn text_height(&self, hdc: HDC) -> usize {
            let properties = unsafe { &*self.properties };
            // let chars_per_line = (utils::rect_width(&self.rect) / (properties.char_width)) as usize;
            // let mut lines_cnt = self.text.len() / chars_per_line;//at least single line
            // if self.text.len() % chars_per_line != 0 {
            //       lines_cnt += 1;
            // }
            let lines_cnt = self.text_lines_cnt(hdc);
            lines_cnt * (properties.char_height as usize)
      }
      fn text_lines_cnt(&self, hdc: HDC) -> usize {
            let properties = unsafe { &*self.properties };
            let line_width = utils::rect_width(&self.rect);
            let mut rest_width = line_width;
            let mut lines_cnt = 1;
            for letter in self.text.iter() {
                  let letter_width = get_char_width(hdc, *letter);
                  if rest_width - letter_width >= 0 {
                        rest_width -= letter_width;
                  } else {
                        lines_cnt += 1;
                        rest_width = line_width - letter_width;
                  }
            }
            lines_cnt as usize
      }
      pub fn height(&self) -> usize {//ceil supports such invariant that height is only positive
            let rect = self.rect;
            (rect.bottom - rect.top) as usize
      }

      pub fn set_properties(&mut self, properties: *const CeilProperties) {
            self.properties = properties;
      }

      pub fn set_height(&mut self, height: usize) {
            self.rect.bottom = self.rect.top + height as LONG;
      }
}