mod application;
mod domain;
mod infrastructure;
mod presentation;

use presentation::cli::run;

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
