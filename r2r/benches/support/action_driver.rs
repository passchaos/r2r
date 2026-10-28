use futures::FutureExt;
use r2r::Node;
use std::{
    future::Future,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    task::{Context, Poll, Wake, Waker},
    time::{Duration, Instant},
};

#[path = "allocation_probe.rs"]
mod allocation_probe;
use allocation_probe::{NativeStats, Probe};

struct WakeCount(AtomicU64);
impl Wake for WakeCount {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}
pub struct Driver {
    probe: Option<Probe>,
    active: bool,
    polls: u64,
    spins: u64,
    wakes: Arc<WakeCount>,
    waker: Waker,
}
impl Driver {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let wakes = Arc::new(WakeCount(AtomicU64::new(0)));
        let waker = Waker::from(wakes.clone());
        Ok(Self {
            probe: Probe::load()?,
            active: false,
            polls: 0,
            spins: 0,
            wakes,
            waker,
        })
    }
    pub fn begin(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(probe) = &self.probe {
            self.polls = 0;
            self.spins = 0;
            self.wakes.0.store(0, Ordering::Relaxed);
            let ret = unsafe { (probe.begin)() };
            if ret != 0 {
                return Err(std::io::Error::from_raw_os_error(ret).into());
            }
            self.active = true;
        }
        Ok(())
    }
    pub fn end(&mut self) -> Result<Option<serde_json::Value>, Box<dyn std::error::Error>> {
        if !self.active {
            return Ok(None);
        }
        self.active = false;
        let mut stats = NativeStats::default();
        let ret = unsafe { (self.probe.as_ref().unwrap().end)(&mut stats) };
        if ret != 0 {
            return Err(std::io::Error::from_raw_os_error(ret).into());
        }
        Ok(Some(serde_json::json!({"abi":1,
            "allocations":{"malloc_calls":stats.malloc_calls,"calloc_calls":stats.calloc_calls,
                "realloc_calls":stats.realloc_calls,"free_calls":stats.free_calls,"null_free_calls":stats.null_free_calls,
                "aligned_calls":stats.aligned_calls,"failed_allocations":stats.failed_allocations,
                "requested_bytes":stats.requested_bytes,"size_overflows":stats.size_overflows,"counter_overflows":stats.counter_overflows},
            "resources":{"user_cpu_ns":stats.user_cpu_ns,"system_cpu_ns":stats.system_cpu_ns,
                "voluntary_context_switches":stats.voluntary_context_switches,"involuntary_context_switches":stats.involuntary_context_switches,
                "process_max_rss_kib":stats.process_max_rss_kib},
            "application":{"future_polls":self.polls,"spin_calls":self.spins,"future_wakes":self.wakes.0.load(Ordering::Relaxed)}})))
    }
    pub fn spin(&mut self, node: &mut Node, timeout: Duration) -> r2r::Result<()> {
        if self.active {
            self.spins += 1;
        }
        node.try_spin_once(timeout)
    }
    pub fn drive<T>(
        &mut self, node: &mut Node, future: impl Future<Output = r2r::Result<T>>,
    ) -> r2r::Result<T> {
        futures::pin_mut!(future);
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let result = if self.active {
                self.polls += 1;
                future.as_mut().poll(&mut Context::from_waker(&self.waker))
            } else {
                match future.as_mut().now_or_never() {
                    Some(value) => Poll::Ready(value),
                    None => Poll::Pending,
                }
            };
            if let Poll::Ready(result) = result {
                return result;
            }
            if Instant::now() >= deadline {
                return Err(r2r::Error::RCL_RET_TIMEOUT);
            }
            self.spin(node, Duration::from_millis(100))?;
        }
    }
}
impl Drop for Driver {
    fn drop(&mut self) {
        if self.active {
            let mut stats = NativeStats::default();
            unsafe {
                (self.probe.as_ref().unwrap().end)(&mut stats);
            }
        }
    }
}
