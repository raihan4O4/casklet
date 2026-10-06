// The GUI must never allocate or attach to a Windows console, including when
// launched from a terminal. The separate casklet CLI keeps its console subsystem.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() {
    casklet_desktop_lib::run();
}
