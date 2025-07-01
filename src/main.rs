mod app;
mod term;
mod ui;

use std::time::Instant;

use clap::Parser;
use color_eyre::Result;

use crate::app::App;

#[derive(Parser)]
#[command(name = "tlook")]
#[command(about = "A terminal-based metrics visualizer")]
pub struct Args {
    /// Long-running processes to monitor (can be specified multiple times)
    #[arg(short = 'p', long = "process", action = clap::ArgAction::Append)]
    pub processes: Vec<String>,

    /// Short-lived commands to run repeatedly (can be specified multiple times)
    #[arg(short = 'c', long = "command", action = clap::ArgAction::Append)]
    pub commands: Vec<String>,

    /// Interval in seconds for repeating commands (default: 1)
    #[arg(long = "interval", default_value = "1")]
    pub interval: u64,

    /// Read from stdin instead of commands/processes
    #[arg(long = "stdin")]
    pub stdin: bool,

    /// Read from a file instead of commands/processes
    #[arg(short = 'f', long = "file")]
    pub file: Option<String>,
}

fn main() -> Result<()> {
    env_logger::init();
    term::install_hooks()?;

    let args = Args::parse();
    let now = Instant::now();

    let input = if args.stdin {
        app::get_input_channel_from_stdin(now)?
    } else if let Some(file) = args.file {
        app::get_input_channel_from_file(file, now)?
    } else if !args.processes.is_empty() || !args.commands.is_empty() {
        app::get_input_channel_from_processes_and_commands(
            args.processes,
            args.commands,
            args.interval,
            now,
        )?
    } else {
        eprintln!("Error: Must specify either --stdin, --file, or one or more -p/-c commands");
        std::process::exit(1);
    };

    let mut terminal = term::init()?;
    let result = App::new(input, now).run(&mut terminal);
    term::restore().expect("terminal restore");
    result
}
