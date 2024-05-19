use std::{
    collections::BTreeMap,
    io,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread,
    time::{Duration, Instant},
};

use color_eyre::{
    eyre::{bail, WrapErr},
    Result,
};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::Frame;

use crate::term;
use crate::ui;

#[derive(Debug)]
pub enum CurrentScreen {
    Main,
    Help,
}

#[derive(Debug)]
pub struct App {
    start_time: Instant,
    pub history: Duration,
    pub window: Duration,
    signals: BTreeMap<String, Vec<(f64, f64)>>,
    input: Receiver<String>,
    tick_rate: Duration,
    current_screen: CurrentScreen,
    pub axis_labels: bool,
    pub legend: bool,
    exit: AtomicBool,
}

impl App {
    pub fn with_input(input: Receiver<String>) -> Self {
        Self {
            start_time: Instant::now(),
            // TODO: confugure this
            history: Duration::from_secs(3600),
            window: Duration::from_secs(60),
            signals: BTreeMap::new(),
            input,
            tick_rate: Duration::from_millis(250),
            current_screen: CurrentScreen::Main,
            axis_labels: false,
            legend: true,
            exit: AtomicBool::new(false),
        }
    }
    pub fn run(&mut self, terminal: &mut term::Tui) -> Result<()> {
        let mut last_tick = Instant::now();

        while !self.exit.load(Ordering::Relaxed) {
            terminal.draw(|frame| self.render_frame(frame))?;

            let timeout = self.tick_rate.saturating_sub(last_tick.elapsed());
            self.handle_events(timeout)
                .wrap_err("handle events failed")?;

            if last_tick.elapsed() >= self.tick_rate {
                self.on_tick();
                last_tick = Instant::now();
            }
        }

        Ok(())
    }

    fn render_frame(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.size());
        if let CurrentScreen::Help = self.current_screen {
            ui::render_help(frame);
        }
    }

    /// updates the application's state based on user input
    fn handle_events(&mut self, timeout: Duration) -> Result<()> {
        if event::poll(timeout)? {
            return match event::read()? {
                Event::Key(key_event) if key_event.kind == KeyEventKind::Press => self
                    .handle_key_event(key_event)
                    .wrap_err_with(|| format!("handling key event failed:\n{key_event:#?}")),
                _ => Ok(()),
            };
        }
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<()> {
        match key_event.code {
            KeyCode::Char('q') => {
                if let CurrentScreen::Help = self.current_screen {
                    self.current_screen = CurrentScreen::Main
                } else {
                    self.exit()
                }
            }
            KeyCode::Char('?') => {
                self.current_screen = match self.current_screen {
                    CurrentScreen::Main => CurrentScreen::Help,
                    CurrentScreen::Help => CurrentScreen::Main,
                };
            }
            KeyCode::Char('w') => {
                self.window = Duration::from_secs_f64(self.window.as_secs_f64() * 0.8)
            }
            KeyCode::Char('W') => {
                self.window = Duration::from_secs_f64(self.window.as_secs_f64() * 1.2)
            }
            KeyCode::Char('h') => {
                self.history = Duration::from_secs_f64(self.history.as_secs_f64() / 2.0)
            }
            KeyCode::Char('H') => {
                self.history = Duration::from_secs_f64(self.history.as_secs_f64() * 2.0)
            }
            KeyCode::Char('a') => self.axis_labels = !self.axis_labels,
            KeyCode::Char('l') => self.legend = !self.legend,
            _ => {}
        }
        Ok(())
    }

    fn on_tick(&mut self) {
        let mut count = 0;
        for line in self.input.try_iter() {
            count += 1;
            match Self::parse_input(&line) {
                Ok((name, value)) => {
                    log::debug!("tick line: {name}={value}");
                    let x_sec = self.start_time.elapsed().as_secs_f64();
                    let data = self.signals.entry(name).or_default();
                    data.push((x_sec, value));

                    let oldest = x_sec - self.history.as_secs_f64();
                    let drain_to = data.partition_point(|x| x.0 < oldest);
                    if drain_to > 0 {
                        data.drain(..drain_to);
                    }
                }
                Err(e) => {
                    // TODO: just skip? and don't exit?
                    log::error!("input err {e} for {line}");
                    self.exit();
                }
            }
        }
        log::debug!("tick {count} lines");
    }

    fn parse_input(line: &str) -> Result<(String, f64)> {
        let Some((name, rest)) = line.split_once('=') else {
            bail!("missing delimiter '='");
        };
        Ok((name.to_string(), rest.parse::<f64>()?))
    }

    fn exit(&self) {
        self.exit.store(true, Ordering::Relaxed);
    }

    pub fn elapsed(&self) -> f64 {
        self.start_time.elapsed().as_secs_f64()
    }

    pub fn signals(&self) -> impl Iterator<Item = (&String, &Vec<(f64, f64)>)> + '_ {
        self.signals.iter()
    }
}

pub fn spawn_stdin_reader() -> io::Result<Receiver<String>> {
    let (tx, rx) = mpsc::channel();
    // TODO join handler
    thread::spawn(move || {
        let input = io::stdin();
        for line in input.lines() {
            // TODO remove unwraps
            let line = line.unwrap();

            let res = tx.send(line);
            if res.is_err() {
                log::error!("receiver closed? {res:?}");
                return;
            }
        }
    });
    Ok(rx)
}
