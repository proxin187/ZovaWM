mod config;
mod xlib;
mod wm;

use config::Config;
use wm::WindowManager;

use std::process;


fn main() {
    let mut wm = match WindowManager::new() {
        Ok(wm) => wm,
        Err(err) => {
            println!("[ERROR] failed to open wm: {}", err.to_string());
            process::exit(1);
        },
    };

    match wm.run() {
        Ok(_) => {},
        Err(err) => {
            println!("[ERROR] failed to run wm: {}", err.to_string());
            process::exit(1);
        },
    }
}
