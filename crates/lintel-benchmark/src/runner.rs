extern crate alloc;

use alloc::fmt;
use core::time::Duration;
use std::process::Command;
use std::time::Instant;

use anyhow::Result;

pub struct Stats {
    pub min: Duration,
    pub max: Duration,
    pub mean: Duration,
    pub median: Duration,
    pub runs: usize,
}

impl fmt::Display for Stats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "mean: {:>8.1}ms  min: {:>8.1}ms  max: {:>8.1}ms  median: {:>8.1}ms  (n={})",
            self.mean_ms(),
            self.min_ms(),
            self.max_ms(),
            self.median_ms(),
            self.runs,
        )
    }
}

impl Stats {
    pub fn mean_ms(&self) -> f64 {
        self.mean.as_secs_f64() * 1000.0
    }
    pub fn min_ms(&self) -> f64 {
        self.min.as_secs_f64() * 1000.0
    }
    pub fn max_ms(&self) -> f64 {
        self.max.as_secs_f64() * 1000.0
    }
    pub fn median_ms(&self) -> f64 {
        self.median.as_secs_f64() * 1000.0
    }
}

pub fn compute_stats(durations: &[Duration]) -> Stats {
    let mut sorted = durations.to_vec();
    sorted.sort();
    let total: Duration = sorted.iter().sum();
    let n = sorted.len();
    Stats {
        min: sorted[0],
        max: sorted[n - 1],
        #[allow(clippy::cast_possible_truncation)]
        mean: total / n as u32,
        median: sorted[n / 2],
        runs: n,
    }
}

pub fn run_timed(cmd: &str, args: &[&str], warmup: usize, runs: usize) -> Result<Stats> {
    for _ in 0..warmup {
        Command::new(cmd)
            .args(args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()?;
    }

    let mut durations = Vec::with_capacity(runs);
    for _ in 0..runs {
        let start = Instant::now();
        Command::new(cmd)
            .args(args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()?;
        durations.push(start.elapsed());
    }

    Ok(compute_stats(&durations))
}

pub fn run_timed_with_setup(
    setup: impl Fn() -> Result<()>,
    cmd: &str,
    args: &[&str],
    runs: usize,
) -> Result<Stats> {
    let mut durations = Vec::with_capacity(runs);
    for _ in 0..runs {
        setup()?;
        let start = Instant::now();
        Command::new(cmd)
            .args(args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()?;
        durations.push(start.elapsed());
    }

    Ok(compute_stats(&durations))
}

pub fn which(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}
