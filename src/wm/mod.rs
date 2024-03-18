use crate::Config;
use crate::xlib;

use std::process::Command;
use std::process::Stdio;
use std::ptr;
use std::env;

pub enum ExitCode {
    Restart,
}

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
    pub workspace: usize,
    pub bar: Bar,
}

pub struct WindowManager {
    pub display: xlib::Display,
    config: Config,
    monitors: Vec<Monitor>,
    window: u64,
    ignore: Vec<&'static str>,
}

impl WindowManager {
    pub fn new() -> Result<WindowManager, Box<dyn std::error::Error>> {
        let mut display = xlib::Display::open(ptr::null())?;
        let window = display.root;
        let monitors = display.get_monitors()?;

        Ok(WindowManager {
            display,
            config: Config::load()?,
            monitors,
            window,
            ignore: vec![
                "rmenu",
            ],
        })
    }

    fn setup(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let keys = [
            x11::keysym::XK_Return,
            x11::keysym::XK_q,
            x11::keysym::XK_d,
            x11::keysym::XK_m,
            x11::keysym::XK_1,
            x11::keysym::XK_2,
            x11::keysym::XK_3,
            x11::keysym::XK_4,
            x11::keysym::XK_Left,
            x11::keysym::XK_Up,
            x11::keysym::XK_Down,
        ];

        for key in keys {
            self.display.grab_key(key, xlib::Mod4Mask, self.display.root);
        }

        self.display.select_input(self.display.root);
        self.display.set_wm_name(self.display.root, "ZovaWM");

        self.display.set_property_u64("_NET_NUMBER_OF_DESKTOPS", self.monitors[0].clients.len() as u64, xlib::XA_CARDINAL)?;

        Ok(())
    }

    fn execv(&self, program: &str, args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
        Command::new(program)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .spawn()?;

        Ok(())
    }

    fn cleanup_bar(&mut self) {
        for monitor in &mut self.monitors {
            self.display.xft_free(&mut monitor.bar);
        }
    }

    fn draw_bar(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        for monitor in &self.monitors {
            self.display.map_window(monitor.bar.window);
            self.display.clear_window(monitor.bar.window);

            for workspace in 0..monitor.clients.len() {
                if workspace == monitor.workspace {
                    self.display.draw_rec((workspace as i32 * 25) + 5, 5, 20, 20, 0x5ec587, monitor.bar.window, monitor.bar.gc);

                    self.display.xft_draw_string(&format!("{}", workspace + 1), (workspace as i32 * 25) + 11, 20, monitor.bar.font, &monitor.bar.bg, monitor.bar.draw);
                } else {
                    self.display.xft_draw_string(&format!("{}", workspace + 1), (workspace as i32 * 25) + 11, 20, monitor.bar.font, &monitor.bar.fg, monitor.bar.draw);
                }
            }

            self.display.xft_draw_string(
                "ZovaWM",
                (self.monitors[0].width as i32 / 2) - (self.display.xft_measure_string("ZovaWM", monitor.bar.font).width as i32 / 2),
                20,
                monitor.bar.font,
                &monitor.bar.fg,
                monitor.bar.draw
            );
        }

        Ok(())
    }

    fn tile_clients(&mut self) {
        for monitor in &self.monitors {
            if monitor.clients[monitor.workspace].len() == 1 {
                self.display.resize_window(
                    monitor.clients[monitor.workspace][0].window,
                    monitor.x + self.config.padding.right,
                    self.config.padding.top,
                    monitor.width - self.config.padding.right as u32 - self.config.padding.left as u32,
                    monitor.height - self.config.padding.bottom as u32 - self.config.padding.top as u32
                );
            } else if !monitor.clients[monitor.workspace].is_empty() {
                self.display.resize_window(
                    monitor.clients[monitor.workspace][0].window,
                    monitor.x + self.config.padding.right,
                    self.config.padding.top,
                    (monitor.width / 2) - self.config.padding.right as u32 - 5,
                    monitor.height - self.config.padding.bottom as u32 - self.config.padding.top as u32
                );

                for (index, client) in monitor.clients[monitor.workspace][1..].iter().enumerate() {
                    self.display.resize_window(
                        client.window,
                        monitor.x + (monitor.width as i32 / 2) + 5,
                        (
                            (monitor.height as i32 - self.config.padding.top - self.config.padding.bottom + 10)
                                / (monitor.clients[monitor.workspace].len() as i32 - 1)
                        ) * index as i32 + self.config.padding.top,
                        (monitor.width as u32 / 2) - self.config.padding.left as u32 - 5,
                        (monitor.height as u32 - self.config.padding.top as u32 - self.config.padding.bottom as u32 + 10)
                            / (monitor.clients[monitor.workspace].len() as u32 - 1) - 10,
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

    fn goto_workspace(&mut self, workspace: usize) -> Result<(), Box<dyn std::error::Error>> {
        let monitor = self.current_monitor();

        if workspace < self.monitors[monitor].clients.len() {
            self.monitors[monitor].workspace = workspace;
            self.display.set_property_u64("_NET_CURRENT_DESKTOP", self.monitors[monitor].workspace as u64, xlib::XA_CARDINAL)?;

            for client in &self.monitors[monitor].clients[self.monitors[monitor].workspace] {
                self.display.map_window(client.window);
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

    fn is_ignored(&mut self, window: u64) -> Result<bool, Box<dyn std::error::Error>> {
        if let Some(name) = self.display.fetch_window_name(window)? {
            Ok(self.ignore.contains(&name.as_str()))
        } else {
            Ok(false)
        }
    }

    fn window_to_client_index(&mut self, window: u64) -> Option<usize> {
        let monitor = self.current_monitor();

        for (index, client) in self.monitors[0].clients[self.monitors[monitor].workspace].iter().enumerate() {
            if client.window == window {
                return Some(index);
            }
        }

        None
    }

    fn move_client(&mut self, old_index: usize, new_index: usize) {
        let monitor = self.current_monitor();
        let workspace = self.monitors[monitor].workspace;

        let client = self.monitors[monitor].clients[workspace].remove(old_index);
        self.monitors[monitor].clients[workspace].insert(new_index, client);
    }

    pub fn run(&mut self) -> Result<ExitCode, Box<dyn std::error::Error>> {
        self.setup()?;

        let home = env::var("HOME")?;
        self.execv("sh", &[&format!("{}/.config/zovawm/startup.sh", home)])?;

        loop {
            self.draw_bar()?;

            let event = self.display.next_event();

            match unsafe { event.type_ } {
                x11::xlib::KeyPress => {
                    let keycode = unsafe { event.key.keycode };

                    match self.display.keycode_to_keysym(keycode) as u32 {
                        x11::keysym::XK_Return => {
                            self.execv("kitty", &[])?;
                        },
                        x11::keysym::XK_d => {
                            self.execv("rmenu", &[])?;
                        },
                        x11::keysym::XK_q => {
                            println!("killing {}", unsafe { event.key.subwindow });

                            self.display.kill_window(unsafe { event.key.subwindow });
                        },
                        x11::keysym::XK_m => {
                            self.cleanup_bar();

                            return Ok(ExitCode::Restart);
                        },
                        x11::keysym::XK_Up => {
                            if let Some(index) = self.window_to_client_index(self.window) {
                                if index > 0 {
                                    self.move_client(index, index - 1);

                                    self.tile_clients();
                                }
                            }
                        },
                        x11::keysym::XK_Down => {
                            if let Some(index) = self.window_to_client_index(self.window) {
                                let monitor = self.current_monitor();

                                if index < self.monitors[monitor].clients[self.monitors[monitor].workspace].len() - 1 {
                                    self.move_client(index, index + 1);

                                    self.tile_clients();
                                }
                            }
                        },
                        x11::keysym::XK_Left => {
                            if let Some(index) = self.window_to_client_index(self.window) {
                                self.move_client(index, 0);

                                self.tile_clients();
                            }
                        },
                        /*
                         * 49 is keysym for 0
                        */
                        keysym => self.goto_workspace(keysym as usize - 49)?,
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

                    self.tile_clients();
                },
                x11::xlib::MapRequest => {
                    let window = unsafe { event.map.window };
                    let ignored = self.is_ignored(window)?;
                    let monitor = self.current_monitor();
                    let workspace = self.monitors[monitor].workspace;

                    if !self.monitors[monitor].clients[workspace].contains(&Client::new(window, ignored)) && !ignored {
                        self.monitors[monitor].clients[workspace].push(Client::new(window, ignored));
                    }

                    self.display.map_window(window);
                    self.display.select_input(window);
                    self.display.set_wm_name(window, "ZovaWM");

                    self.window = window;

                    self.tile_clients();
                },
                x11::xlib::EnterNotify => {
                    let window = unsafe { event.crossing.window };

                    self.display.raise_window(window);
                    self.display.set_input_focus(window);
                    self.display.set_focus_icccm(window);
                    self.display.set_property_u64("_NET_ACTIVE_WINDOW", window, xlib::XA_WINDOW)?;

                    self.window = window;
                },
                _ => {},
            }
        }
    }
}


