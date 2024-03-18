mod config;
mod xlib;
mod wm;

use config::Config;
use wm::WindowManager;
use wm::ExitCode;

use std::process::Command;
use std::process::Stdio;
use std::process;
use std::env;


fn main() {
    let mut wm = match WindowManager::new() {
        Ok(wm) => wm,
        Err(err) => {
            println!("[ERROR] failed to open wm: {}", err.to_string());
            process::exit(1);
        },
    };

    match wm.run() {
        Ok(exit) => {
            match exit {
                ExitCode::Restart => {
                    let result = Command::new(env::current_exe().unwrap_or_default().to_str().unwrap_or_default())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .stdin(Stdio::null())
                        .spawn();

                    match result {
                        Ok(_) => {
                            println!("[+] restarted successfully");
                        },
                        Err(err) => {
                            println!("[+] failed to restart: {}", err);
                        },
                    }
                },
            }
        },
        Err(err) => {
            println!("[ERROR] failed to run wm: {}", err.to_string());
            process::exit(1);
        },
    }
}
