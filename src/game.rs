use tokio::time;
use tokio::time::{Duration, Instant};

const PULSE_DURATION: Duration = Duration::from_millis(200);
const PULSES_PER_ROUND: u64 = 10;
const ROUNDS_PER_TICK: u64 = 10;

pub struct Game {
    pulse_count: u64,
    round_count: u64,
    tick_count: u64,
}

impl Game {
    pub fn new() -> Self {
        Game {
            pulse_count: 0,
            round_count: 0,
            tick_count: 0,
        }
    }

    pub async fn pulse(&mut self) {
        let started_at = Instant::now();

        self.pulse_count += 1;

        if self.pulse_count % PULSES_PER_ROUND == 0 {
            self.round();
        }

        // println!(
        //     "Pulse {} took {:?} ({:.3}% of max)",
        //     pulse_count,
        //     started_at.elapsed(),
        //     100f64 * (started_at.elapsed().as_nanos() as f64) / (PULSE_DURATION.as_nanos() as f64)
        // );

        time::sleep_until(started_at + PULSE_DURATION).await
    }

    fn round(&mut self) {
        self.round_count += 1;

        println!("--- ROUND ---");

        if self.round_count % ROUNDS_PER_TICK == 0 {
            self.tick();
        }
    }

    fn tick(&mut self) {
        self.tick_count += 1;

        println!("[---- TICK ----]");
    }
}
