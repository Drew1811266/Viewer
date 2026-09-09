use std::{
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use crate::RendererInitError;

#[derive(Default)]
struct State {
    live_devices: usize,
    active: bool,
    closed: bool,
    jobs: Vec<Job>,
    progress: Option<Job>,
    completion_wake: Option<Arc<dyn Fn() + Send + Sync>>,
}
struct Control {
    state: Mutex<State>,
    changed: Condvar,
}
struct Owners(Arc<Control>);
impl Drop for Owners {
    fn drop(&mut self) {
        self.0.state.lock().unwrap().closed = true;
        self.0.changed.notify_all();
    }
}

/// One driver-owned worker, at most two not-yet-completed devices, and at most
/// one construction/active-device permit. Never blocks an actor on the GPU.
#[derive(Clone)]
pub struct GpuRetirementService {
    control: Arc<Control>,
    _owners: Arc<Owners>,
}

struct Job {
    device: Option<wgpu::Device>,
    _queue: Option<wgpu::Queue>,
    completed: Arc<AtomicBool>,
}

impl Default for GpuRetirementService {
    fn default() -> Self {
        Self::new()
    }
}
impl GpuRetirementService {
    pub fn new() -> Self {
        let control = Arc::new(Control {
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
        });
        let worker = control.clone();
        let started = std::thread::Builder::new()
            .name("viewer-gpu-retirement".into())
            .spawn(move || {
                loop {
                    let (mut jobs, progress) = {
                        let mut state = worker.state.lock().unwrap();
                        while state.jobs.is_empty() && state.progress.is_none() && !state.closed {
                            state = worker.changed.wait(state).unwrap();
                        }
                        if state.closed && state.jobs.is_empty() && state.progress.is_none() {
                            return;
                        }
                        let progress = state.progress.as_ref().and_then(|job| job.device.clone());
                        (std::mem::take(&mut state.jobs), progress)
                    };
                    if let Some(device) = progress {
                        let _ = device.poll(wgpu::PollType::Poll);
                    }
                    for job in &jobs {
                        if let Some(device) = &job.device {
                            // Failure/elapsed time is NOT completion and must not
                            // falsify live resource accounting.
                            let _ = device.poll(wgpu::PollType::Poll);
                        }
                    }
                    let mut state = worker.state.lock().unwrap();
                    let count = jobs.len();
                    jobs.retain(|job| !job.completed.load(Ordering::Acquire));
                    let completed_count = count - jobs.len();
                    state.live_devices -= completed_count;
                    state.jobs.extend(jobs);
                    let completion_wake = (completed_count != 0)
                        .then(|| state.completion_wake.clone())
                        .flatten();
                    if state
                        .progress
                        .as_ref()
                        .is_some_and(|job| job.completed.load(Ordering::Acquire))
                    {
                        state.progress = None;
                    }
                    if let Some(wake) = completion_wake {
                        drop(state);
                        wake();
                        state = worker.state.lock().unwrap();
                    }
                    if !state.jobs.is_empty() || state.progress.is_some() {
                        drop(
                            worker
                                .changed
                                .wait_timeout(state, Duration::from_millis(16))
                                .unwrap(),
                        );
                    }
                }
            });
        if started.is_err() {
            control.state.lock().unwrap().closed = true;
        }
        Self {
            _owners: Arc::new(Owners(control.clone())),
            control,
        }
    }

    pub(crate) fn acquire(&self) -> Result<DevicePermit, RendererInitError> {
        let mut state = self.control.state.lock().unwrap();
        if state.closed || state.active || state.live_devices >= 2 {
            return Err(RendererInitError::RetirementBacklog);
        }
        state.active = true;
        state.live_devices += 1;
        Ok(DevicePermit {
            service: self.clone(),
            released: false,
        })
    }

    pub fn live_devices(&self) -> usize {
        self.control.state.lock().unwrap().live_devices
    }
}

pub(crate) struct DevicePermit {
    service: GpuRetirementService,
    released: bool,
}
impl DevicePermit {
    pub(crate) fn set_completion_wake(&self, wake: Arc<dyn Fn() + Send + Sync>) {
        self.service.control.state.lock().unwrap().completion_wake = Some(wake);
    }
    pub(crate) fn request_progress(&self, device: wgpu::Device, completed: Arc<AtomicBool>) {
        let mut state = self.service.control.state.lock().unwrap();
        // One active device, one outstanding progress request; this is not a
        // second queue of device lifetimes and does not consume another permit.
        state.progress = Some(Job {
            device: Some(device),
            _queue: None,
            completed,
        });
        self.service.control.changed.notify_all();
    }
    pub(crate) fn retire(
        mut self,
        device: wgpu::Device,
        queue: wgpu::Queue,
        completed: Arc<AtomicBool>,
    ) {
        self.retire_job(Job {
            device: Some(device),
            _queue: Some(queue),
            completed,
        });
    }
    fn retire_job(&mut self, job: Job) {
        let mut state = self.service.control.state.lock().unwrap();
        state.active = false;
        state.progress = None; // the retirement job now drives this device
        state.jobs.push(job);
        self.released = true;
        self.service.control.changed.notify_all();
    }
}
impl Drop for DevicePermit {
    fn drop(&mut self) {
        if !self.released {
            let mut state = self.service.control.state.lock().unwrap();
            state.active = false;
            state.live_devices -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_device_completion_wakes_latest_actor_once_not_every_poll() {
        use std::sync::atomic::AtomicUsize;
        let service = GpuRetirementService::new();
        let mut old = service.acquire().unwrap();
        let completed = Arc::new(AtomicBool::new(false));
        let old_wakes = Arc::new(AtomicUsize::new(0));
        let counter = old_wakes.clone();
        old.set_completion_wake(Arc::new(move || {
            counter.fetch_add(1, Ordering::Relaxed);
        }));
        old.retire_job(Job {
            device: None,
            _queue: None,
            completed: completed.clone(),
        });
        let current = service.acquire().unwrap();
        let current_wakes = Arc::new(AtomicUsize::new(0));
        let counter = current_wakes.clone();
        current.set_completion_wake(Arc::new(move || {
            counter.fetch_add(1, Ordering::Relaxed);
        }));
        std::thread::sleep(Duration::from_millis(40));
        assert_eq!(current_wakes.load(Ordering::Relaxed), 0);
        completed.store(true, Ordering::Release);
        service.control.changed.notify_all();
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while current_wakes.load(Ordering::Relaxed) == 0 {
            assert!(
                std::time::Instant::now() < deadline,
                "old device relief must wake the new actor"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        std::thread::sleep(Duration::from_millis(40));
        assert_eq!(current_wakes.load(Ordering::Relaxed), 1);
        assert_eq!(old_wakes.load(Ordering::Relaxed), 0);
    }
    #[test]
    fn stalled_completion_bounds_recreation_before_device_creation() {
        let service = GpuRetirementService::new();
        let stalled = Arc::new(AtomicBool::new(false));
        let mut first = service.acquire().unwrap();
        first.retire_job(Job {
            device: None,
            _queue: None,
            completed: stalled.clone(),
        });
        let mut second = service.acquire().unwrap();
        second.retire_job(Job {
            device: None,
            _queue: None,
            completed: stalled.clone(),
        });
        for _ in 0..100 {
            assert!(service.acquire().is_err());
        }
        assert_eq!(service.live_devices(), 2);
        stalled.store(true, Ordering::Release);
        service.control.changed.notify_all();
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while service.live_devices() != 0 {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(service.acquire().is_ok());
    }
}
