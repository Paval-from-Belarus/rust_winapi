extern crate utils;
extern crate thread_pool;

use std::{env, mem, ptr, slice};
use std::cmp::{max, min};
use std::ptr::copy_nonoverlapping;
use winapi::um::fileapi::{CreateFileW, OPEN_EXISTING};
use winapi::um::handleapi::INVALID_HANDLE_VALUE;
use winapi::um::memoryapi::{CreateFileMappingW, FILE_MAP_ALL_ACCESS, MapViewOfFile};
use winapi::um::mmeapi::midiOutLongMsg;
use winapi::um::processthreadsapi::PPROC_THREAD_ATTRIBUTE_LIST;
use winapi::um::winbase::{FILE_FLAG_RANDOM_ACCESS, lstrcpyA};
use winapi::um::wingdi::wglGetLayerPaletteEntries;
use winapi::um::winnt::{FILE_ALL_ACCESS, FILE_ATTRIBUTE_NORMAL, HANDLE, INT, PAGE_READWRITE, SUBLANG_SOTHO_NORTHERN_SOUTH_AFRICA};
use thread_pool::{Task, TaskHandler, ThreadPool};
use utils::{bitflags, WindowsString};

struct FileMappingRec {
    file_handle: HANDLE,
    map_handle: HANDLE,
    map_view: *mut u8,
}

struct Sorter<'a> {
    file_offset: *mut u8,
    file_size: usize,
    pool: Option<ThreadPool>,
    array: Vec<&'a [u8]>,
}
bitflags!(
    pub FileMappingError(usize),
    NOT_FOUND = 1,
    EMPTY_FILE = 2,
    MAPPINT_ERROR = 3,
    MAP_VIEW_ERROR = 4
);
struct SortParams<'a> {
    min_pivot: usize,
    max_pivot: usize,
    array: *mut Vec<&'a [u8]>,
    pool: *mut ThreadPool,
}


impl<'a> Sorter<'a> {
    const SIZE_THRESHOLD: usize = 300;
    const MAX_WORKERS_COUNT: usize = 20;
    pub fn new(file_offset: *mut u8, file_size: usize) -> Self {
        let workers_count = usize::min((file_size / Sorter::SIZE_THRESHOLD) / 2, Sorter::MAX_WORKERS_COUNT);
        let average_task_count = workers_count * 2;
        let pool;
        if workers_count <= 1 {
            pool = None;
        } else {
            pool = Some(ThreadPool::new(average_task_count, workers_count));
        }
        let bytes = unsafe { slice::from_raw_parts(file_offset, file_size) };
        let array = std::str::from_utf8(bytes).expect("The source file contains not utf8 symbols")
            .split("\n")
            .map(|line| line.as_bytes())
            .collect::<Vec<&[u8]>>();
        // let mut last_offset = file_offset;
        // let mut length = 0;
        // let mut array = Vec::<&[u8]>::new();
        // for byte in bytes.iter() {
        //     length += 1;
        //     if length > 1 && byte.eq(&b'\n') {
        //         let next_offset = unsafe { last_offset.add(length) }; //next offset of string
        //         let small_string = unsafe { slice::from_raw_parts(last_offset, length - 1) };//only data without last space
        //         array.push(small_string);
        //         length = 0;
        //         last_offset = next_offset;
        //     }
        // }
        // if length != 0 {
        //     let small_string = unsafe { slice::from_raw_parts(last_offset, length) }; //the last char is not space, so, we can use it
        //     array.push(small_string);
        // }
        Sorter { file_offset, file_size, pool, array }
    }

    pub fn sort(&mut self) {
        let mut thread_pool: *mut ThreadPool = ptr::null_mut();
        if let Some(pool) = &mut self.pool {
            thread_pool = pool;
        }
        let params = SortParams {
            min_pivot: 0,
            max_pivot: self.array.len().saturating_sub(1),
            array: &mut self.array,
            pool: thread_pool,
        };
        Sorter::sort_task(&params);
        let mut buffer = Vec::<Vec<u8>>::with_capacity(self.array.len());
        for value in self.array.iter() {
            let mut cloned = Vec::with_capacity(value.len());
            value.iter().for_each(|byte| cloned.push(*byte));
            buffer.push(cloned);
        }
        let mut copy_ptr = self.file_offset;
        let last_offset = unsafe { self.file_offset.add(self.file_size) };
        for cloned_str in buffer {
            for byte in cloned_str {
                if copy_ptr.eq(&last_offset) {
                    return;
                }
                unsafe {
                    copy_ptr.write(byte);
                    copy_ptr = copy_ptr.add(1);
                }
            }
            if copy_ptr.eq(&last_offset) {
                return;
            }
            // if !copy_ptr.eq(&last_offset) { //to prevent issues
            unsafe {
                copy_ptr.write(b'\n');
                copy_ptr = copy_ptr.add(1);
            }
            // }
        }
    }
    pub fn sort_task(params_ptr: *const SortParams) {
        let params = unsafe { &*params_ptr };
        let array = unsafe { &mut *params.array };
        if params.pool.is_null() || (params.max_pivot - params.min_pivot + 1) < Sorter::SIZE_THRESHOLD * 2 {
            array[params.min_pivot..=params.max_pivot].sort();
            return;
        }
        let pool = unsafe { &mut *params.pool };
        let min_pivot = params.min_pivot;
        let middle_pivot = min_pivot + Sorter::SIZE_THRESHOLD;
        let max_pivot = params.max_pivot;
        let min_params = SortParams { min_pivot, max_pivot: middle_pivot, pool, array };
        let max_params = SortParams { min_pivot: middle_pivot + 1, max_pivot, pool, array };
        let task_handler = unsafe { mem::transmute::<fn(*const SortParams), TaskHandler>(Self::sort_task) };
        let min_future = pool.submit(task_handler, &min_params as *const SortParams as _);
        let max_future = pool.submit(task_handler, &max_params as *const SortParams as _);
        min_future.get();
        max_future.get();
        let (left, right) = array.split_at(middle_pivot);
        // let left: Vec<&[u8]> = array[min_pivot..=middle_pivot].to_vec();
        // let right: Vec<&[u8]> = array[(middle_pivot + 1)..=max_pivot].to_vec();
        let merged = Sorter::merge(left, right);
        array.splice(min_pivot..=max_pivot, merged);
    }
    pub fn merge(left: &[&'a[u8]], right: &[&'a [u8]]) -> Vec<&'a [u8]> {
        let min_length = usize::min(left.len(), right.len());
        let mut target: Vec<&[u8]> = Vec::with_capacity(left.len() + right.len());
        let mut left_pivot = 0;
        let mut right_pivot = 0;
        while left_pivot < min_length && right_pivot < min_length {
            let left_str = left.get(left_pivot).unwrap();
            let right_str = right.get(right_pivot).unwrap();
            if left_str.cmp(right_str).is_lt() {
                target.push(left_str);
                left_pivot += 1;
            } else {
                target.push(right_str);
                right_pivot += 1;
            }
        }
        if left_pivot < left.len() {
            left[left_pivot..left.len()].iter().for_each(|value| target.push(value));
        }
        if right_pivot < right.len() {
            right[right_pivot..right.len()].iter().for_each(|value| target.push(value));
        }
        target
    }
}


impl FileMappingRec {
    pub fn new(file_name: &str) -> Result<Self, FileMappingError> {
        let file_handle = unsafe {
            CreateFileW(
                file_name.as_os_str().as_ptr(),
                FILE_ALL_ACCESS,
                0, //not shared
                ptr::null_mut(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL | FILE_FLAG_RANDOM_ACCESS,
                ptr::null_mut(),
            )
        };
        if file_handle == INVALID_HANDLE_VALUE {
            utils::close_handle(file_handle);
            return Err(FileMappingError::from(FileMappingError::NOT_FOUND));
        }
        if utils::get_file_size(file_handle).is_err() {
            utils::close_handle(file_handle);
            return Err(FileMappingError::from(FileMappingError::EMPTY_FILE));
        }
        let map_handle = unsafe {
            CreateFileMappingW(
                file_handle,
                ptr::null_mut(),
                PAGE_READWRITE,
                0,
                0,
                ptr::null_mut(),
            )
        };
        if map_handle.is_null() {
            utils::close_handle(file_handle);
            return Err(FileMappingError::from(FileMappingError::MAPPINT_ERROR));
        }
        let map_view = unsafe {
            MapViewOfFile(
                map_handle,
                FILE_MAP_ALL_ACCESS,
                0,
                0,
                0,
            )
        } as *mut u8;
        if map_view.is_null() {
            utils::close_handle(map_handle);
            utils::close_handle(file_handle);
            return Err(FileMappingError::from(FileMappingError::MAP_VIEW_ERROR));
        }
        Ok(FileMappingRec { file_handle, map_handle, map_view })
    }
    pub fn get_map_view_offset(&self) -> *mut u8 {
        self.map_view
    }
    pub fn get_file_size(&self) -> usize {
        utils::get_file_size(self.file_handle).unwrap() as usize
    }
}


impl Drop for FileMappingRec {
    fn drop(&mut self) {
        utils::unmap_file_view(self.map_view);
        utils::close_handle(self.map_handle);
        utils::close_handle(self.file_handle);
    }
}

fn main() {
    let source_file_name: String = text_io::read!();
    // let args: Vec<String> = env::args().collect();
    // if args.capacity() != 1 {
    //     println!("Not enough params. Source file is expected");
    //     return;
    // }
    // let source_file_name = args.get(0).unwrap();
    let file = FileMappingRec::new(source_file_name.as_str()).expect("File mapping is failed");
    let mut sorter = Sorter::new(file.get_map_view_offset(), file.get_file_size());
    sorter.sort();
}

#[cfg(test)]
mod tests {
    use std::mem;
    use super::*;

    #[test]
    fn it_works() {
        let mut array: Vec<&[u8]> = vec!(&[10, 2, 4], &[10, 2], &[1, 2, 3]);
        let params = SortParams {
            min_pivot: 0,
            max_pivot: 2,
            array: &mut array,
            pool: ptr::null_mut(),
        };
        Sorter::sort_task(&params);
        let sorted: Vec<&[u8]> = vec!(&[1, 2, 3], &[10, 2], &[10, 2, 4]);
        assert_eq!(array, sorted);
        test_range_sorting();
    }

    fn test_range_sorting() {
        let mut array: Vec<&[u8]> = vec!(&[10, 2, 4], &[10, 2], &[1, 2, 3]);
        let params = SortParams {
            min_pivot: 1,
            max_pivot: 2,
            array: &mut array,
            pool: ptr::null_mut(),
        };
        Sorter::sort_task(&params);
        let sorted: Vec<&[u8]> = vec!(&[10, 2, 4], &[1, 2, 3], &[10, 2]);
        assert_eq!(array, sorted);
    }
}