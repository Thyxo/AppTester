// Skjul konsolvinduet i release-builds på Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    pwa_launcher_lib::run()
}
