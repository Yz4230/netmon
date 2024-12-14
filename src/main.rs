use std::{
    collections::HashMap,
    sync::{
        atomic::{self, AtomicBool},
        Arc, RwLock,
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::{anyhow, Result};
use blake2::{digest::consts::U3, Blake2b, Digest};
use ratatui::{
    crossterm::event::{self, Event, KeyCode},
    style::{Color, Stylize},
    symbols::Marker,
    text::Text,
    widgets::{Axis, Block, Chart, Dataset, GraphType},
    DefaultTerminal, Frame,
};
use sysinfo::Networks;

fn main() -> Result<()> {
    let mut terminal = ratatui::init();
    terminal.clear()?;
    let app_result = App::new().run(terminal);
    ratatui::restore();
    app_result
}

struct App {
    networks: Arc<RwLock<Networks>>,
    running: Arc<AtomicBool>,
    data: Arc<RwLock<HashMap<String, Vec<(f64, f64)>>>>,

    collector: Option<thread::JoinHandle<()>>,
    collector_interval: Duration,
}

impl App {
    fn new() -> Self {
        Self {
            networks: Arc::new(RwLock::new(Networks::new_with_refreshed_list())),
            running: Arc::new(AtomicBool::new(false)),
            data: Arc::new(RwLock::new(HashMap::new())),

            collector: None,
            collector_interval: Duration::from_millis(100),
        }
    }

    fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        self.running.store(true, atomic::Ordering::Relaxed);
        self.start_collector();

        let tick_rate = Duration::from_millis(100);
        let mut last_tick = Instant::now();

        loop {
            terminal.draw(|frame| self.draw(frame).unwrap())?;

            let timeout = tick_rate.saturating_sub(last_tick.elapsed());
            if event::poll(timeout)? {
                if let Event::Key(input) = event::read()? {
                    if input.code == KeyCode::Char('q') {
                        terminal.draw(|frame| {
                            frame.render_widget(Text::from("👋 Quitting..."), frame.area())
                        })?;
                        break;
                    }
                }
            }

            if last_tick.elapsed() >= tick_rate {
                last_tick = Instant::now();
            }
        }

        self.running.store(false, atomic::Ordering::Relaxed);
        self.collector
            .ok_or(anyhow!("Collector thread is not running"))?
            .join()
            .unwrap();

        Ok(())
    }

    fn start_collector(&mut self) {
        let networks = self.networks.clone();
        let running = self.running.clone();
        let data = self.data.clone();
        let interval = self.collector_interval;
        self.collector = Some(thread::spawn(move || {
            let start_time = Instant::now();
            while running.load(atomic::Ordering::Relaxed) {
                let elapsed = start_time.elapsed().as_secs_f64();
                networks.write().unwrap().refresh(true);
                for (name, network) in networks.read().unwrap().iter() {
                    let mut data = data.write().unwrap();
                    let ifdata = data.entry(name.clone()).or_insert_with(Vec::new);
                    let transmitted = network.transmitted() as f64;
                    ifdata.push((elapsed, transmitted / interval.as_secs_f64()));
                }
                thread::sleep(interval);
            }
        }));
    }

    fn draw(&self, frame: &mut Frame) -> Result<()> {
        let data = self.data.read().unwrap();

        let datasets = data
            .iter()
            .map(|(name, data)| {
                let mut hasher = Blake2b::<U3>::new();
                hasher.update(name.as_bytes());
                let hash = hasher.finalize();
                let color = Color::Rgb(hash[0], hash[1], hash[2]);

                Dataset::default()
                    .name(name.clone())
                    .marker(Marker::Braille)
                    .graph_type(GraphType::Line)
                    .fg(color)
                    .data(data)
            })
            .collect::<Vec<_>>();

        let last_ts = data
            .values()
            .map(|d| d.last().map_or(0.0, |(t, _)| *t))
            .max_by(|a, b| a.total_cmp(b))
            .unwrap_or(0.0);

        let max_transmitted = data
            .values()
            .map(|d| {
                d.iter()
                    .map(|(_, t)| *t)
                    .max_by(|a, b| a.total_cmp(b))
                    .unwrap_or(0.0)
            })
            .max_by(|a, b| a.total_cmp(b))
            .unwrap_or(0.0);

        let chart = Chart::new(datasets)
            .block(Block::bordered())
            .x_axis(
                Axis::default()
                    .title("Time")
                    .bounds([0.0, last_ts])
                    .labels(vec!["0".gray(), format!("{:.1}s", last_ts).gray()]),
            )
            .y_axis(
                Axis::default()
                    .title("Transmitted")
                    .bounds([0.0, max_transmitted])
                    .labels(vec![
                        "0".gray(),
                        (max_transmitted * 8f64).humanize_bps().gray(),
                    ]),
            );

        frame.render_widget(chart, frame.area());

        Ok(())
    }
}

trait NumericFormatter {
    fn humanize_size(self) -> String;
    fn humanize_bps(self) -> String;
}

impl<T: Into<f64>> NumericFormatter for T {
    fn humanize_size(self) -> String {
        let size = self.into();
        const UNITS: [&str; 9] = ["", "K", "M", "G", "T", "P", "E", "Z", "Y"];
        let unit = (0..=UNITS.len())
            .find(|i| size < (1024f64).powi(*i as i32))
            .unwrap_or(UNITS.len())
            .saturating_sub(1);

        format!("{:.1}{}", size / (1024f64).powi(unit as i32), UNITS[unit])
    }

    fn humanize_bps(self) -> String {
        format!("{}bps", self.humanize_size())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_humanize_size() {
        assert_eq!(0.humanize_size(), "0.0");
        assert_eq!(1.humanize_size(), "1.0");
        assert_eq!(1024.humanize_size(), "1.0K");
        assert_eq!(1024f64.powi(2).humanize_size(), "1.0M");
        assert_eq!(1024f64.powi(3).humanize_size(), "1.0G");
        assert_eq!(1024f64.powi(4).humanize_size(), "1.0T");
        assert_eq!(1024f64.powi(5).humanize_size(), "1.0P");
        assert_eq!(1024f64.powi(6).humanize_size(), "1.0E");
        assert_eq!(1024f64.powi(7).humanize_size(), "1.0Z");
        assert_eq!(1024f64.powi(8).humanize_size(), "1.0Y");
    }
}
