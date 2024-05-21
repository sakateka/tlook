mod app;
mod term;
mod ui;

use std::env;

use color_eyre::Result;

use crate::app::App;

// TODO: args
fn main() -> Result<()> {
    env_logger::init();
    term::install_hooks()?;

    let input = app::get_input_channel(env::args().nth(1).unwrap_or("stdin".to_string()))?;
    let mut terminal = term::init()?;
    let result = App::with_input(input).run(&mut terminal);
    term::restore().expect("terminal restore");
    result
}
