use crate::{
    AllocationClass, AssetGeneration, ImageMemoryCoordinator, MemoryAdmissionError, MemoryLease,
};
use std::{ops::Deref, sync::Arc};

/// Shared immutable storage; only a unique owner can initialize its bytes.
#[derive(Clone, Debug)]
pub struct SharedPixels(Arc<PixelStorage>);

#[derive(Debug)]
struct PixelStorage {
    // Drop the allocation before its accounting owner. Moving this Vec into
    // an Arc does not allocate/copy a second pixel buffer.
    bytes: Vec<u8>,
    _lease: MemoryLease,
}

impl SharedPixels {
    pub fn is_accounted_by(&self, memory: &ImageMemoryCoordinator) -> bool {
        self.0._lease.is_accounted_by(memory)
    }
    pub fn try_zeroed(
        memory: &ImageMemoryCoordinator,
        generation: AssetGeneration,
        bytes: u64,
    ) -> Result<Self, MemoryAdmissionError> {
        let lease = memory.try_reserve(AllocationClass::DecodedPixels, bytes, generation)?;
        let mut storage = Vec::new();
        let len = usize::try_from(bytes).map_err(|_| MemoryAdmissionError::AccountingOverflow)?;
        storage
            .try_reserve_exact(len)
            .map_err(|_| MemoryAdmissionError::AllocationFailed)?;
        storage.resize(len, 0);
        lease.commit()?;
        Ok(Self(Arc::new(PixelStorage {
            bytes: storage,
            _lease: lease,
        })))
    }
    pub fn try_copy_from_slice(
        memory: &ImageMemoryCoordinator,
        generation: AssetGeneration,
        bytes: &[u8],
    ) -> Result<Self, MemoryAdmissionError> {
        let mut storage = Self::try_zeroed(memory, generation, bytes.len() as u64)?;
        storage
            .get_mut()
            .expect("new storage is unique")
            .copy_from_slice(bytes);
        Ok(storage)
    }
    pub fn get_mut(&mut self) -> Option<&mut [u8]> {
        Arc::get_mut(&mut self.0).map(|storage| storage.bytes.as_mut_slice())
    }
}
impl Deref for SharedPixels {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.0.bytes
    }
}
impl AsRef<[u8]> for SharedPixels {
    fn as_ref(&self) -> &[u8] {
        self
    }
}
impl PartialEq for SharedPixels {
    fn eq(&self, other: &Self) -> bool {
        self.as_ref() == other.as_ref()
    }
}
impl Eq for SharedPixels {}
