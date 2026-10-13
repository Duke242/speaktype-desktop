//! Which app is in front, so dictation can match its tone: the foreground
//! window's executable name and title.

use std::{ffi::c_void, path::Path};

use crate::platform::FrontApp;

type Hwnd = *mut c_void;
type Handle = *mut c_void;

const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
/// Longest title or path read, in UTF-16 units.
const MAX_CHARS: usize = 1024;

#[link(name = "user32")]
unsafe extern "system" {
    fn GetForegroundWindow() -> Hwnd;
    fn GetWindowThreadProcessId(window: Hwnd, process_id: *mut u32) -> u32;
    fn GetWindowTextW(window: Hwnd, text: *mut u16, max_count: i32) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn OpenProcess(desired_access: u32, inherit_handle: i32, process_id: u32) -> Handle;
    fn QueryFullProcessImageNameW(
        process: Handle,
        flags: u32,
        exe_name: *mut u16,
        size: *mut u32,
    ) -> i32;
    fn CloseHandle(object: Handle) -> i32;
}

pub fn frontmost_app() -> Option<FrontApp> {
    // SAFETY: no arguments; returns a borrowed HWND or null.
    let window = unsafe { GetForegroundWindow() };
    if window.is_null() {
        return None;
    }
    let mut title = vec![0u16; MAX_CHARS];
    // SAFETY: the buffer holds MAX_CHARS units, and the count passed matches it.
    let len = unsafe { GetWindowTextW(window, title.as_mut_ptr(), MAX_CHARS as i32) };
    let title = String::from_utf16_lossy(&title[..len.clamp(0, MAX_CHARS as i32) as usize]);

    let mut pid = 0u32;
    // SAFETY: window is a window handle and pid a valid place to write to.
    unsafe { GetWindowThreadProcessId(window, &mut pid) };
    let exe = if pid != 0 { executable(pid) } else { None };
    let stem = exe
        .as_deref()
        .and_then(|path| Path::new(path).file_stem())
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    Some(FrontApp {
        id: stem.clone(),
        name: stem,
        title,
    })
}

/// The full path of the process's executable. Works for most processes
/// without extra rights, though not for some elevated ones.
fn executable(pid: u32) -> Option<String> {
    // SAFETY: plain call; a null handle means it failed.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return None;
    }
    let mut path = vec![0u16; MAX_CHARS];
    let mut size = MAX_CHARS as u32;
    // SAFETY: process is an open handle, and size is the buffer's length in
    // units; on success it's set to the length written.
    let ok = unsafe { QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut size) };
    // SAFETY: process was opened above and is closed once.
    unsafe { CloseHandle(process) };
    (ok != 0).then(|| String::from_utf16_lossy(&path[..(size as usize).min(MAX_CHARS)]))
}
