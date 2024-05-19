mod app;
mod term;
mod ui;

use color_eyre::Result;

use crate::app::App;

// TODO: args
fn main() -> Result<()> {
    env_logger::init();
    term::install_hooks()?;

    let input = app::spawn_stdin_reader()?;
    let mut terminal = term::init()?;
    let result = App::with_input(input).run(&mut terminal);
    term::restore().expect("terminal restore");
    result
}
