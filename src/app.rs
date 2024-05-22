use std::{
    collections::BTreeMap,
    fmt::Display,
    fs::File,
    io::{self, BufRead, BufReader},
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

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ScreenMode {
    Main,
    Help,
    Pause,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum ChartScale {
    Liner,
    Asinh,
}

impl ChartScale {
    pub fn next(&self) -> Self {
        match self {
            ChartScale::Liner => ChartScale::Asinh,
            ChartScale::Asinh => ChartScale::Liner,
        }
    }
}

impl Display for ChartScale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChartScale::Liner => f.write_str("liner"),
            ChartScale::Asinh => f.write_str("asinh"),
        }
    }
}

#[derive(Default)]
pub struct Signals {
    pub original: Vec<f64>,
    pub chart: Vec<(f64, f64)>,
}

impl Signals {
    fn drain(&mut self, oldest: f64) -> usize {
        let drain_to = self.chart.partition_point(|x| x.0 < oldest);
        if drain_to > 0 {
            self.chart.drain(..drain_to);
            self.original.drain(..drain_to);
        }
        self.original.len()
    }
}

pub struct Signal {
    pub name: String,
    pub x_time: f64,
    pub value: f64,
}

pub struct App {
    pub history: Duration,
    pub window: Duration,
    pub scale_mode: ChartScale,
    pub axis_labels: bool,
    pub legend: bool,

    elapsed: f64,
    start_time: Instant,
    input: Receiver<Signal>,
    signals: BTreeMap<String, Signals>,
    tick_rate: Duration,
    current_mode: ScreenMode,
    exit: AtomicBool,
}

impl App {
    pub fn new(input: Receiver<Signal>, start_time: Instant) -> Self {
        Self {
            // TODO: confugure this
            history: Duration::from_secs(3600),
            window: Duration::from_secs(60),
            scale_mode: ChartScale::Liner,
            axis_labels: false,
            legend: true,

            elapsed: 0.0,
            start_time,
            input,
            signals: BTreeMap::new(),
            tick_rate: Duration::from_millis(250),
            current_mode: ScreenMode::Main,
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
        if let ScreenMode::Help = self.current_mode {
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
                if let ScreenMode::Help = self.current_mode {
                    self.current_mode = ScreenMode::Main
                } else {
                    self.exit()
                }
            }
            KeyCode::Char('?') => {
                self.current_mode = match self.current_mode {
                    ScreenMode::Main => ScreenMode::Help,
                    ScreenMode::Help => ScreenMode::Main,
                    mode => mode,
                };
            }
            KeyCode::Char('w') => {
                self.window = Duration::from_secs_f64(self.window.as_secs_f64() * 0.8);
            }
            KeyCode::Char('W') => {
                self.window = Duration::from_secs_f64(self.window.as_secs_f64() * 1.2);
            }
            KeyCode::Char('h') => {
                let x_sec = self.start_time.elapsed().as_secs_f64();
                let oldest = x_sec - self.history.as_secs_f64();
                let keys: Vec<String> = self.signals.keys().cloned().collect();
                for k in keys {
                    let remaining = {
                        let Some(s) = self.signals.get_mut(&k) else {
                            continue;
                        };
                        s.drain(oldest)
                    };
                    if remaining == 0 {
                        self.signals.remove(&k);
                    }
                }
                self.history = Duration::from_secs_f64(self.history.as_secs_f64() / 2.0);
            }
            KeyCode::Char('H') => {
                self.history = Duration::from_secs_f64(self.history.as_secs_f64() * 2.0);
            }
            KeyCode::Char('a') => self.axis_labels = !self.axis_labels,
            KeyCode::Char('l') => self.legend = !self.legend,
            KeyCode::Char(' ') => {
                self.current_mode = match self.current_mode {
                    ScreenMode::Main => ScreenMode::Pause,
                    ScreenMode::Pause => ScreenMode::Main,
                    mode => mode,
                };
            }
            KeyCode::Char('s') => {
                self.scale_mode = self.scale_mode.next();
                self.apply_new_scale_mode()
            }
            _ => {}
        }
        Ok(())
    }

    fn on_tick(&mut self) {
        if self.current_mode == ScreenMode::Pause {
            return;
        }
        self.elapsed = self.start_time.elapsed().as_secs_f64();

        let mut count = 0;
        for signal in self.input.try_iter() {
            let data = self.signals.entry(signal.name.clone()).or_default();
            data.original.push(signal.value);
            data.chart
                .push((signal.x_time, Self::scale(self.scale_mode, signal.value)));

            let oldest = signal.x_time - self.history.as_secs_f64();
            data.drain(oldest);
            count += 1;
        }
        log::debug!("tick: receive {count} signals");
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

    fn apply_new_scale_mode(&mut self) {
        for (_, item) in self.signals.iter_mut() {
            item.chart.iter_mut().enumerate().for_each(|(idx, data)| {
                data.1 = Self::scale(self.scale_mode, item.original[idx]);
            });
        }
    }

    fn scale(mode: ChartScale, value: f64) -> f64 {
        match mode {
            ChartScale::Liner => value,
            ChartScale::Asinh => value.asinh(),
        }
    }

    pub fn elapsed(&self) -> f64 {
        self.elapsed
    }

    pub fn signals(&self) -> impl Iterator<Item = (&String, &Signals)> + '_ {
        self.signals.iter()
    }
}

pub fn stdin_reader() -> Box<dyn Iterator<Item = io::Result<String>>> {
    Box::new(io::stdin().lines())
}

pub fn file_reader(file: String) -> Box<dyn Iterator<Item = io::Result<String>>> {
    let f = File::open(file).unwrap();
    Box::new(BufReader::new(f).lines())
}

pub fn get_input_channel(mode: String, start_time: Instant) -> io::Result<Receiver<Signal>> {
    let (tx, rx) = mpsc::channel();

    // TODO join handler
    thread::spawn(move || {
        let lines = if mode == "stdin" {
            stdin_reader()
        } else {
            file_reader(mode)
        };
        for line in lines {
            let Ok(line) = line else {
                log::error!("ignore input error: {:?}", line);
                continue;
            };

            for metric in line.split(';').filter(|x| !x.is_empty()) {
                match App::parse_input(metric) {
                    Ok((name, value)) => {
                        log::debug!("line: {name}={value}");
                        let x_time = start_time.elapsed().as_secs_f64();
                        let res = tx.send(Signal {
                            name,
                            x_time,
                            value,
                        });
                        if res.is_err() {
                            log::error!("receiver closed? {res:?}");
                            return;
                        }
                    }
                    Err(e) => {
                        log::error!("ignore parsing err {e} for {line}");
                        continue;
                    }
                }
            }
        }
    });
    Ok(rx)
}
