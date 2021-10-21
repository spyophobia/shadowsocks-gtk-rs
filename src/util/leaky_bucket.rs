//! This module contains an implementation of the leaky bucket
//! rate limiter algorithm.

use std::{
    fmt::Display,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct LeakyBucketOverflowed {
    times: usize,
    within: Duration,
    history: Vec<Instant>,
}
impl Display for LeakyBucketOverflowed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "NaiveLeakyBucket overflowed: more than {} times within {:?}",
            self.times, self.within
        )
    }
}
impl From<&NaiveLeakyBucket> for LeakyBucketOverflowed {
    fn from(lb: &NaiveLeakyBucket) -> Self {
        let NaiveLeakyBucket { times, within, history } = lb.clone();
        Self { times, within, history }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct NaiveLeakyBucketConfig {
    times: usize,
    within: Duration,
}

impl NaiveLeakyBucketConfig {
    pub fn new(times: usize, within: Duration) -> Self {
        Self { times, within }
    }
}

#[derive(Debug, Clone)]
pub struct NaiveLeakyBucket {
    times: usize,
    within: Duration,
    history: Vec<Instant>,
}

impl From<NaiveLeakyBucketConfig> for NaiveLeakyBucket {
    fn from(NaiveLeakyBucketConfig { times, within }: NaiveLeakyBucketConfig) -> Self {
        Self {
            times,
            within,
            history: vec![],
        }
    }
}

impl NaiveLeakyBucket {
    pub fn push(&mut self) -> Result<(), LeakyBucketOverflowed> {
        let now = Instant::now();
        self.history.push(now);
        self.history = self
            .history
            .iter()
            .filter(|&&t| now.saturating_duration_since(t) < self.within)
            .map(|&t| t)
            .collect();
        match self.history.len() <= self.times {
            true => Ok(()),
            false => Err((self as &Self).into()),
        }
    }
}

#[cfg(test)]
mod test {
    use std::{thread::sleep, time::Duration};

    use super::{NaiveLeakyBucket, NaiveLeakyBucketConfig};

    #[test]
    fn size_0() {
        let mut lb: NaiveLeakyBucket = NaiveLeakyBucketConfig::new(0, Duration::from_secs(1)).into();
        assert!(lb.push().is_err())
    }
    #[test]
    fn size_3() {
        let mut lb: NaiveLeakyBucket = NaiveLeakyBucketConfig::new(3, Duration::from_secs(10)).into();
        for _ in 0..3 {
            assert!(lb.push().is_ok());
        }
        assert!(lb.push().is_err());
    }
    #[test]
    fn expire_1() {
        let mut lb: NaiveLeakyBucket = NaiveLeakyBucketConfig::new(2, Duration::from_millis(100)).into();
        assert!(lb.push().is_ok()); // len: 0
        sleep(Duration::from_millis(40));
        assert!(lb.push().is_ok()); // len: 1
        sleep(Duration::from_millis(40));
        assert!(lb.push().is_err()); // len 2
        sleep(Duration::from_millis(80)); // expire 1
        assert!(lb.push().is_ok()); // len 1
    }
}
