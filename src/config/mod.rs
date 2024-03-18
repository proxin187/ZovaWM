use toml::Table;

use std::env;
use std::fs;

pub struct Padding {
    pub top: i32,
    pub bottom: i32,
    pub left: i32,
    pub right: i32,
}

pub struct Config {
    pub bar: bool,
    pub padding: Padding,
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
            })
        } else {
            Ok(Config {
                bar: true,
                padding: Padding {
                    top:    50,
                    bottom: 10,
                    left:   10,
                    right:  10,
                },
            })
        }
    }

    pub fn get_int(config: &toml::map::Map<String, toml::Value>, key: &str, default: usize) -> usize {
        config.get(key).map_or(default, |x| x.as_integer().unwrap_or_default() as usize)
    }

    pub fn get_bool(config: &toml::map::Map<String, toml::Value>, key: &str, default: bool) -> bool {
        config.get(key).map_or(default, |x| x.as_bool().unwrap_or_default())
    }
}

