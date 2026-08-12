use std::io::Write;

fn main() {
    let command_name =
        std::env::var("YAZELIX_SCREEN_COMMAND_NAME").unwrap_or_else(|_| "yzs".to_string());
    match yazelix_screen::run_screen_cli(std::env::args().skip(1), &command_name) {
        Ok(()) => {}
        Err(message) => {
            let _ = writeln!(std::io::stderr(), "{message}");
            std::process::exit(1);
        }
    }
}
