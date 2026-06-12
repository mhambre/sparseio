use std::time::Duration;

use futures_timer::Delay;
use parking_lot::Mutex;

const MT_N: usize = 624;
const MT_M: usize = 397;
const MATRIX_A: u32 = 0x9908_b0df;
const UPPER_MASK: u32 = 0x8000_0000;
const LOWER_MASK: u32 = 0x7fff_ffff;

/// Deterministic MT19937 generator used for repeatable mock jitter.
pub(crate) struct MersenneTwister {
    state: [u32; MT_N],
    index: usize,
}

impl MersenneTwister {
    /// Seed a generator with the standard MT19937 initialization routine.
    pub(crate) fn new(seed: u32) -> Self {
        let mut state = [0; MT_N];
        state[0] = seed;

        for i in 1..MT_N {
            state[i] = 1_812_433_253u32
                .wrapping_mul(state[i - 1] ^ (state[i - 1] >> 30))
                .wrapping_add(i as u32);
        }

        Self { state, index: MT_N }
    }

    /// Return the next tempered 32-bit value from the generator.
    pub(crate) fn next_u32(&mut self) -> u32 {
        if self.index >= MT_N {
            self.twist();
        }

        let mut y = self.state[self.index];
        self.index += 1;

        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^ (y >> 18)
    }

    /// Refresh the state array after the current batch is exhausted.
    fn twist(&mut self) {
        for i in 0..MT_N {
            let x = (self.state[i] & UPPER_MASK) | (self.state[(i + 1) % MT_N] & LOWER_MASK);
            let mut xa = x >> 1;

            if x & 1 != 0 {
                xa ^= MATRIX_A;
            }

            self.state[i] = self.state[(i + MT_M) % MT_N] ^ xa;
        }

        self.index = 0;
    }
}

/// Async latency profile with deterministic jitter.
pub(crate) struct Latency {
    base: Duration,
    jitter: Duration,
    rng: Mutex<MersenneTwister>,
}

impl Latency {
    /// Create a latency profile with a fixed base delay and jitter range.
    pub(crate) fn new(base: Duration, jitter: Duration, seed: u32) -> Self {
        Self {
            base,
            jitter,
            rng: Mutex::new(MersenneTwister::new(seed)),
        }
    }

    /// Return the default profile for upstream reader calls.
    pub(crate) fn reader() -> Self {
        Self::new(Duration::from_millis(40), Duration::from_millis(20), 0x5254_0001)
    }

    /// Return the default profile for intra-datacenter cache calls.
    pub(crate) fn intra_dc() -> Self {
        Self::new(Duration::from_micros(200), Duration::from_micros(100), 0x1dc0_0001)
    }

    /// Await the next configured delay without blocking the executor thread.
    pub(crate) async fn pause(&self) {
        let delay = self.next_delay();

        if !delay.is_zero() {
            Delay::new(delay).await;
        }
    }

    /// Calculate the next base-plus-jitter duration for this profile.
    fn next_delay(&self) -> Duration {
        let jitter_nanos = self.jitter.as_nanos();

        if jitter_nanos == 0 {
            return self.base;
        }

        let jitter = u128::from(self.rng.lock().next_u32()) % (jitter_nanos + 1);
        let total = self.base.as_nanos().saturating_add(jitter);
        let nanos = total.min(u128::from(u64::MAX)) as u64;

        Duration::from_nanos(nanos)
    }
}

impl Default for Latency {
    /// Create a no-delay profile for tests that need immediate completion.
    fn default() -> Self {
        Self::new(Duration::ZERO, Duration::ZERO, 0)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{Latency, MersenneTwister};

    #[test]
    fn mersenne_twister_is_deterministic_for_seed() {
        let mut rng = MersenneTwister::new(5489);

        assert_eq!(rng.next_u32(), 3_499_211_612);
        assert_eq!(rng.next_u32(), 581_869_302);
        assert_eq!(rng.next_u32(), 3_890_346_734);
    }

    #[test]
    fn default_latency_has_no_delay() {
        let latency = Latency::default();

        assert_eq!(latency.next_delay(), Duration::ZERO);
    }

    #[test]
    fn latency_adds_seeded_jitter_to_base() {
        let latency = Latency::new(Duration::from_nanos(10), Duration::from_nanos(3), 5489);

        assert_eq!(latency.next_delay(), Duration::from_nanos(10));
        assert_eq!(latency.next_delay(), Duration::from_nanos(12));
    }

    #[test]
    fn reader_latency_is_significantly_higher_than_intra_dc_latency() {
        assert!(Latency::reader().next_delay() > Latency::intra_dc().next_delay());
    }
}
