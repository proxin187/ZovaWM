use crate::wm::Monitor;
use crate::wm::Bar;

pub use x11::xlib::{XA_WINDOW, XA_CARDINAL, XA_ATOM};
pub use x11::xlib::{Mod4Mask, Button1, Button3};
use x11::xinerama;
use x11::xrender;
use x11::xlib;
use x11::xft;

use std::ffi::CStr;
use std::process;
use std::mem;
use std::ptr;

struct WindowProperty {
    data: *mut u8,
    length: u64,
}

#[derive(Debug)]
pub struct Pointer {
    pub x: i32,
    pub y: i32,
}

pub struct Display {
    ptr: *mut xlib::_XDisplay,
    screen: i32,
    pub root: u64,
}

impl Drop for Display {
    fn drop(&mut self) {
        self.close();
    }
}

impl Display {
    pub fn open(ptr: *const i8) -> Result<Display, Box<dyn std::error::Error>> {
        let ptr = unsafe { xlib::XOpenDisplay(ptr) };

        if ptr.is_null() {
            Err("failed to open display".into())
        } else {
            unsafe {
                let root = xlib::XDefaultRootWindow(ptr);
                let screen = xlib::XDefaultScreen(ptr);

                xlib::XSetErrorHandler(Some(Self::handle_error));

                Ok(Display {
                    ptr,
                    screen,
                    root,
                })
            }
        }
    }

    /*
     * We want to ignore BadWindow errors as there is no way to check whether a window is valid,
     * this makes BadWindow errors common especially from UnmapNotify events
    */
    unsafe extern "C" fn handle_error(ptr: *mut xlib::_XDisplay, error: *mut xlib::XErrorEvent) -> i32 {
        unsafe {
            if (*error).error_code == xlib::BadWindow {
                println!("[+] non-fatal error: req_code: {}, err_code: {}", (*error).request_code, (*error).error_code);

                return 0;
            } else {
                println!("[+] fatal error: req_code: {}, err_code: {}", (*error).request_code, (*error).error_code);

                let mut buffer: [i8; 100] = [0i8; 100];
                xlib::XGetErrorText(ptr, (*error).error_code as i32, buffer.as_mut_ptr(), 100);

                println!("[+] {}", String::from_utf8(buffer.map(|x| x as u8).to_vec()).unwrap_or_default());

                process::exit(1);
            }
        }
    }

    pub fn close(&mut self) {
        unsafe {
            xlib::XCloseDisplay(self.ptr);
        }
    }

    pub fn sync(&mut self) {
        unsafe {
            xlib::XSync(self.ptr, xlib::True);
        }
    }

    pub fn get_monitors(&mut self) -> Result<Vec<Monitor>, Box<dyn std::error::Error>> {
        let mut monitors: Vec<Monitor> = Vec::new();

        unsafe {
            if xinerama::XineramaIsActive(self.ptr) == xlib::True {
                let mut xmonitor_count = 0;

                let xmonitors = xinerama::XineramaQueryScreens(self.ptr, &mut xmonitor_count);

                for index in 0..xmonitor_count {
                    let xmonitor =  *xmonitors.offset(index as isize);

                    monitors.push(Monitor {
                        x: xmonitor.x_org as i32,
                        y: xmonitor.y_org as i32,
                        width: xmonitor.width as u32,
                        height: xmonitor.height as u32,
                        clients: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
                        fullscreen: None,
                        workspace: 0,
                        bar: self.create_bar(xmonitor.x_org as i32, xmonitor.width as u32)?,
                    });
                }
            } else {
                let width = self.display_width();

                monitors.push(Monitor {
                    x: 0,
                    y: 0,
                    width,
                    height: self.display_height(),
                    clients: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
                    fullscreen: None,
                    workspace: 0,
                    bar: self.create_bar(0, width)?,
                });
            }
        }

        Ok(monitors)
    }

    pub fn query_pointer(&mut self) -> Pointer {
        unsafe {
            let mut root_return = self.root;
            let mut root_x = 0;
            let mut root_y = 0;

            xlib::XQueryPointer(
                self.ptr,
                root_return,
                &mut root_return,
                &mut root_return,
                &mut root_x,
                &mut root_y,
                &mut 0,
                &mut 0,
                &mut 0,
            );

            Pointer {
                x: root_x,
                y: root_y,
            }
        }
    }

    pub fn frame_window(&mut self, window: u64) {
        unsafe {
            let mut attr: xlib::XWindowAttributes = mem::zeroed();
            xlib::XGetWindowAttributes(self.ptr, window, &mut attr);

            let frame = xlib::XCreateSimpleWindow(
                self.ptr,
                self.root,
                attr.x,
                attr.y,
                attr.width as u32,
                attr.height as u32,
                3,        // border width
                0xff0000, // border color
                0x0000ff, // border bg color
            );

            xlib::XSelectInput(self.ptr, frame, xlib::SubstructureRedirectMask | xlib::SubstructureNotifyMask);

            xlib::XReparentWindow(
                self.ptr,
                window,
                frame,
                0,
                0,
            );

            xlib::XMapWindow(self.ptr, frame);
        }
    }

    pub fn clear_window(&mut self, window: u64) {
        unsafe {
            xlib::XClearWindow(self.ptr, window);
        }
    }

    pub fn clear_window_area(&mut self, window: u64, x: i32, y: i32, width: u32, height: u32) {
        unsafe {
            xlib::XClearArea(self.ptr, window, x, y, width, height, xlib::False);
        }
    }

    pub fn create_bar(&mut self, x: i32, width: u32) -> Result<Bar, Box<dyn std::error::Error>> {
        unsafe {
            let window = xlib::XCreateSimpleWindow(
                self.ptr,
                self.root,
                x + 10,
                10,
                width - 20,
                30,
                0,
                0x0000ff,
                0x0d1617,
            );

            let gc = xlib::XCreateGC(self.ptr, window, 0, &mut mem::zeroed());
            let draw = xft::XftDrawCreate(self.ptr, window, xlib::XDefaultVisual(self.ptr, self.screen), xlib::XDefaultColormap(self.ptr, self.screen));
            let font = self.load_font("DejaVu Sans Mono:size=11:antialias=true")?;
            let bg = self.xft_color_alloc_name("#0d1617")?;
            let fg = self.xft_color_alloc_name("#5ec587")?;


            Ok(Bar {
                window,
                gc,
                draw,
                font,
                fg,
                bg,
            })
        }
    }

    pub fn xft_free(&mut self, bar: &mut Bar) {
        unsafe {
            xft::XftColorFree(
                self.ptr,
                xlib::XDefaultVisual(self.ptr, self.screen),
                xlib::XDefaultColormap(self.ptr, self.screen),
                &mut bar.fg,
            );

            xft::XftColorFree(
                self.ptr,
                xlib::XDefaultVisual(self.ptr, self.screen),
                xlib::XDefaultColormap(self.ptr, self.screen),
                &mut bar.fg,
            );

            xft::XftFontClose(self.ptr, bar.font);
            xft::XftDrawDestroy(bar.draw);

            xlib::XFreeGC(self.ptr, bar.gc);
        }
    }

    pub fn xft_draw_string(
        &self,
        text: &str,
        x: i32,
        y: i32,
        font: *mut xft::XftFont,
        color: *const xft::XftColor,
        draw: *mut xft::XftDraw,
    ) {
        unsafe {
            xft::XftDrawStringUtf8(draw, color, font, x, y, Self::null_terminate(text).as_ptr(), text.len() as i32);
        }
    }

    pub fn xft_measure_string(&self, text: &str, font: *mut xft::XftFont) -> xrender::_XGlyphInfo {
        unsafe {
            let mut extents: xrender::_XGlyphInfo = mem::zeroed();

            xft::XftTextExtentsUtf8(self.ptr, font, Self::null_terminate(text).as_ptr(), text.len() as i32, &mut extents);

            extents
        }
    }

    pub fn xft_color_alloc_name(&mut self, rgb: &str) -> Result<xft::XftColor, Box<dyn std::error::Error>> {
        unsafe {
            let mut color: xft::XftColor = mem::zeroed();

            let result = xft::XftColorAllocName(
                self.ptr,
                xlib::XDefaultVisual(self.ptr, self.screen),
                xlib::XDefaultColormap(self.ptr, self.screen),
                Self::null_terminate(rgb).as_ptr() as *const i8,
                &mut color,
            );

            if result == 0 {
                Err("XftColorAllocName failed".into())
            } else {
                Ok(color)
            }
        }
    }

    pub fn load_font(&mut self, font: &str) -> Result<*mut xft::XftFont, Box<dyn std::error::Error>> {
        unsafe {
            let font = xft::XftFontOpenName(self.ptr, self.screen, Self::null_terminate(font).as_ptr() as *const i8);

            if font.is_null() {
                Err("XftFontOpenName failed".into())
            } else {
                Ok(font)
            }

        }
    }

    pub fn draw_rec(&mut self, x: i32, y: i32, width: u32, height: u32, color: u64, window: u64, gc: *mut xlib::_XGC) {
        unsafe {
            xlib::XSetForeground(self.ptr, gc, color);
            xlib::XFillRectangle(self.ptr, window, gc, x, y, width, height);
        }
    }

    fn null_terminate(string: &str) -> String {
        format!("{}\0", string)
    }

    pub fn property_exists(&mut self, property: &str) -> bool {
        unsafe {
            xlib::XInternAtom(self.ptr, Self::null_terminate(property).as_ptr() as *const i8, xlib::True) != 0
        }
    }

    pub fn set_property_null(&mut self, property: &str, type_: u64) -> Result<(), Box<dyn std::error::Error>> {
        unsafe {
            let p_atom = xlib::XInternAtom(self.ptr, Self::null_terminate(property).as_ptr() as *const i8, xlib::False);

            xlib::XDeleteProperty(self.ptr, self.root, p_atom);
            xlib::XChangeProperty(self.ptr, self.root, p_atom, type_, 32, xlib::PropModeReplace, (&0 as *const i32) as *const u8, 1);
        }

        Ok(())
    }

    pub fn set_property_u64(&mut self, property: &str, value: u64, type_: u64) -> Result<(), Box<dyn std::error::Error>> {
        unsafe {
            let p_atom = xlib::XInternAtom(self.ptr, Self::null_terminate(property).as_ptr() as *const i8, xlib::False);

            xlib::XDeleteProperty(self.ptr, self.root, p_atom);
            xlib::XChangeProperty(self.ptr, self.root, p_atom, type_, 32, xlib::PropModeReplace, (&value as *const u64) as *const u8, 1);
        }

        Ok(())
    }

    pub fn atom_name(&mut self, atom: u64) -> &str {
        unsafe {
            CStr::from_ptr(xlib::XGetAtomName(self.ptr, atom)).to_str().unwrap_or("no atom")
        }
    }

    pub fn intern_atom(&mut self, atom: &str) -> u64 {
        unsafe {
            xlib::XInternAtom(self.ptr, Self::null_terminate(atom).as_ptr() as *const i8, xlib::False)
        }
    }

    fn get_window_property(&mut self, window: u64, atom: u64) -> WindowProperty {
        unsafe {
            let mut a_atom: xlib::Atom = mem::zeroed();
            let mut a_format = 0;
            let mut length = 0;
            let mut after = 0;
            let mut data: *mut u8 = ptr::null_mut();

            xlib::XGetWindowProperty(
                self.ptr,
                window,
                atom,
                0,
                i64::MAX,
                xlib::False,
                xlib::XA_ATOM,
                &mut a_atom,
                &mut a_format,
                &mut length,
                &mut after,
                &mut data,
            );

            WindowProperty {
                data,
                length,
            }
        }
    }

    pub fn atom_cmp(&mut self, window: u64, property: &str, value: &str) -> bool {
        unsafe {
            let p_atom = xlib::XInternAtom(self.ptr, Self::null_terminate(property).as_ptr() as *const i8, xlib::False);
            let v_atom = xlib::XInternAtom(self.ptr, Self::null_terminate(value).as_ptr() as *const i8, xlib::True);
            let window_property = self.get_window_property(window, p_atom);

            for index in 0..window_property.length {
                let atom = *(window_property.data.offset(index as isize) as *mut xlib::Atom);

                if atom == v_atom {
                    return true;
                }
            }

            false
        }
    }

    pub fn set_wm_name(&mut self, window: u64, name: &str) {
        unsafe {
            let mut text_property: xlib::XTextProperty = mem::zeroed();
            xlib::XStringListToTextProperty([Self::null_terminate(name).as_ptr() as *const i8].as_ptr() as *mut *mut i8, 1, &mut text_property);

            xlib::XSetWMName(self.ptr, window, &mut text_property);
        }
    }

    pub fn display_width(&mut self) -> u32 {
        unsafe {
            xlib::XDisplayWidth(self.ptr, xlib::XDefaultScreen(self.ptr)) as u32
        }
    }

    pub fn display_height(&mut self) -> u32 {
        unsafe {
            xlib::XDisplayHeight(self.ptr, xlib::XDefaultScreen(self.ptr)) as u32
        }
    }

    pub fn fetch_window_name(&self, window: u64) -> Result<Option<String>, Box<dyn std::error::Error>> {
        unsafe {
            let mut w_name: *mut i8 = ptr::null_mut();

            xlib::XFetchName(self.ptr, window, &mut w_name);

            if !w_name.is_null() {
                let string = CStr::from_ptr(w_name as *const i8).to_str()?.to_string().clone();

                xlib::XFree(w_name as *mut std::ffi::c_void);

                Ok(Some(string))
            } else {
                Ok(None)
            }
        }
    }

    pub fn get_window_attributes(&mut self, window: u64) -> xlib::XWindowAttributes {
        unsafe {
            let mut attr: xlib::XWindowAttributes = mem::zeroed();

            xlib::XGetWindowAttributes(self.ptr, window, &mut attr);

            attr
        }
    }

    pub fn map_window(&mut self, window: u64) {
        unsafe {
            xlib::XMapWindow(self.ptr, window);
        }
    }

    pub fn unmap_window(&mut self, window: u64) {
        unsafe {
            xlib::XUnmapWindow(self.ptr, window);
        }
    }

    pub fn resize_window(&mut self, window: u64, x: i32, y: i32, width: u32, height: u32) {
        unsafe {
            xlib::XMoveResizeWindow(self.ptr, window, x, y, width, height);
        }
    }

    pub fn kill_window(&mut self, window: u64) {
        unsafe {
            let wm_delete_window = xlib::XInternAtom(self.ptr, Self::null_terminate("WM_DELETE_WINDOW").as_ptr() as *const i8, xlib::False);
            let wm_protocols = xlib::XInternAtom(self.ptr, Self::null_terminate("WM_PROTOCOLS").as_ptr() as *const i8, xlib::False);

            let mut protocols: *mut xlib::Atom = mem::zeroed();
            let mut supported = false;
            let mut count = 0;

            xlib::XGetWMProtocols(self.ptr, window, &mut protocols, &mut count);

            for index in 0..count {
                if (*protocols.offset(index as isize)) == wm_delete_window {
                    supported = true;
                }
            }

            if supported {
                let mut event: xlib::XEvent = mem::zeroed();

                event.client_message.type_ = xlib::ClientMessage;
                event.client_message.message_type = wm_protocols;
                event.client_message.window = window;
                event.client_message.format = 32;
                event.client_message.data.set_long(0, wm_delete_window as i64);

                xlib::XSendEvent(self.ptr, window, xlib::False, 0, &mut event);
            } else {
                xlib::XKillClient(self.ptr, window);
            }
        }
    }

    pub fn keycode_to_keysym(&mut self, keysym: u32) -> u64 {
        unsafe {
            xlib::XKeycodeToKeysym(self.ptr, keysym as u8, 0)
        }
    }

    pub fn string_to_keysym(string: &str) -> u64 {
        unsafe {
            xlib::XStringToKeysym(Self::null_terminate(string).as_ptr() as *const i8)
        }
    }

    pub fn grab_key(&mut self, keysym: u32, mask: u32, window: u64) {
        unsafe {
            xlib::XGrabKey(
                self.ptr,
                xlib::XKeysymToKeycode(self.ptr, keysym.into()).into(),
                mask,
                window,
                xlib::True,
                xlib::GrabModeAsync,
                xlib::GrabModeAsync,
            );
        }
    }

    pub fn grab_button(&mut self, button: u32, window: u64) {
        unsafe {
            xlib::XGrabButton(
                self.ptr,
                button,
                xlib::Mod4Mask,
                window,
                xlib::True,
                (xlib::ButtonPressMask | xlib::ButtonReleaseMask | xlib::PointerMotionMask) as u32,
                xlib::GrabModeAsync,
                xlib::GrabModeAsync,
                0,
                0
            );
        }
    }

    pub fn grab_pointer(&mut self, window: u64) {
        unsafe {
            xlib::XGrabPointer(
                self.ptr,
                window,
                xlib::True,
                (xlib::PointerMotionMask | xlib::ButtonReleaseMask) as u32,
                xlib::GrabModeAsync,
                xlib::GrabModeAsync,
                0,
                0,
                xlib::CurrentTime
            );
        }
    }

    pub fn ungrab_pointer(&mut self) {
        unsafe {
            xlib::XUngrabPointer(self.ptr, xlib::CurrentTime);
        }
    }

    pub fn select_input(&mut self, window: u64) {
        unsafe {
            xlib::XSelectInput(self.ptr, window, xlib::SubstructureNotifyMask | xlib::SubstructureRedirectMask | xlib::EnterWindowMask);
        }
    }

    pub fn set_input_focus(&mut self, window: u64) {
        unsafe {
            xlib::XSetInputFocus(self.ptr, window, xlib::RevertToParent, xlib::CurrentTime);
        }
    }

    pub fn set_focus_icccm(&mut self, window: u64) {
        unsafe {
            let mut event: xlib::XEvent = mem::zeroed();

            let wm_protocols = xlib::XInternAtom(self.ptr, Self::null_terminate("WM_PROTOCOLS").as_ptr() as *const i8, xlib::False);
            let wm_take_focus = xlib::XInternAtom(self.ptr, Self::null_terminate("WM_TAKE_FOCUS").as_ptr() as *const i8, xlib::False);

            // TODO: use client_message here?
            event.type_ = xlib::ClientMessage;
            event.client_message.window = window;
            event.client_message.message_type = wm_protocols;
            event.client_message.format = 32;
            event.client_message.data.set_long(0, wm_take_focus as i64);
            event.client_message.data.set_long(1, xlib::CurrentTime as i64);

            xlib::XSendEvent(
                self.ptr,
                window,
                xlib::False,
                xlib::NoEventMask,
                &mut event,
            );
        }
    }

    pub fn raise_window(&mut self, window: u64) {
        unsafe {
            xlib::XRaiseWindow(self.ptr, window);
        }
    }

    pub fn next_event(&mut self) -> xlib::XEvent {
        unsafe {
            let mut event: xlib::XEvent = mem::zeroed();
            xlib::XNextEvent(self.ptr, &mut event);

            event
        }
    }
}


