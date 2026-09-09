//! Accounted application payloads, not wgpu/Metal physical residency.
use crate::retirement::DevicePermit;
use std::{
    ops::Deref,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
use viewer_render_core::{
    AllocationClass, AssetGeneration, ImageMemoryCoordinator, MemoryAdmissionError, MemoryLease,
};

type Retired = Box<dyn Send>;
#[derive(Default)]
struct Pending {
    owners: Vec<Retired>,
    write_bytes: u64,
}
struct Inner {
    device: wgpu::Device,
    queue: wgpu::Queue,
    memory: ImageMemoryCoordinator,
    pending: Mutex<Pending>,
    permit: Option<DevicePermit>,
    progress_pending: Arc<AtomicBool>,
    retiring_batches: Arc<AtomicUsize>,
}
#[derive(Clone)]
pub(crate) struct GpuMemory(Arc<Inner>);
struct Allocation<T: Send + 'static> {
    raw: Option<T>,
    lease: Option<MemoryLease>,
    owner: GpuMemory,
}
struct Retiring<T> {
    _raw: T,
    _lease: MemoryLease,
}
impl<T: Send + 'static> Drop for Allocation<T> {
    fn drop(&mut self) {
        let lease = self.lease.take().unwrap();
        let _ = lease.retire();
        let retired = Retiring {
            _raw: self.raw.take().unwrap(),
            _lease: lease,
        };
        self.owner
            .0
            .pending
            .lock()
            .unwrap()
            .owners
            .push(Box::new(retired));
    }
}
#[derive(Clone)]
pub(crate) struct GpuBuffer(Arc<Allocation<wgpu::Buffer>>);
impl Deref for GpuBuffer {
    type Target = wgpu::Buffer;
    fn deref(&self) -> &Self::Target {
        self.0.raw.as_ref().unwrap()
    }
}
#[derive(Clone)]
pub(crate) struct GpuTexture(Arc<Allocation<wgpu::Texture>>);
impl Deref for GpuTexture {
    type Target = wgpu::Texture;
    fn deref(&self) -> &Self::Target {
        self.0.raw.as_ref().unwrap()
    }
}

pub(crate) struct BufferWrite<'a> {
    pub(crate) buffer: &'a wgpu::Buffer,
    pub(crate) offset: u64,
    pub(crate) data: &'a [u8],
}

impl GpuMemory {
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        memory: ImageMemoryCoordinator,
        permit: DevicePermit,
    ) -> Self {
        Self(Arc::new(Inner {
            device: device.clone(),
            queue: queue.clone(),
            memory,
            pending: Mutex::default(),
            permit: Some(permit),
            progress_pending: Arc::new(AtomicBool::new(false)),
            retiring_batches: Arc::new(AtomicUsize::new(0)),
        }))
    }
    pub(crate) fn coordinator(&self) -> &ImageMemoryCoordinator {
        &self.0.memory
    }
    pub(crate) fn buffer(
        &self,
        descriptor: &wgpu::BufferDescriptor<'_>,
        class: AllocationClass,
    ) -> Result<GpuBuffer, MemoryAdmissionError> {
        let lease = self
            .0
            .memory
            .try_reserve(class, descriptor.size, AssetGeneration(0))?;
        let raw = self.0.device.create_buffer(descriptor);
        lease.commit()?;
        Ok(GpuBuffer(Arc::new(Allocation {
            raw: Some(raw),
            lease: Some(lease),
            owner: self.clone(),
        })))
    }
    pub(crate) fn buffer_init(
        &self,
        label: &'static str,
        bytes: &[u8],
        usage: wgpu::BufferUsages,
    ) -> Result<GpuBuffer, MemoryAdmissionError> {
        let size = (bytes.len() as u64)
            .checked_add(3)
            .ok_or(MemoryAdmissionError::AccountingOverflow)?
            / 4
            * 4;
        let buffer = self.buffer(
            &wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: true,
            },
            AllocationClass::RendererBuffers,
        )?;
        buffer
            .slice(..)
            .get_mapped_range_mut()
            .map_err(|_| MemoryAdmissionError::AllocationFailed)?
            .slice(..bytes.len())
            .copy_from_slice(bytes);
        buffer.unmap();
        Ok(buffer)
    }
    pub(crate) fn texture(
        &self,
        descriptor: &wgpu::TextureDescriptor<'_>,
        bytes: u64,
        generation: AssetGeneration,
    ) -> Result<GpuTexture, MemoryAdmissionError> {
        let lease = self
            .0
            .memory
            .try_reserve(AllocationClass::GpuTexture, bytes, generation)?;
        let raw = self.0.device.create_texture(descriptor);
        lease.commit()?;
        Ok(GpuTexture(Arc::new(Allocation {
            raw: Some(raw),
            lease: Some(lease),
            owner: self.clone(),
        })))
    }
    /// Queue-managed upload payload allowance. This is not a measurement of
    /// opaque queue staging and is separate from the three mapped image slots.
    pub(crate) fn write_buffer(
        &self,
        buffer: &wgpu::Buffer,
        offset: u64,
        data: &[u8],
    ) -> Result<(), MemoryAdmissionError> {
        self.write_buffers(&[BufferWrite {
            buffer,
            offset,
            data,
        }])
    }

    /// Admit a group of queue writes as one operation. Queue writes themselves
    /// cannot fail after admission, so reserving the aggregate first prevents
    /// a temporary pressure refusal halfway through a multi-buffer update.
    pub(crate) fn write_buffers(
        &self,
        writes: &[BufferWrite<'_>],
    ) -> Result<(), MemoryAdmissionError> {
        let total = writes.iter().try_fold(0_u64, |total, write| {
            let bytes = (write.data.len() as u64)
                .checked_add(3)
                .ok_or(MemoryAdmissionError::AccountingOverflow)?
                / 4
                * 4;
            total
                .checked_add(bytes)
                .ok_or(MemoryAdmissionError::AccountingOverflow)
        })?;
        if total == 0 {
            return Ok(());
        }
        // Bound unsubmitted allowances; submitting never waits for the GPU.
        if self
            .0
            .pending
            .lock()
            .unwrap()
            .write_bytes
            .saturating_add(total)
            > 1 << 20
        {
            self.submit([]);
        }
        let lease =
            self.0
                .memory
                .try_reserve(AllocationClass::UploadStaging, total, AssetGeneration(0))?;
        for write in writes.iter().filter(|write| !write.data.is_empty()) {
            self.0
                .queue
                .write_buffer(write.buffer, write.offset, write.data);
        }
        lease.commit()?;
        lease.retire()?;
        let mut pending = self.0.pending.lock().unwrap();
        pending.write_bytes += total;
        pending.owners.push(Box::new(lease));
        Ok(())
    }
    pub(crate) fn submit(
        &self,
        commands: impl IntoIterator<Item = wgpu::CommandBuffer>,
    ) -> wgpu::SubmissionIndex {
        let index = self.0.queue.submit(commands);
        let owners = std::mem::take(&mut *self.0.pending.lock().unwrap());
        if !owners.owners.is_empty() {
            let batches = self.0.retiring_batches.clone();
            batches.fetch_add(1, Ordering::Release);
            self.0.queue.on_submitted_work_done(move || {
                drop(owners);
                batches.fetch_sub(1, Ordering::Release);
            });
        }
        index
    }
    pub(crate) fn flush(&self) {
        if !self.0.pending.lock().unwrap().owners.is_empty() {
            self.submit([]);
        }
    }

    pub(crate) fn wake_when_progress(&self, wake: Arc<dyn Fn() + Send + Sync>) {
        self.flush();
        if self.0.progress_pending.swap(true, Ordering::AcqRel) {
            return;
        }
        let completed = Arc::new(AtomicBool::new(false));
        let callback = completed.clone();
        let pending = self.0.progress_pending.clone();
        self.0.queue.on_submitted_work_done(move || {
            callback.store(true, Ordering::Release);
            pending.store(false, Ordering::Release);
            wake();
        });
        self.0
            .permit
            .as_ref()
            .unwrap()
            .request_progress(self.0.device.clone(), completed);
    }

    pub(crate) fn has_retirement_work(&self) -> bool {
        self.0.retiring_batches.load(Ordering::Acquire) != 0
            || !self.0.pending.lock().unwrap().owners.is_empty()
    }
    pub(crate) fn set_completion_wake(&self, wake: Arc<dyn Fn() + Send + Sync>) {
        self.0.permit.as_ref().unwrap().set_completion_wake(wake);
    }
}
impl Drop for Inner {
    fn drop(&mut self) {
        let owners = std::mem::take(self.pending.get_mut().unwrap());
        self.queue.submit([]);
        let completed = Arc::new(AtomicBool::new(false));
        let callback = completed.clone();
        self.queue.on_submitted_work_done(move || {
            drop(owners);
            callback.store(true, Ordering::Release);
        });
        self.permit
            .take()
            .unwrap()
            .retire(self.device.clone(), self.queue.clone(), completed);
    }
}
