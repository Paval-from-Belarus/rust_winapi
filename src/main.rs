#![cfg(windows)]
// Let's put this so that it won't open the console
#![windows_subsystem = "windows"]
#![allow(non_snake_case)]

mod resources;
mod utils;

use std::error::Error;
use std::mem::MaybeUninit;
use winapi::shared::minwindef::*;
use winapi::shared::windef::*;
use winapi::um::libloaderapi::{GetModuleFileNameW, GetModuleHandleW};
use winapi::um::winuser::*;
use winapi::um::mmsystem::*;
use winapi::um::mmeapi::*;
use winapi::um::winnt::{LONG, PCWSTR, PPROCESS_MITIGATION_IMAGE_LOAD_POLICY};
use std::ptr;
use std::sync::{Arc, Mutex};
use winapi::um::winbase::UMS_THREAD_INFO_CLASS;
use winapi::um::wingdi::{BitBlt, BITMAP, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, EndPage, Rectangle, RestoreDC, SaveDC, SelectObject, SRCCOPY};
use crate::resources::{HERO_BITMAP, TITLE_ICON};
use crate::utils::{createMainWindow, FormParams, WindowsString};

//get a win32 lpstr from a &str, converting u8 to u16 and appending '\0'

unsafe fn onLeftButtonDown(hWindow: HWND) {
    let hInstance = GetModuleHandleW(ptr::null_mut());
    let mut name: Vec<u16> = Vec::with_capacity(MAX_PATH);
    let read_len = GetModuleFileNameW(hInstance, name.as_mut_ptr(), MAX_PATH as u32);
    name.set_len(read_len as usize);
    // We could convert name to String using String::from_utf16_lossy(&name)
    MessageBoxW(
        hWindow,
        name.as_ptr(),
        "This program is:".as_os_str().as_ptr(),
        MB_OK | MB_ICONINFORMATION,
    );
}

static mut HERO: Option<FlyHero> = Option::None;
static mut BACK_BUFFER: Option<BackBuffer> = Option::None;
// Window procedure function to handle events
pub unsafe extern "system" fn MainWindowProc(hWindow: HWND, msg: UINT, wParam: WPARAM, lParam: LPARAM) -> LRESULT {gi
    match msg {
        WM_PAINT => {
            // let (hero, backBuffer) = unsafe {
            //     (HERO.unwrap(), BACK_BUFFER.unwrap())
            // };
            // let mut paintStruct = MaybeUninit::<PAINTSTRUCT>::uninit();
            // hero.draw(backBuffer.hdc);
            // let hdc = BeginPaint(hWindow, paintStruct.as_mut_ptr());
            // let psBoard = paintStruct.assume_init();
            // BitBlt(hdc, 0, 0, 800, 800, backBuffer.hdc, 0, 0, SRCCOPY);
            // EndPaint(hWindow, &psBoard);
        }
        WM_CLOSE => {
            DestroyWindow(hWindow);
        }
        WM_DESTROY => {
            PostQuitMessage(0);
        }
        WM_LBUTTONDOWN => {
            onLeftButtonDown(hWindow);
        }
        _ => return DefWindowProcW(hWindow, msg, wParam, lParam),
    }
    return 0;
}


#[derive(Copy, Clone)]
pub struct BackBuffer {
    hdc: HDC,
    hBitmap: HBITMAP,
}

#[derive(Copy, Clone)]
struct FlyHero {
    rect: RECT,
    sprite: HBITMAP,
}

impl FlyHero {
    const DEFAULT_WIDTH: LONG = 100;
    const DEFAULT_HEIGHT: LONG = 100;
    pub fn new(x: LONG, y: LONG, sprite: HBITMAP) -> FlyHero {
        FlyHero {
            rect: RECT {
                left: x,
                top: y + FlyHero::DEFAULT_HEIGHT,
                right: x + FlyHero::DEFAULT_WIDTH,
                bottom: y,
            },
            sprite,
        }
    }
    pub fn draw(&self, hdc: HDC) {
        let positions = self.rect;
        unsafe {
            SaveDC(hdc);
            SelectObject(hdc, self.sprite as HGDIOBJ);
            Rectangle(hdc, positions.left, positions.top, positions.right, positions.bottom);
            RestoreDC(hdc, -1); //restore previous hdc
        }
    }
}

impl BackBuffer {
    pub fn create(hWindow: HWND, width: INT, height: INT) -> BackBuffer {
        let (hdc, hBitmap) = unsafe {
            let hdcWindow = GetDC(hWindow);
            let hdc = CreateCompatibleDC(hdcWindow);
            let hBitmap = CreateCompatibleBitmap(hdcWindow, width, height);
            SaveDC(hdc);
            SelectObject(hdc, hBitmap as HGDIOBJ);
            ReleaseDC(hWindow, hdcWindow);
            (hdc, hBitmap)
        };
        return BackBuffer {
            hdc,
            hBitmap,
        };
    }
    pub fn release(self) {
        unsafe {
            RestoreDC(self.hdc, -1);
            DeleteObject(self.hBitmap as HGDIOBJ);
            DeleteDC(self.hdc);
        }
    }
}


// Message handling loop
fn messageDispatchLoop(hWindow: HWND) -> WPARAM {
    unsafe {
        let mut msg = MaybeUninit::<MSG>::uninit();
        while GetMessageW(msg.as_mut_ptr(), hWindow, 0, 0) > 0 {
            let msg = msg.assume_init();
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let msg = msg.assume_init();
        msg.wParam
    }
}


fn main() {
    let params = FormParams::getDefaultParams();
    let hMainWindow = unsafe {
        createMainWindow("Cool piano", "Piano cool", Some(MainWindowProc), &params)
            .expect("create_main_window method failed")
    };
    let hInstance = unsafe {
        GetModuleHandleW(ptr::null_mut())
    };
    let hero = unsafe {
        let bitmap = LoadImageW(hInstance, MAKEINTRESOURCEW(HERO_BITMAP),
                                IMAGE_BITMAP, 0, 0, LR_CREATEDIBSECTION) as HBITMAP;
        FlyHero::new(0, 0, bitmap)
    };
    let backBuffer = BackBuffer::create(hMainWindow, params.width, params.height);
    unsafe {
        // HERO = Some(hero);
        BACK_BUFFER = Some(backBuffer);
    }
    unsafe {
        ShowWindow(hMainWindow, SW_SHOW);
        UpdateWindow(hMainWindow);
    }
    messageDispatchLoop(hMainWindow);
}