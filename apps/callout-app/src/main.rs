// Windows'ta release derlemede konsol penceresi açılmasın.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    callout_app_lib::run()
}
