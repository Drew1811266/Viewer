use crate::{
    UploadError,
    gpu_memory::{GpuBuffer, GpuMemory},
};
use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};
use viewer_render_core::{AllocationClass, MemoryAdmissionError};

pub(crate) const SLOT_BYTES: u64 = 1 << 20;
pub(crate) const SLOT_COUNT: usize = 3;
struct Slot {
    buffer: GpuBuffer,
    state: Arc<AtomicU8>,
}
pub(crate) struct UploadPool {
    slots: [Slot; SLOT_COUNT],
    memory: GpuMemory,
}
impl UploadPool {
    pub(crate) fn new(memory: &GpuMemory) -> Result<Self, MemoryAdmissionError> {
        let mut slots = Vec::with_capacity(SLOT_COUNT);
        for _ in 0..SLOT_COUNT {
            slots.push(Slot {
                buffer: memory.buffer(
                    &wgpu::BufferDescriptor {
                        label: Some("Viewer reused 1 MiB image upload slot"),
                        size: SLOT_BYTES,
                        usage: wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC,
                        mapped_at_creation: true,
                    },
                    AllocationClass::UploadStaging,
                )?,
                state: Arc::new(AtomicU8::new(1)),
            });
        }
        Ok(Self {
            slots: slots.try_into().ok().unwrap(),
            memory: memory.clone(),
        })
    }
    pub(crate) fn padded_row(width: u32, bytes_per_pixel: u32) -> Result<u32, UploadError> {
        let row = width
            .checked_mul(bytes_per_pixel)
            .and_then(|n| n.checked_add(255))
            .ok_or(UploadError::SizeOverflow)?
            / 256
            * 256;
        if row == 0 {
            return Err(UploadError::ZeroDimensions);
        }
        if u64::from(row) > SLOT_BYTES {
            return Err(UploadError::RowExceedsSlot);
        }
        Ok(row)
    }
    /// Returns rows copied and submitted in queue order, without waiting. A
    /// resource is drawable only after the caller has submitted ALL its rows.
    pub(crate) fn copy_rows(
        &mut self,
        device: &wgpu::Device,
        texture: &wgpu::Texture,
        pixels: &[u8],
        dimensions: (u32, u32),
        bytes_per_pixel: u32,
        first_row: u32,
    ) -> Result<u32, UploadError> {
        let (width, height) = dimensions;
        let padded = Self::padded_row(width, bytes_per_pixel)?;
        let Some(slot) = self
            .slots
            .iter_mut()
            .find(|slot| slot.state.load(Ordering::Acquire) != 0)
        else {
            return Ok(0);
        };
        if slot.state.load(Ordering::Acquire) == 2 {
            return Err(UploadError::StagingMapFailed);
        }
        let rows = (SLOT_BYTES / u64::from(padded)).min(u64::from(height - first_row)) as u32;
        let source_row = width as usize * bytes_per_pixel as usize;
        {
            let mut mapped = slot
                .buffer
                .slice(..)
                .get_mapped_range_mut()
                .map_err(|_| UploadError::StagingMapFailed)?;
            for row in 0..rows as usize {
                let source = (first_row as usize + row) * source_row;
                let target = row * padded as usize;
                mapped
                    .slice(target..target + source_row)
                    .copy_from_slice(&pixels[source..source + source_row]);
                mapped
                    .slice(target + source_row..target + padded as usize)
                    .fill(0);
            }
        }
        slot.buffer.unmap();
        slot.state.store(0, Ordering::Release);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Viewer image row band"),
        });
        encoder.copy_buffer_to_texture(
            wgpu::TexelCopyBufferInfo {
                buffer: &slot.buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(rows),
                },
            },
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: first_row,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width,
                height: rows,
                depth_or_array_layers: 1,
            },
        );
        let state = slot.state.clone();
        encoder.map_buffer_on_submit(&slot.buffer, wgpu::MapMode::Write, .., move |result| {
            state.store(if result.is_ok() { 1 } else { 2 }, Ordering::Release);
        });
        self.memory.submit([encoder.finish()]);
        Ok(rows)
    }
}
