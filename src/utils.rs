pub fn S(st: &str) -> String {
  st.to_string()
}

#[derive(Clone, Debug)]
pub struct KeyValue(pub String, pub String);

impl KeyValue {
  pub fn new() -> KeyValue {
    KeyValue {
      0: "".to_string(),
      1: "".to_string(),
    }
  }

  pub fn new_values(key: &str, value: &str) -> KeyValue {
    KeyValue {
      0: key.to_string(),
      1: value.to_string(),
    }
  }
}

use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct Debouncer<T> {
  delay: Duration,
  last_event: Option<Instant>,
  pending: Option<T>,
}

impl<T> Debouncer<T> {
  pub fn new(delay: Duration) -> Debouncer<T> {
    Debouncer {
      delay,
      last_event: None,
      pending: None,
    }
  }

  pub fn push(&mut self, value: T) {
    self.push_at(value, Instant::now());
  }

  pub fn push_at(&mut self, value: T, now: Instant) {
    self.pending = Some(value);
    self.last_event = Some(now);
  }

  pub fn poll(&mut self) -> Option<T> {
    self.poll_at(Instant::now())
  }

  pub fn poll_at(&mut self, now: Instant) -> Option<T> {
    if let (Some(last), Some(_)) = (self.last_event, self.pending.as_ref()) {
      if now.duration_since(last) >= self.delay {
        self.last_event = None;
        return self.pending.take();
      }
    }
    None
  }

  pub fn is_pending(&self) -> bool {
    self.last_event.is_some() && self.pending.is_some()
  }
}

#[cfg(test)]
mod tests {
  use super::Debouncer;
  use std::time::{Duration, Instant};

  #[test]
  fn debouncer_waits_for_delay() {
    let mut debouncer = Debouncer::new(Duration::from_millis(50));
    let start = Instant::now();
    debouncer.push_at(1, start);

    assert_eq!(debouncer.poll_at(start + Duration::from_millis(10)), None);
  }

  #[test]
  fn debouncer_pending_clears_after_emit() {
    let mut debouncer = Debouncer::new(Duration::from_millis(50));
    let start = Instant::now();
    debouncer.push_at(1, start);

    assert!(debouncer.is_pending());
    assert_eq!(debouncer.poll_at(start + Duration::from_millis(60)), Some(1));
    assert!(!debouncer.is_pending());
  }

  #[test]
  fn debouncer_emits_latest_after_delay() {
    let mut debouncer = Debouncer::new(Duration::from_millis(50));
    let start = Instant::now();
    debouncer.push_at(1, start);
    debouncer.push_at(2, start + Duration::from_millis(20));

    assert_eq!(debouncer.poll_at(start + Duration::from_millis(69)), None);
    assert_eq!(debouncer.poll_at(start + Duration::from_millis(70)), Some(2));
  }
}
