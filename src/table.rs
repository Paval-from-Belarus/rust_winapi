use std::cell::{Cell, RefCell, RefMut};
use std::rc::Rc;
use winapi::shared::minwindef::{DWORD, INT, UINT};
use winapi::shared::windef::{COLORREF, HBRUSH, HDC, HGDIOBJ, HPEN, POINT, RECT};
use winapi::um::wingdi::{DeleteObject, DT_RASCAMERA, PS_SOLID, RC_GDI20_OUTPUT, Rectangle, RGB, TextOutW};
use winapi::um::winnt::{LONG, LPCWSTR, WCHAR};
use winapi::um::winuser::{DrawTextW, DT_BOTTOM, DT_CENTER, DT_LEFT, DT_RIGHT, DT_TOP, DT_VCENTER, DT_WORDBREAK, FillRect, FrameRect, VK_BACK, VK_DELETE};
use winapi_util::console::Color;
use winapi_util::console::Intense::No;
use crate::table::CeilPropertiesType::{Pressed, Released};
use crate::utils;
use crate::utils::WindowsString;

pub struct TextTable {
      rows: Vec<TextRow>,
      chosen_ceil: Option<*mut TextCeil>,
      row_width: usize,
      row_height: usize,
      column_cnt: usize,
      table_width: usize,
      table_height: usize,
}

pub struct TextRow {
      row: Vec<TextCeil>,
      max_height: usize,
      ceil_width: usize,
      released_properties: CeilProperties,
      pressed_properties: CeilProperties,
      // start_x: usize,
      // //the upper lower bound of row
      // start_y: usize,
}

pub struct TextCeil {
      text: Vec<u16>,
      rect: RECT,
      //to draw
      properties: CeilProperties,
}

pub enum CeilPropertiesType {
      Pressed,
      Released,
}

#[derive(Clone)]
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
            for _ in 0..row_cnt {
                  let row = TextRow::new((row_width, row_height), start_x, start_y, column_cnt);
                  rows.push(row);
                  start_y += row_height;
            }
            TextTable { rows, column_cnt, row_height, row_width, table_width, table_height, chosen_ceil: None }
      }
      pub fn draw(&mut self, hdc: HDC) {
            self.rows.iter_mut().for_each(|row| row.draw(hdc));
      }
      pub fn set_char_properties(&mut self, char_width: LONG, char_height: LONG) {
            self.rows.iter_mut().for_each(|row| row.set_char_properties(char_width, char_height));
      }
      pub fn get_caret_pos(&self) -> Option<POINT> {
            Some(POINT { x: 300, y: 150 })
      }

      pub fn resize(&mut self, client_window: &RECT) {
            let table_height = utils::rect_height(client_window) as usize;
            let table_width = utils::rect_width(client_window) as usize;
            let row_height = table_height / self.rows.len();
            let row_width = table_width;
            let mut start_y = client_window.top as usize;
            let start_x = client_window.left as usize;
            for row in self.rows.iter_mut() {
                  row.resize((row_width, row_height), start_x, start_y);
                  start_y += row_height;
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
            let row_index = usize::min(y as usize / self.row_height, self.rows.len() - 1);
            let row = self.rows.get_mut(row_index).unwrap();
            if let Some(last_chosen) = self.chosen_ceil {
                  unsafe {
                        (*last_chosen).set_properties(row.get_properties(Released));
                  }
            }
            let properties = row.get_properties(Pressed);
            let ceil = row.ceil(x as usize);
            ceil.set_properties(properties);
            self.chosen_ceil = Some(ceil);
      }
      pub fn handle_type(&mut self, key: WCHAR) {
            // if key != VK_DELETE && key != VK_BACK {
            //
            // } else {
            //
            // }
      }
      pub fn erase_letter(&mut self) {
            if let Some(ceil) = self.chosen_ceil {
                  unsafe {}
            }
      }
      pub fn finalize(&mut self) {
            self.chosen_ceil = None;
            self.rows.iter_mut().for_each(|row| row.finalize());
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
            let pressed_properties = TextRow::generate_properties(Pressed);
            let released_properties = TextRow::generate_properties(Released);
            let mut row = Vec::<TextCeil>::with_capacity(column_cnt);
            for _ in 0..column_cnt {
                  let ceil = TextCeil::new(ceil_rect.clone(), released_properties.clone());
                  row.push(ceil);
                  utils::offset_rect(&mut ceil_rect, ceil_width as INT, 0);
            }
            TextRow {
                  row,
                  max_height: ceil_height,
                  ceil_width,
                  pressed_properties,
                  released_properties,
            }
      }
      pub fn get_properties(&self, properties_type: CeilPropertiesType) -> CeilProperties {
            match properties_type {
                  Pressed => { self.pressed_properties.clone() }
                  Released => { self.released_properties.clone() }
            }
      }
      pub fn set_char_properties(&mut self, char_width: LONG, char_height: LONG) {
            self.released_properties.char_width = char_width;
            self.released_properties.char_height = char_height;
            self.pressed_properties.char_width = char_width;
            self.released_properties.char_height = char_height;
            //ceils holds only reference to properties
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
            self.ceil_width = ceil_width;
            self.max_height = ceil_height;
      }
      pub fn shift(&mut self, delta_x: isize, delta_y: isize) {
            // self.start_x += delta_x;
            // self.start_y += delta_y;
            for ceil in self.row.iter_mut() {
                  utils::offset_rect(&mut ceil.rect, delta_x as INT, delta_y as INT);
            }
      }
      ///return current row height
      pub fn shrink(&mut self, height: usize) -> usize {
            if self.max_height >= height { //do nothing where row is already huge
                  return self.max_height;
            }
            let max_height = self.row.iter()
                .map(|ceil| ceil.height())
                .max().unwrap();
            self.max_height = max_height;
            self.row.iter_mut()
                .for_each(|ceil| ceil.set_height(max_height));
            self.max_height
      }
      //todo: repleace to change properties
      // pub fn set_format(&mut self, format: UINT) {
      //       self.row.iter_mut().for_each(|ceil| ceil.set_format(format));
      // }
      pub fn draw(&mut self, hdc: HDC) {
            self.row.iter_mut().for_each(|ceil| ceil.draw(hdc));
      }
      pub fn ceil(&mut self, column_offset: usize) -> &mut TextCeil {
            debug_assert!(self.row.len() > 0);
            let ceil_index = usize::min(column_offset / self.ceil_width, self.row.len() - 1);
            // RefCell::clone(&self.row.get_mut(ceil_index))
            self.row.get_mut(ceil_index).unwrap()
      }
      pub fn finalize(&mut self) {
            self.released_properties.finalize();
            self.pressed_properties.finalize();
      }
}

impl TextCeil {
      pub fn new(rect: RECT, properties: CeilProperties) -> TextCeil {
            let text = String::from("Aasdfasdff adfa").as_os_str();
            TextCeil { rect, text, properties }
      }
      pub fn draw(&self, hdc: HDC) {
            unsafe {
                  FillRect(hdc, &self.rect, self.properties.fill);
                  FrameRect(hdc, &self.rect, self.properties.border);
                  TextOutW(hdc, self.rect.left, self.rect.top, self.text.as_ptr() as LPCWSTR, self.text.len() as INT);
                  // DrawTextW(hdc, self.text.as_ptr() as LPCWSTR, self.text.len() as INT, &mut self.rect as _, self.properties.text_format);
            }
      }
      pub fn text_height(&self) -> usize {
            let chars_per_line = utils::rect_width(&self.rect) / self.properties.char_width;
            let lines_cnt = self.text.len() / (chars_per_line as usize) + 1;//at least single line
            lines_cnt * (self.properties.char_height as usize)
      }
      pub fn height(&self) -> usize {//ceil supports such invariant that height is only positive
            let rect = self.rect;
            (rect.bottom - rect.top) as usize
      }
      pub fn set_properties(&mut self, properties: CeilProperties) {
            self.properties = properties;
      }
      pub fn set_height(&mut self, height: usize) {
            self.rect.bottom = self.rect.top + height as LONG;
      }
}