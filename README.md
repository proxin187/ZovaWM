# ZovaWM

ZovaWM is a window manager for x11 written in rust.

![ZovaWM screenshot](assets/zovawm.png)


## Why use ZovaWM?

The main purpose of ZovaWM is a full-featured minimal window manager,
this makes it a great lightweight option while still having features such as
multi monitor support.


## Running from a display manager 

Running ZovaWM from a display manager is really easy,
you only need to write the following into `/usr/share/xsessions/zovawm.desktop`.

```
[Desktop Entry]
Name=zovawm
Comment=zova window manager
Exec=zovawm
Type=Application
```


## Running in Xephyr

### Single monitor (Xephyr)

```
$ Xephyr -br -ac -noreset -screen 800x600 :1
$ DISPLAY=:1 cargo run
```


### Multi monitor (Xephyr)

When running in multi monitor Xephyr it is a know problem that the cursor can get a little messed up sometimes,
to my knowledge this has nothing to do with ZovaWM as i have tested in other WM's such as dwm and gotten the same result.

```
$ Xephyr -br -ac -noreset +xinerama -screen 800x600 -screen 800x600 :1
$ DISPLAY=:1 cargo run
```


