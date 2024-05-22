mod app;
mod term;
mod ui;

use std::{env, time::Instant};

use color_eyre::Result;

use crate::app::App;

// TODO: clap
fn main() -> Result<()> {
    env_logger::init();
    term::install_hooks()?;

    let now = Instant::now();
    let input_mode = env::args().nth(1).unwrap_or("stdin".to_string());
    let input = app::get_input_channel(input_mode, now)?;
    let mut terminal = term::init()?;
    let result = App::new(input, now).run(&mut terminal);
    term::restore().expect("terminal restore");
    result
}
