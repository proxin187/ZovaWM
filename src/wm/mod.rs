use crate::config::{Action, Internal};
use crate::Config;
use crate::xlib;

use nix::sys::signal;
use nix::unistd;

use std::ffi::{CStr, CString};
use std::process::Command;
use std::ptr;
use std::env;


#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Client {
    window: u64,
    tiled: bool,
}

impl Client {
    pub fn new(window: u64, tiled: bool) -> Client {
        Client {
            window,
            tiled
        }
    }
}

#[derive(Debug)]
pub struct Bar {
    pub window: u64,
    pub gc: *mut x11::xlib::_XGC,
    pub draw: *mut x11::xft::XftDraw,
    pub font: *mut x11::xft::XftFont,
    pub fg: x11::xft::XftColor,
    pub bg: x11::xft::XftColor,
}

#[derive(Debug)]
pub struct Monitor {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub clients: [Vec<Client>; 4],
    pub fullscreen: Option<Client>,
    pub workspace: usize,
    pub bar: Option<Bar>,
}

pub struct FloatClient {
    start: Option<x11::xlib::XButtonEvent>,
    attr: Option<x11::xlib::XWindowAttributes>,
}

pub struct WindowManager {
    pub display: xlib::Display,
    config: Config,
    monitors: Vec<Monitor>,
    float_client: FloatClient,
    window: u64,
}

impl WindowManager {
    pub fn new() -> Result<WindowManager, Box<dyn std::error::Error>> {
        let mut display = xlib::Display::open(ptr::null())?;
        let config = Config::load()?;
        let window = display.root;
        let monitors = display.get_monitors(config.bar, &Vec::new())?;

        display.set_net_supported(display.root);
        display.set_desktop_viewport(display.root);

        Ok(WindowManager {
            display,
            config,
            monitors,
            float_client: FloatClient {
                start: None,
                attr: None,
            },
            window,
        })
    }

    fn setup(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let sa = signal::SigAction::new(
            signal::SigHandler::SigIgn,
            signal::SaFlags::SA_NOCLDSTOP | signal::SaFlags::SA_NOCLDWAIT | signal::SaFlags::SA_RESTART,
            signal::SigSet::empty()
        );

        unsafe {
            signal::sigaction(signal::Signal::SIGCHLD, &sa)?;
        }

        let keys = [
            x11::keysym::XK_1,
            x11::keysym::XK_2,
            x11::keysym::XK_3,
            x11::keysym::XK_4,
        ];

        for key in keys {
            self.display.grab_key(key, xlib::Mod4Mask, self.display.root);
        }

        for (key, _) in &self.config.keybindings {
            self.display.grab_key(*key, xlib::Mod4Mask, self.display.root);
        }

        self.display.grab_button(xlib::Button1, self.display.root);
        self.display.grab_button(xlib::Button3, self.display.root);

        self.display.select_input(self.display.root);
        self.display.set_wm_name(self.display.root, "ZovaWM");

        self.display.set_property_u64("_NET_NUMBER_OF_DESKTOPS", self.monitors[0].clients.len() as u64, xlib::XA_CARDINAL)?;

        Ok(())
    }

    fn execv(&self, program: &str, args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
        match unsafe { unistd::fork() } {
            Ok(unistd::ForkResult::Parent { child, .. }) => {
                println!("[+] child pid: {}", child);

                unsafe {
                    signal::signal(signal::SIGCHLD, signal::SigHandler::SigIgn)?;
                }
            },
            Ok(unistd::ForkResult::Child) => {
                unistd::setsid()?;

                let args = args.iter()
                    .map(|x| CString::new(*x).unwrap())
                    .collect::<Vec<CString>>();

                let result = unistd::execvp(
                    CString::new(program)?.as_c_str(),
                    args.iter()
                        .map(|x| x.as_c_str())
                        .collect::<Vec<&CStr>>()
                        .as_slice()
                );

                println!("[+] execvp failed: {:?}", result);
            },
            Err(_) => println!("failed to fork"),
        }

        Ok(())
    }

    fn cleanup_bar(&mut self) {
        for monitor in &mut self.monitors {
            if let Some(bar) = &mut monitor.bar {
                self.display.xft_free(bar);
            }
        }
    }

    fn draw_bar(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        for monitor in &self.monitors {
            if let Some(bar) = &monitor.bar {
                self.display.map_window(bar.window);
                self.display.clear_window(bar.window);

                for workspace in 0..monitor.clients.len() {
                    if workspace == monitor.workspace {
                        self.display.draw_rec((workspace as i32 * 25) + 5, 5, 20, 20, 0x5ec587, bar.window, bar.gc);

                        self.display.xft_draw_string(&format!("{}", workspace + 1), (workspace as i32 * 25) + 11, 20, bar.font, &bar.bg, bar.draw);
                    } else {
                        self.display.xft_draw_string(&format!("{}", workspace + 1), (workspace as i32 * 25) + 11, 20, bar.font, &bar.fg, bar.draw);
                    }
                }

                self.display.xft_draw_string(
                    "ZovaWM",
                    (self.monitors[0].width as i32 / 2) - (self.display.xft_measure_string("ZovaWM", bar.font).width as i32 / 2),
                    20,
                    bar.font,
                    &bar.fg,
                    bar.draw
                );
            }
        }

        Ok(())
    }

    fn tile_clients(&mut self) {
        for monitor in &self.monitors {
            let clients = monitor.clients[monitor.workspace].iter()
                .filter(|x| x.tiled)
                .collect::<Vec<&Client>>();

            if let Some(client) = monitor.fullscreen {
                self.display.resize_window(
                    client.window,
                    monitor.x,
                    monitor.y,
                    monitor.width,
                    monitor.height
                );
            } else if clients.len() == 1 {
                self.display.resize_window(
                    clients[0].window,
                    monitor.x + self.config.padding.right,
                    self.config.padding.top,
                    monitor.width - self.config.padding.right as u32 - self.config.padding.left as u32,
                    monitor.height - self.config.padding.bottom as u32 - self.config.padding.top as u32
                );
            } else if !clients.is_empty() {
                self.display.resize_window(
                    clients[0].window,
                    monitor.x + self.config.padding.right,
                    self.config.padding.top,
                    (monitor.width / 2) - self.config.padding.right as u32 - 5,
                    monitor.height - self.config.padding.bottom as u32 - self.config.padding.top as u32
                );

                for (index, client) in clients[1..].iter().enumerate() {
                    self.display.resize_window(
                        client.window,
                        monitor.x + (monitor.width as i32 / 2) + 5,
                        (
                            (monitor.height as i32 - self.config.padding.top - self.config.padding.bottom + 10)
                                / (clients.len() as i32 - 1)
                        ) * index as i32 + self.config.padding.top,
                        (monitor.width as u32 / 2) - self.config.padding.left as u32 - 5,
                        (monitor.height as u32 - self.config.padding.top as u32 - self.config.padding.bottom as u32 + 10)
                            / (clients.len() as u32 - 1) - 10,
                    );
                }
            }
        }
    }

    fn current_monitor(&mut self) -> usize {
        let pointer = self.display.query_pointer();

        for (index, monitor) in self.monitors.iter().enumerate() {
            if (monitor.x..monitor.x + monitor.width as i32).contains(&pointer.x) {
                return index;
            }
        }

        0
    }

    fn window_to_client_index(&mut self, window: u64) -> Option<usize> {
        let monitor = self.current_monitor();

        for (index, client) in self.monitors[monitor].clients[self.monitors[monitor].workspace].iter().enumerate() {
            if client.window == window {
                return Some(index);
            }
        }

        None
    }

    fn is_tiled(&mut self, window: u64, monitor: usize, workspace: usize) -> bool {
        !self.monitors[monitor].clients[workspace].iter()
            .filter(|c| c.tiled && c.window == window)
            .collect::<Vec<&Client>>()
            .is_empty()
    }

    fn goto_workspace(&mut self, workspace: usize) -> Result<(), Box<dyn std::error::Error>> {
        let monitor = self.current_monitor();

        if workspace < self.monitors[monitor].clients.len() {
            self.monitors[monitor].workspace = workspace;
            self.display.set_property_u64("_NET_CURRENT_DESKTOP", self.monitors[monitor].workspace as u64, xlib::XA_CARDINAL)?;

            for client in self.monitors[monitor].clients[self.monitors[monitor].workspace].clone() {
                self.display.map_window(client.window);

                if !client.tiled {
                    self.display.raise_window(client.window);
                }
            }

            for (workspace, clients) in self.monitors[monitor].clients.iter().enumerate() {
                if workspace != self.monitors[monitor].workspace {
                    for client in clients {
                        self.display.unmap_window(client.window);
                    }
                }
            }

            self.tile_clients();
        }

        Ok(())
    }

    fn toggle_fullscreen(&mut self, window: u64) -> Result<(), Box<dyn std::error::Error>> {
        let monitor = self.current_monitor();
        let state_fullscreen = self.display.intern_atom("_NET_WM_STATE_FULLSCREEN");

        if self.monitors[monitor].fullscreen.is_none() {
            if let Some(client) = self.window_to_client_index(window) {
                self.display.set_property_u64("_NET_WM_STATE", state_fullscreen, xlib::XA_ATOM)?;

                self.monitors[monitor].fullscreen = Some(self.monitors[monitor].clients[self.monitors[monitor].workspace][client]);
            }
        } else {
            self.display.set_property_null("_NET_WM_STATE", xlib::XA_ATOM)?;

            self.monitors[monitor].fullscreen = None;
        }

        self.tile_clients();

        Ok(())
    }

    fn move_client(&mut self, old_index: usize, new_index: usize) {
        let monitor = self.current_monitor();
        let workspace = self.monitors[monitor].workspace;

        let client = self.monitors[monitor].clients[workspace].remove(old_index);
        self.monitors[monitor].clients[workspace].insert(new_index, client);
    }

    fn change_focus(&mut self, window: u64) -> Result<(), Box<dyn std::error::Error>> {
        // self.display.raise_window(window);
        self.display.set_input_focus(window);
        self.display.set_focus_icccm(window);
        self.display.set_property_u64("_NET_ACTIVE_WINDOW", window, xlib::XA_WINDOW)?;

        self.window = window;

        Ok(())
    }

    fn change_focus_index(&mut self, index: usize) -> Result<(), Box<dyn std::error::Error>> {
        let monitor = self.current_monitor();
        let workspace = self.monitors[monitor].workspace;
        let window = self.monitors[monitor].clients[workspace][index].window;

        self.change_focus(window)?;

        Ok(())
    }

    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.setup()?;

        let home = env::var("HOME")?;
        Command::new("sh")
            .args(&[&format!("{}/.config/zovawm/startup.sh", home)])
            .spawn()?;

        loop {
            self.draw_bar()?;

            let event = self.display.next_event();

            match unsafe { event.type_ } {
                x11::xlib::KeyPress => {
                    let keycode = unsafe { event.key.keycode };
                    let keysym = self.display.keycode_to_keysym(keycode) as u32;

                    if let Some(action) = self.config.keybindings.get(&keysym) {
                        match action {
                            Action::Exec(path) => {
                                self.execv(&path, &[])?;
                            },
                            Action::Internal(internal) => {
                                match internal {
                                    Internal::Fullscreen => {
                                        self.toggle_fullscreen(self.window)?;
                                    },
                                    Internal::Kill => {
                                        self.display.kill_window(unsafe { event.key.subwindow });
                                    },
                                    Internal::Exit => {
                                        self.cleanup_bar();

                                        return Ok(());
                                    },
                                    Internal::Restart => {
                                        self.cleanup_bar();

                                        self.config = Config::load()?;
                                        self.monitors = self.display.get_monitors(self.config.bar, &self.monitors)?;

                                        self.setup()?;

                                        /*
                                         * this may cause zombie processes to pile up when restarting
                                        */
                                        let home = env::var("HOME")?;
                                        Command::new("sh")
                                            .args(&[&format!("{}/.config/zovawm/startup.sh", home)])
                                            .spawn()?;
                                    },
                                    Internal::FocusUp => {
                                        if let Some(index) = self.window_to_client_index(self.window) {
                                            if index > 0 {
                                                self.change_focus_index(index - 1)?;
                                            }
                                        }
                                    },
                                    Internal::FocusDown => {
                                        if let Some(index) = self.window_to_client_index(self.window) {
                                            let monitor = self.current_monitor();

                                            if index < self.monitors[monitor].clients[self.monitors[monitor].workspace].len() - 1 {
                                                self.change_focus_index(index + 1)?;
                                            }
                                        }
                                    },
                                    Internal::FocusMaster => {
                                        self.change_focus_index(0)?;
                                    },
                                    Internal::WindowUp => {
                                        if let Some(index) = self.window_to_client_index(self.window) {
                                            if index > 0 {
                                                self.move_client(index, index - 1);

                                                self.tile_clients();
                                            }
                                        }
                                    },
                                    Internal::WindowDown => {
                                        if let Some(index) = self.window_to_client_index(self.window) {
                                            let monitor = self.current_monitor();

                                            if index < self.monitors[monitor].clients[self.monitors[monitor].workspace].len() - 1 {
                                                self.move_client(index, index + 1);

                                                self.tile_clients();
                                            }
                                        }
                                    },
                                    Internal::WindowMaster => {
                                        if let Some(index) = self.window_to_client_index(self.window) {
                                            self.move_client(index, 0);

                                            self.tile_clients();
                                        }
                                    },
                                    Internal::ToggleFloat => {
                                        let window = unsafe { event.key.subwindow };
                                        let monitor = self.current_monitor();
                                        let workspace = self.monitors[monitor].workspace;

                                        if self.is_tiled(window, monitor, workspace) {
                                            self.monitors[monitor].clients[workspace] = self.monitors[monitor].clients[workspace].iter()
                                                .map(|c| {
                                                    if c.window == window {
                                                        Client::new(c.window, false)
                                                    } else {
                                                        Client::new(c.window, c.tiled)
                                                    }
                                                })
                                                .collect::<Vec<Client>>();
                                        } else {
                                            self.monitors[monitor].clients[workspace] = self.monitors[monitor].clients[workspace].iter()
                                                .map(|c| {
                                                    if c.window == window {
                                                        Client::new(c.window, true)
                                                    } else {
                                                        Client::new(c.window, c.tiled)
                                                    }
                                                })
                                                .collect::<Vec<Client>>();
                                        }

                                        self.tile_clients();
                                    },
                                }
                            },
                        }
                    }

                    if (49..49 + 4).contains(&keysym) {
                        self.goto_workspace(keysym as usize - 49)?;
                    }
                },
                x11::xlib::UnmapNotify => {
                    let window = unsafe { event.unmap.window };
                    let monitor = self.current_monitor();
                    let workspace = self.monitors[monitor].workspace;

                    /* Something wrong with this lol
                    for monitor in &mut self.monitors {
                        for workspace in 0..monitor.clients.len() {
                            monitor.clients[workspace] = monitor.clients[workspace].iter()
                                .filter(|c| c.window != window)
                                .map(|c| *c)
                                .collect::<Vec<Client>>();
                        }
                    }
                    */

                    self.monitors[monitor].clients[workspace] = self.monitors[monitor].clients[self.monitors[monitor].workspace].iter()
                        .filter(|c| c.window != window)
                        .map(|c| *c)
                        .collect::<Vec<Client>>();

                    if let Some(client) = self.monitors[monitor].fullscreen {
                        if client.window == window {
                            self.monitors[monitor].fullscreen = None;
                        }
                    }

                    self.tile_clients();
                },
                x11::xlib::MapRequest => {
                    let window = unsafe { event.map.window };
                    let monitor = self.current_monitor();
                    let workspace = self.monitors[monitor].workspace;
                    let ignored = self.display.atom_cmp(window, "_NET_WM_WINDOW_TYPE", "_NET_WM_WINDOW_TYPE_DOCK")
                        || self.display.atom_cmp(window, "_NET_WM_WINDOW_TYPE", "_NET_WM_WINDOW_TYPE_DIALOG")
                        || self.display.atom_cmp(window, "_NET_WM_WINDOW_TYPE", "_NET_WM_WINDOW_TYPE_UTILITY")
                        || self.display.atom_cmp(window, "_NET_WM_WINDOW_TYPE", "_NET_WM_WINDOW_TYPE_SPLASH");

                    if !self.monitors[monitor].clients[workspace].contains(&Client::new(window, ignored)) && !ignored {
                        self.monitors[monitor].clients[workspace].push(Client::new(window, true));
                    } else if !self.display.atom_cmp(window, "_NET_WM_WINDOW_TYPE", "_NET_WM_WINDOW_TYPE_DOCK") {
                        self.monitors[monitor].clients[workspace].push(Client::new(window, false));
                    }

                    self.display.map_window(window);
                    self.display.select_input(window);
                    self.display.set_wm_name(window, "ZovaWM");

                    self.window = window;

                    self.tile_clients();
                },
                x11::xlib::EnterNotify => {
                    let window = unsafe { event.crossing.window };

                    self.change_focus(window)?;
                },
                x11::xlib::ClientMessage => {
                    let message_type = unsafe { event.client_message.message_type };
                    let message_data = unsafe { event.client_message.data };
                    let message_window = unsafe { event.client_message.window };

                    let state_fullscreen = self.display.intern_atom("_NET_WM_STATE_FULLSCREEN") as i64;

                    if message_type == self.display.intern_atom("_NET_WM_STATE") {
                        if message_data.get_long(1) == state_fullscreen || message_data.get_long(2) == state_fullscreen {
                            self.toggle_fullscreen(message_window)?;
                        }
                    }
                },
                x11::xlib::ButtonPress => {
                    let window = unsafe { event.key.subwindow };
                    let monitor = self.current_monitor();
                    let workspace = self.monitors[monitor].workspace;

                    if !self.is_tiled(window, monitor, workspace) {
                        self.display.grab_pointer(window);

                        self.float_client.start = Some(unsafe { event.button });
                        self.float_client.attr = Some(self.display.get_window_attributes(unsafe { event.button.subwindow }));
                    }
                },
                x11::xlib::ButtonRelease => {
                    if self.float_client.start.is_some() {
                        self.display.ungrab_pointer();

                        self.float_client.start = None;
                        self.float_client.attr = None;
                    }
                },
                x11::xlib::MotionNotify => {
                    if let Some(start) = &self.float_client.start {
                        if let Some(attr) = self.float_client.attr {
                            let x_diff = unsafe { event.button.x_root } - start.x_root;
                            let y_diff = unsafe { event.button.y_root } - start.y_root;
                            let move_ = start.button == xlib::Button1;

                            self.display.resize_window(
                                start.subwindow,
                                attr.x + move_.then(|| x_diff).unwrap_or(0),
                                attr.y + move_.then(|| y_diff).unwrap_or(0),
                                (attr.width + (!move_).then(|| x_diff).unwrap_or(0)) as u32,
                                (attr.height + (!move_).then(|| y_diff).unwrap_or(0)) as u32,
                            );
                        }
                    }
                },
                _ => {},
            }
        }
    }
}


