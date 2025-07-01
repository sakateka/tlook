use std::{
    collections::{BTreeMap, HashMap},
    fmt::Display,
    fs::File,
    io::{self, BufRead, BufReader},
    process::{Command, Stdio},
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
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::Frame;

use crate::term;
use crate::ui;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ScreenMode {
    Main,
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

#[derive(Default)]
pub struct ChartBounds {
    pub max_name_len: usize,
    pub original_min: f64,
    pub original_max: f64,
    pub scaled_min: f64,
    pub scaled_max: f64,
    pub max_values: HashMap<String, f64>,
    pub label_values: HashMap<String, f64>,
    pub cursor_points: [(f64, f64); 3],
}

#[derive(Debug)]
pub struct ChartLine<'a> {
    pub color_idx: usize,
    pub name: String,
    pub data: &'a [(f64, f64)],
}

pub struct App {
    pub history: Duration,
    pub window: Duration,
    pub move_speed: f64,
    pub scale_mode: ChartScale,
    pub axis_labels: bool,
    pub legend: bool,
    pub show_cursor: bool,

    input: Receiver<Signal>,
    current_mode: ScreenMode,
    start_point: Instant,
    elapsed: f64,
    signals: BTreeMap<String, Signals>,
    tick_rate: Duration,
    show_help: bool,

    chart_bounds: ChartBounds,
    cursor_position: f64,

    exit: AtomicBool,
}

impl App {
    pub fn new(input: Receiver<Signal>, start_time: Instant) -> Self {
        let window = Duration::from_secs(60);
        Self {
            // TODO: confugure this
            history: Duration::from_secs(3600),
            window,
            move_speed: 1.0,
            scale_mode: ChartScale::Liner,
            axis_labels: false,
            legend: true,

            input,
            current_mode: ScreenMode::Main,
            elapsed: 0.0,
            start_point: start_time,
            signals: BTreeMap::new(),
            tick_rate: Duration::from_millis(250),
            show_help: false,

            chart_bounds: Default::default(),
            show_cursor: false,
            cursor_position: window.as_secs_f64() / 2.0,

            exit: AtomicBool::new(false),
        }
    }
    pub fn run(&mut self, terminal: &mut term::Tui) -> Result<()> {
        let mut last_tick = Instant::now();

        while !self.exit.load(Ordering::Relaxed) {
            self.set_chart_bounds();
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
        frame.render_widget(self, frame.area());
        if self.show_help {
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

    fn handle_key_event(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('q') => {
                if self.show_help {
                    self.show_help = false;
                } else {
                    self.exit()
                }
            }
            KeyCode::Char('?') => self.show_help = !self.show_help,
            KeyCode::Char('w') => {
                self.window = Duration::from_secs_f64(self.window.as_secs_f64() * 0.8);
                self.cursor_position *= 0.8;
            }
            KeyCode::Char('W') => {
                self.window = Duration::from_secs_f64(self.window.as_secs_f64() * 1.2);
                self.cursor_position *= 1.2;
            }
            KeyCode::Char('h') => {
                let x_sec = self.start_point.elapsed().as_secs_f64();
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
                };
            }
            KeyCode::Char('s') => {
                self.scale_mode = self.scale_mode.next();
                self.apply_new_scale_mode()
            }
            KeyCode::Char('m') => self.move_speed /= 10.0,
            KeyCode::Char('M') => self.move_speed *= 10.0,
            KeyCode::Left if self.in_pause() && key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.elapsed -= self.move_speed;
            }
            KeyCode::Right if self.in_pause() && key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.elapsed += self.move_speed
            }
            KeyCode::Left if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                let new_pos = self.cursor_position - self.move_speed;
                self.cursor_position = new_pos.clamp(0.0, self.window());
            }
            KeyCode::Right if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                let new_pos = self.cursor_position + self.move_speed;
                self.cursor_position = new_pos.clamp(0.0, self.window());
            }
            KeyCode::Char('c') => self.show_cursor = !self.show_cursor,
            _ => {}
        }
        Ok(())
    }

    fn on_tick(&mut self) {
        if self.current_mode == ScreenMode::Pause {
            return;
        }
        self.elapsed = self.start_point.elapsed().as_secs_f64();

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
    pub fn window(&self) -> f64 {
        self.window.as_secs_f64()
    }
    fn left_border(&self) -> f64 {
        self.elapsed() - self.window()
    }
    fn on_screen(&self, time: f64) -> bool {
        let left_border = self.left_border();
        let right_border = self.elapsed();
        time >= left_border && time <= right_border
    }

    fn in_pause(&self) -> bool {
        self.current_mode == ScreenMode::Pause
    }

    pub fn chart_bounds(&self) -> &ChartBounds {
        &self.chart_bounds
    }
    fn set_chart_bounds(&mut self) {
        let mut max_values = HashMap::new();
        let mut cursor_values = HashMap::new();
        let cursor_point = self.cursor_point();
        let (max_name_len, original_min_max, scaled_min_max) = self
            .signals
            .iter()
            .map(|(name, set)| {
                let (original_min_max, scaled_min_max) = set
                    .original
                    .iter()
                    .zip(set.chart.iter())
                    .filter(|(_, (elapsed, _))| self.on_screen(*elapsed))
                    .inspect(|&item| {
                        let (original, (elapsed, _)) = item;
                        let val = cursor_values.entry(name.clone()).or_insert((f64::MAX, 0.0));
                        let point_diff = (cursor_point - elapsed).abs();
                        if point_diff < val.0 {
                            val.0 = point_diff;
                            val.1 = *original;
                        }
                    })
                    .fold(
                        ((f64::MAX, f64::MIN), (f64::MAX, f64::MIN)),
                        |(acc_orig, acc_scaled), (orig_val, (_, val))| {
                            (
                                (acc_orig.0.min(*orig_val), acc_orig.1.max(*orig_val)),
                                (acc_scaled.0.min(*val), acc_scaled.1.max(*val)),
                            )
                        },
                    );
                (name, (original_min_max, scaled_min_max))
            })
            .fold(
                (0, (f64::MAX, f64::MIN), (f64::MAX, f64::MIN)),
                |(name_len, oacc, sacc), (name, ((omin, omax), (smin, smax)))| {
                    let val = max_values.entry(name.clone()).or_insert(f64::MIN);
                    *val = val.max(omax);

                    (
                        name_len.max(name.len()),
                        (oacc.0.min(omin), oacc.1.max(omax)),
                        (sacc.0.min(smin), sacc.1.max(smax)),
                    )
                },
            );

        let cursor_points = [
            (cursor_point, scaled_min_max.0),
            (cursor_point, scaled_min_max.1),
            (cursor_point, scaled_min_max.0),
        ];
        let label_values = cursor_values
            .into_iter()
            .map(|(name, (_, val))| (name, val))
            .collect();

        self.chart_bounds = ChartBounds {
            max_name_len,
            original_min: original_min_max.0,
            original_max: original_min_max.1,
            scaled_min: scaled_min_max.0,
            scaled_max: scaled_min_max.1,
            max_values,
            label_values,
            cursor_points,
        }
    }

    pub fn cursor_point(&self) -> f64 {
        self.left_border() + self.cursor_position
    }

    pub fn datasets(&self, bounds: &ChartBounds) -> Vec<ChartLine> {
        let mut sets = Vec::with_capacity(self.signals.len());
        if self.show_cursor {
            sets.push(ChartLine {
                color_idx: 0,
                name: "".to_string(),
                data: self.chart_bounds.cursor_points.as_slice(),
            });
        }
        sets.extend(
            self.signals
                .iter()
                .enumerate()
                .filter(|(_, (_, set))| set.chart.iter().any(|v| self.on_screen(v.0)))
                .map(|(color_idx, (name, set))| {
                    let curr_val = if self.show_cursor {
                        bounds
                            .label_values
                            .get(name)
                            .map_or("-".into(), |v| format!("{:.2}", v))
                    } else {
                        set.original
                            .iter()
                            .zip(set.chart.iter())
                            .rev()
                            .find(|(_, (time, _))| self.on_screen(*time))
                            .map_or("-".into(), |v| format!("{:.2}", v.0))
                    };
                    let max_in_window = bounds
                        .max_values
                        .get(name)
                        .map_or("-".into(), |v| format!("{:.2}", v));
                    let name = format!(
                        "{name:0$} {1} (max {2})",
                        bounds.max_name_len, curr_val, max_in_window,
                    );
                    ChartLine {
                        color_idx,
                        name,
                        data: set.chart.as_slice(),
                    }
                }),
        );
        sets
    }
}

pub fn stdin_reader() -> Box<dyn Iterator<Item = io::Result<String>>> {
    Box::new(io::stdin().lines())
}

pub fn file_reader(file: String) -> Box<dyn Iterator<Item = io::Result<String>>> {
    let f = File::open(file).unwrap();
    Box::new(BufReader::new(f).lines())
}

fn process_lines_from_iterator<I>(lines: I, start_time: Instant, tx: mpsc::Sender<Signal>)
where
    I: Iterator<Item = io::Result<String>>,
{
    for line in lines {
        let Ok(line) = line else {
            log::error!("ignore input error: {:?}", line);
            continue;
        };

        if !process_metric_line_with_context(&line, "line", start_time, &tx) {
            return;
        }
    }
}

pub fn get_input_channel_from_stdin(start_time: Instant) -> io::Result<Receiver<Signal>> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let lines = stdin_reader();
        process_lines_from_iterator(lines, start_time, tx);
    });
    Ok(rx)
}

pub fn get_input_channel_from_file(
    file: String,
    start_time: Instant,
) -> io::Result<Receiver<Signal>> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let lines = file_reader(file);
        process_lines_from_iterator(lines, start_time, tx);
    });
    Ok(rx)
}

fn is_shell_script(command: &str) -> bool {
    command.contains(';')
        || command.contains('|')
        || command.contains("&&")
        || command.contains("||")
        || command.contains('$')
        || command.contains('<')
        || command.contains('>')
}

fn parse_command_args(command: &str) -> Result<(String, Vec<String>), String> {
    if is_shell_script(command) {
        // Execute as shell script
        Ok((
            "sh".to_string(),
            vec!["-c".to_string(), command.to_string()],
        ))
    } else {
        // Parse as individual command with arguments
        let parsed_args = shell_words::split(command)
            .map_err(|e| format!("Failed to parse command '{}': {}", command, e))?;

        if parsed_args.is_empty() {
            return Err("Empty command string".to_string());
        }

        let cmd = parsed_args[0].clone();
        let args = parsed_args[1..].to_vec();
        Ok((cmd, args))
    }
}

fn process_metric_line_with_context(
    line: &str,
    context: &str,
    start_time: Instant,
    tx: &mpsc::Sender<Signal>,
) -> bool {
    for metric in line.split(';').filter(|x| !x.is_empty()) {
        match App::parse_input(metric) {
            Ok((name, value)) => {
                log::debug!("'{}': {name}={value}", context);
                let x_time = start_time.elapsed().as_secs_f64();
                let res = tx.send(Signal {
                    name,
                    x_time,
                    value,
                });
                if res.is_err() {
                    log::error!("receiver closed? {res:?}");
                    return false;
                }
            }
            Err(e) => {
                log::debug!("ignore parsing err {e} for {line} from '{}'", context);
                continue;
            }
        }
    }
    true
}

pub fn get_input_channel_from_processes(
    processes: Vec<String>,
    start_time: Instant,
    tx: mpsc::Sender<Signal>,
) {
    for process_str in processes {
        let tx_clone = tx.clone();
        let start_time_clone = start_time;

        thread::spawn(move || {
            loop {
                log::info!("Starting process: {}", process_str);

                let (cmd, args) = match parse_command_args(&process_str) {
                    Ok((cmd, args)) => (cmd, args),
                    Err(e) => {
                        log::error!("{}", e);
                        thread::sleep(Duration::from_secs(5));
                        continue;
                    }
                };

                log::info!("Starting process: {process_str}");
                let mut child = match Command::new(&cmd)
                    .args(&args)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                {
                    Ok(child) => child,
                    Err(e) => {
                        log::error!("Failed to spawn process '{}': {}", process_str, e);
                        thread::sleep(Duration::from_secs(5));
                        continue;
                    }
                };

                // Read from stdout continuously for long-running processes
                if let Some(stdout) = child.stdout.take() {
                    let reader = BufReader::new(stdout);
                    for line in reader.lines() {
                        let line = match line {
                            Ok(line) => line,
                            Err(e) => {
                                log::error!("Failed to read from process '{}': {}", process_str, e);
                                break;
                            }
                        };

                        if !process_metric_line_with_context(
                            &line,
                            &process_str,
                            start_time_clone,
                            &tx_clone,
                        ) {
                            return;
                        }
                    }
                }

                // Wait for the process to finish
                match child.wait() {
                    Ok(status) => {
                        log::info!("Process '{}' exited with status: {}", process_str, status);
                    }
                    Err(e) => {
                        log::error!("Failed to wait for process '{}': {}", process_str, e);
                    }
                }

                // Restart the process after a short delay
                log::info!("Restarting process '{}' in 1 second...", process_str);
                thread::sleep(Duration::from_secs(1));
            }
        });
    }
}

pub fn get_input_channel_from_commands(
    commands: Vec<String>,
    interval_secs: u64,
    start_time: Instant,
    tx: mpsc::Sender<Signal>,
) {
    for command_str in commands {
        let tx_clone = tx.clone();
        let start_time_clone = start_time;
        let interval = Duration::from_secs(interval_secs);

        thread::spawn(move || {
            loop {
                log::info!("Executing command: {}", command_str);

                let (cmd, args) = match parse_command_args(&command_str) {
                    Ok((cmd, args)) => (cmd, args),
                    Err(e) => {
                        log::error!("{}", e);
                        thread::sleep(interval);
                        continue;
                    }
                };

                // Spawn the command and wait for it to complete
                let output = match Command::new(&cmd).args(&args).output() {
                    Ok(output) => output,
                    Err(e) => {
                        log::error!("Failed to execute command '{}': {}", command_str, e);
                        thread::sleep(interval);
                        continue;
                    }
                };

                // Process the output
                let stdout_str = String::from_utf8_lossy(&output.stdout);
                for line in stdout_str.lines() {
                    if !process_metric_line_with_context(
                        line,
                        &command_str,
                        start_time_clone,
                        &tx_clone,
                    ) {
                        return;
                    }
                }

                if !output.status.success() {
                    let stderr_str = String::from_utf8_lossy(&output.stderr);
                    log::warn!(
                        "Command '{}' failed with status {}: {}",
                        command_str,
                        output.status,
                        stderr_str
                    );
                }

                // Wait for the specified interval before running again
                thread::sleep(interval);
            }
        });
    }
}

pub fn get_input_channel_from_processes_and_commands(
    processes: Vec<String>,
    commands: Vec<String>,
    interval_secs: u64,
    start_time: Instant,
) -> io::Result<Receiver<Signal>> {
    let (tx, rx) = mpsc::channel();

    // Handle long-running processes
    if !processes.is_empty() {
        get_input_channel_from_processes(processes, start_time, tx.clone());
    }

    // Handle interval-based commands
    if !commands.is_empty() {
        get_input_channel_from_commands(commands, interval_secs, start_time, tx.clone());
    }

    // Drop the original sender so the channel closes when all threads finish
    drop(tx);
    Ok(rx)
}
