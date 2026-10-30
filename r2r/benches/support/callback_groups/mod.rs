use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    Condvar, Mutex,
};
use std::time::{Duration, Instant};

#[inline(never)]
pub fn work(sequence: usize, operations: usize) -> i64 {
    let mut value = (sequence as u64).wrapping_add(0x9e3779b97f4a7c15);
    for step in 0..operations {
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        value = value.wrapping_add(step as u64 + 0x517cc1b727220a95);
    }
    (value & i64::MAX as u64) as i64
}

#[derive(Default)]
pub struct Row {
    pub seen: AtomicUsize,
    pub native: AtomicU64,
    pub enter: AtomicU64,
    pub finish: AtomicU64,
}
pub struct State {
    pub origin: Instant,
    pub rows: Vec<Row>,
    pub active: AtomicUsize,
    pub peak: AtomicUsize,
    pub probes: AtomicUsize,
    pub barrier: (Mutex<bool>, Condvar),
    pub limit: usize,
    pub operations: usize,
}
impl State {
    pub fn now(&self) -> u64 {
        self.origin.elapsed().as_nanos() as u64
    }
    pub fn invoke(&self, a: i64, b: i64, native: i64) -> i64 {
        let active = self.active.fetch_add(1, Ordering::AcqRel) + 1;
        assert!(active <= self.limit, "callback group exceeded its concurrency bound");
        self.peak.fetch_max(active, Ordering::Relaxed);
        let _active = Active(self);
        if a < 0 {
            assert!(matches!(a, -3 | -4) && b == 0);
            if self.probes.fetch_add(1, Ordering::AcqRel) == 1 {
                self.release();
            }
            let (lock, ready) = &self.barrier;
            let mut released = lock.lock().unwrap();
            while !*released {
                let (next, timeout) = ready
                    .wait_timeout(released, Duration::from_secs(5))
                    .unwrap();
                released = next;
                assert!(!timeout.timed_out(), "same-source overlap challenge timed out");
            }
            return a;
        }
        assert_eq!(b, self.operations as i64);
        let index = usize::try_from(a).unwrap();
        let row = self.rows.get(index).expect("invalid request sequence");
        assert_eq!(row.seen.swap(1, Ordering::AcqRel), 0, "duplicate request");
        row.native.store(native as u64, Ordering::Relaxed);
        row.enter.store(self.now(), Ordering::Release);
        let result = work(index, self.operations);
        row.finish.store(self.now(), Ordering::Release);
        result
    }
    pub fn release(&self) {
        let (lock, ready) = &self.barrier;
        *lock.lock().unwrap() = true;
        ready.notify_all();
    }
}
struct Active<'a>(&'a State);
impl Drop for Active<'_> {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::Release);
    }
}

pub fn cpu_ns() -> u64 {
    let mut value = std::mem::MaybeUninit::<libc::timespec>::uninit();
    assert_eq!(
        unsafe { libc::clock_gettime(libc::CLOCK_PROCESS_CPUTIME_ID, value.as_mut_ptr()) },
        0
    );
    let value = unsafe { value.assume_init() };
    value.tv_sec as u64 * 1_000_000_000 + value.tv_nsec as u64
}

pub fn process_metrics() -> (usize, u64, u64) {
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    let (mut threads, mut rss, mut hwm) = (0, 0, 0);
    for line in status.lines() {
        let mut fields = line.split_whitespace();
        match fields.next() {
            Some("Threads:") => threads = fields.next().unwrap().parse().unwrap(),
            Some("VmRSS:") => rss = fields.next().unwrap().parse().unwrap(),
            Some("VmHWM:") => hwm = fields.next().unwrap().parse().unwrap(),
            _ => {}
        }
    }
    assert!(threads > 0 && rss > 0 && hwm > 0, "Linux process metrics missing");
    (threads, rss, hwm.max(rss))
}
