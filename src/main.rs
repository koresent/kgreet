mod auth;
mod config;
mod i18n;
mod media;
mod system;
mod ui;

use std::path::PathBuf;
use std::process;

use gtk4::gio;
use gtk4::prelude::*;

use crate::config::Config;
use crate::ui::greeter_window::{RuntimeOptions, launch_ui};

struct CliArgs {
    config_path: Option<PathBuf>,
    windowed: bool,
    mock: bool,
}

fn parse_args() -> CliArgs {
    let mut args = std::env::args().skip(1);
    let mut parsed = CliArgs {
        config_path: None,
        windowed: false,
        mock: false,
    };

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-c" | "--config" => {
                if let Some(path) = args.next() {
                    parsed.config_path = Some(PathBuf::from(path));
                } else {
                    eprintln!("kgreet: --config requires a file path");
                    process::exit(2);
                }
            }
            "--windowed" | "--preview" => {
                parsed.windowed = true;
            }
            "--mock" => {
                parsed.mock = true;
            }
            "-h" | "--help" => {
                print_help();
                process::exit(0);
            }
            "-V" | "--version" => {
                println!("kgreet {}", env!("CARGO_PKG_VERSION"));
                process::exit(0);
            }
            other => {
                eprintln!("kgreet: unknown argument '{other}'");
                print_help();
                process::exit(2);
            }
        }
    }

    parsed
}

fn print_help() {
    println!(
        "kgreet {}\n\
         Usage: kgreet [OPTIONS]\n\n\
         Options:\n  \
           -c, --config <PATH>   Path to YAML configuration file\n  \
               --windowed        Open in a standard window instead of layer-shell\n  \
               --mock            Simulate greetd/logind calls for safe local testing\n  \
           -h, --help            Show help\n  \
           -V, --version         Show version",
        env!("CARGO_PKG_VERSION")
    );
}

fn main() {
    let cli = parse_args();
    let (cfg, config_warning) = Config::load(cli.config_path.as_deref());

    let app = gtk4::Application::builder()
        .application_id("dev.koresent.kgreet")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();

    let opts = RuntimeOptions {
        windowed: cli.windowed,
        mock: cli.mock,
        config_warning,
    };

    app.connect_activate(move |app| {
        launch_ui(app, cfg.clone(), opts.clone());
    });

    let empty_args: [&str; 0] = [];
    app.run_with_args(&empty_args);
}
