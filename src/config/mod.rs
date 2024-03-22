use crate::xlib;

use toml::Table;

use std::collections::HashMap;
use std::env;
use std::fs;

pub struct Padding {
    pub top: i32,
    pub bottom: i32,
    pub left: i32,
    pub right: i32,
}

pub enum Internal {
    Fullscreen,
    Kill,
    Restart,
    FocusUp,
    FocusDown,
    FocusMaster,
    WindowUp,
    WindowDown,
    WindowMaster,
    ToggleFloat,
}

pub enum Action {
    Exec(String),
    Internal(Internal),
}

pub struct Config {
    pub bar: bool,
    pub padding: Padding,
    pub keybindings: HashMap<u32, Action>,
}

impl Config {
    pub fn load() -> Result<Config, Box<dyn std::error::Error>> {
        let home = env::var("HOME")?;

        if let Ok(content) = fs::read_to_string(format!("{}/.config/zovawm/config.toml", home)) {
            let config = content.parse::<Table>()?;

            Ok(Config {
                bar: Self::get_bool(&config, "default-bar", true),
                padding: Padding {
                    top:    Self::get_int(&config, "top-padding",       50) as i32,
                    bottom: Self::get_int(&config, "bottom-padding",    10) as i32,
                    left:   Self::get_int(&config, "left-padding",      10) as i32,
                    right:  Self::get_int(&config, "right-padding",     10) as i32,
                },
                keybindings: Self::get_keybindings(&config)?,
            })
        } else {
            let mut keybindings: HashMap<u32, Action> = HashMap::new();

            keybindings.insert(xlib::Display::string_to_keysym("Return") as u32, Action::Exec(String::from("kitty")));
            keybindings.insert(xlib::Display::string_to_keysym("d") as u32, Action::Exec(String::from("rmenu")));

            Ok(Config {
                bar: true,
                padding: Padding {
                    top:    50,
                    bottom: 10,
                    left:   10,
                    right:  10,
                },
                keybindings,
            })
        }
    }

    pub fn get_keybindings(config: &toml::map::Map<String, toml::Value>) -> Result<HashMap<u32, Action>, Box<dyn std::error::Error>> {
        let mut keybindings: HashMap<u32, Action> = HashMap::new();

        if let Some(keybindings_value) = config.get("keybindings") {
            for keybinding in keybindings_value.as_array().unwrap_or(&Vec::new()) {
                if let Some(table) = keybinding.as_table() {
                    let key = xlib::Display::string_to_keysym(table.get("key").map_or("none", |x| x.as_str().unwrap_or_default())) as u32;

                    if let Some(exec) = table.get("exec") {
                        keybindings.insert(key, Action::Exec(exec.as_str().unwrap_or_default().to_string()));
                    } else if let Some(internal) = table.get("internal") {
                        match internal.as_str().unwrap_or_default() {
                            "fullscreen" => { keybindings.insert(key, Action::Internal(Internal::Fullscreen)); },
                            "kill" => { keybindings.insert(key, Action::Internal(Internal::Kill)); },
                            "restart" => { keybindings.insert(key, Action::Internal(Internal::Restart)); },
                            "focus_up" => { keybindings.insert(key, Action::Internal(Internal::FocusUp)); },
                            "focus_down" => { keybindings.insert(key, Action::Internal(Internal::FocusDown)); },
                            "focus_master" => { keybindings.insert(key, Action::Internal(Internal::FocusMaster)); },
                            "window_up" => { keybindings.insert(key, Action::Internal(Internal::WindowUp)); },
                            "window_down" => { keybindings.insert(key, Action::Internal(Internal::WindowDown)); },
                            "window_master" => { keybindings.insert(key, Action::Internal(Internal::WindowMaster)); },
                            "toggle_float" => { keybindings.insert(key, Action::Internal(Internal::ToggleFloat)); },
                            internal => println!("[+] unknown internal: {}", internal),
                        }
                    } else {
                        println!("[+] ignoring keybinding: no exec or internal set");
                    }
                }
            }
        }

        Ok(keybindings)
    }

    pub fn get_int(config: &toml::map::Map<String, toml::Value>, key: &str, default: usize) -> usize {
        config.get(key).map_or(default, |x| x.as_integer().unwrap_or_default() as usize)
    }

    pub fn get_bool(config: &toml::map::Map<String, toml::Value>, key: &str, default: bool) -> bool {
        config.get(key).map_or(default, |x| x.as_bool().unwrap_or_default())
    }
}

