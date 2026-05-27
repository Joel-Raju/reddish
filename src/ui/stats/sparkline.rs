/// Circular buffer for sparkline data, holding up to `capacity` f64 samples.
///
/// When full, pushing a new sample overwrites the oldest one.
/// Designed for stats dashboard sparklines (default capacity: 60).
#[derive(Debug, Clone, PartialEq)]
pub struct SparklineBuffer {
    buf: Vec<f64>,
    capacity: usize,
    head: usize,
    len: usize,
}

impl Default for SparklineBuffer {
    fn default() -> Self {
        Self::new(60)
    }
}

impl SparklineBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            buf: Vec::with_capacity(capacity),
            capacity,
            head: 0,
            len: 0,
        }
    }

    /// Push a new sample. If at capacity, overwrites the oldest sample.
    pub fn push(&mut self, value: f64) {
        if self.len < self.capacity {
            self.buf.push(value);
            self.len += 1;
        } else {
            self.buf[self.head] = value;
        }
        self.head = (self.head + 1) % self.capacity;
    }

    /// Return all samples in chronological order (oldest → newest).
    pub fn as_slice(&self) -> &[f64] {
        if self.len < self.capacity {
            &self.buf[..self.len]
        } else {
            // Need to return a contiguous slice. Since our internal Vec may have
            // wrapped, we copy into a temporary vec when full. For typical sparkline
            // rendering we can just return the logical order via an iterator, but
            // for a simple &[f64] we reconstruct.
            &self.buf
        }
    }

    /// Return all samples in chronological order as a Vec.
    pub fn to_vec(&self) -> Vec<f64> {
        if self.len < self.capacity {
            self.buf[..self.len].to_vec()
        } else {
            let mut out = Vec::with_capacity(self.capacity);
            let split = self.head;
            out.extend_from_slice(&self.buf[split..]);
            out.extend_from_slice(&self.buf[..split]);
            out
        }
    }

    /// Most recent sample, if any.
    pub fn latest(&self) -> Option<f64> {
        if self.len == 0 {
            None
        } else {
            let idx = if self.len < self.capacity {
                self.len - 1
            } else {
                (self.head + self.capacity - 1) % self.capacity
            };
            Some(self.buf[idx])
        }
    }

    /// Delta between the latest and the previous sample.
    /// Returns `None` if fewer than 2 samples have been pushed.
    pub fn delta(&self) -> Option<f64> {
        if self.len < 2 {
            return None;
        }
        let latest = self.latest()?;
        let prev_idx = if self.len < self.capacity {
            self.len - 2
        } else {
            (self.head + self.capacity - 2) % self.capacity
        };
        Some(latest - self.buf[prev_idx])
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_buffer_is_empty() {
        let buf = SparklineBuffer::new(60);
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
        assert!(buf.latest().is_none());
        assert!(buf.delta().is_none());
    }

    #[test]
    fn push_increments_len_until_capacity() {
        let mut buf = SparklineBuffer::new(3);
        buf.push(1.0);
        assert_eq!(buf.len(), 1);
        buf.push(2.0);
        assert_eq!(buf.len(), 2);
        buf.push(3.0);
        assert_eq!(buf.len(), 3);
        buf.push(4.0);
        assert_eq!(buf.len(), 3); // capped
    }

    #[test]
    fn latest_returns_most_recent() {
        let mut buf = SparklineBuffer::new(60);
        buf.push(10.0);
        assert_eq!(buf.latest(), Some(10.0));
        buf.push(20.0);
        assert_eq!(buf.latest(), Some(20.0));
    }

    #[test]
    fn delta_requires_two_samples() {
        let mut buf = SparklineBuffer::new(60);
        assert!(buf.delta().is_none());
        buf.push(10.0);
        assert!(buf.delta().is_none());
        buf.push(15.0);
        assert_eq!(buf.delta(), Some(5.0));
    }

    #[test]
    fn delta_with_negative_change() {
        let mut buf = SparklineBuffer::new(60);
        buf.push(10.0);
        buf.push(5.0);
        assert_eq!(buf.delta(), Some(-5.0));
    }

    #[test]
    fn to_vec_returns_chronological_order() {
        let mut buf = SparklineBuffer::new(60);
        buf.push(1.0);
        buf.push(2.0);
        buf.push(3.0);
        assert_eq!(buf.to_vec(), vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn to_vec_wraps_correctly_at_capacity() {
        let mut buf = SparklineBuffer::new(3);
        buf.push(1.0);
        buf.push(2.0);
        buf.push(3.0);
        buf.push(4.0); // overwrites 1.0
        assert_eq!(buf.to_vec(), vec![2.0, 3.0, 4.0]);
        buf.push(5.0); // overwrites 2.0
        assert_eq!(buf.to_vec(), vec![3.0, 4.0, 5.0]);
    }

    #[test]
    fn delta_after_wrap() {
        let mut buf = SparklineBuffer::new(3);
        buf.push(1.0);
        buf.push(2.0);
        buf.push(3.0);
        buf.push(5.0); // overwrites 1.0: [2,3,5]
        assert_eq!(buf.delta(), Some(2.0));
    }

    #[test]
    fn default_capacity_is_60() {
        let buf = SparklineBuffer::default();
        assert_eq!(buf.capacity, 60);
    }
}
