use std::{collections::BTreeMap, error::Error, fmt};

use crate::AssetGeneration;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryBudget {
    pub cpu_staging_bytes: u64,
    pub gpu_texture_bytes: u64,
    pub disk_cache_bytes: u64,
}

impl MemoryBudget {
    pub fn new(
        cpu_staging_bytes: u64,
        gpu_texture_bytes: u64,
        disk_cache_bytes: u64,
    ) -> Result<Self, CacheError> {
        if cpu_staging_bytes == 0 || gpu_texture_bytes == 0 || disk_cache_bytes == 0 {
            return Err(CacheError::InvalidBudget);
        }
        Ok(Self {
            cpu_staging_bytes,
            gpu_texture_bytes,
            disk_cache_bytes,
        })
    }

    pub const fn baseline_8gb() -> Self {
        Self {
            cpu_staging_bytes: 256 * 1024 * 1024,
            gpu_texture_bytes: 256 * 1024 * 1024,
            disk_cache_bytes: 2 * 1024 * 1024 * 1024,
        }
    }

    pub const fn single_texture_limit_bytes(self) -> u64 {
        self.gpu_texture_bytes / 4
    }

    const fn limit(self, tier: CacheTier) -> u64 {
        match tier {
            CacheTier::CpuStaging => self.cpu_staging_bytes,
            CacheTier::GpuTexture => self.gpu_texture_bytes,
            CacheTier::DiskDerived => self.disk_cache_bytes,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CacheTier {
    CpuStaging,
    GpuTexture,
    DiskDerived,
}

impl CacheTier {
    const fn index(self) -> usize {
        match self {
            Self::CpuStaging => 0,
            Self::GpuTexture => 1,
            Self::DiskDerived => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourcePriority {
    Visible,
    Resident,
    Prefetch,
}

impl ResourcePriority {
    const fn eviction_rank(self) -> u8 {
        match self {
            Self::Prefetch => 0,
            Self::Resident => 1,
            Self::Visible => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PressureLevel {
    Normal,
    Warning,
    Critical,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheEntry<K> {
    pub key: K,
    pub tier: CacheTier,
    pub bytes: u64,
    pub generation: AssetGeneration,
    pub priority: ResourcePriority,
    pub rebuildable: bool,
}

#[derive(Clone, Debug)]
struct StoredEntry<K> {
    entry: CacheEntry<K>,
    last_used: u64,
}

#[derive(Clone, Debug)]
pub struct BudgetedLru<K> {
    budget: MemoryBudget,
    entries: BTreeMap<K, StoredEntry<K>>,
    usage: [u64; 3],
    clock: u64,
}

impl<K: Clone + Ord> BudgetedLru<K> {
    pub fn new(budget: MemoryBudget) -> Self {
        Self {
            budget,
            entries: BTreeMap::new(),
            usage: [0; 3],
            clock: 0,
        }
    }

    pub fn insert(&mut self, entry: CacheEntry<K>) -> Result<Vec<K>, CacheError> {
        let limit = self.budget.limit(entry.tier);
        if entry.bytes > limit {
            return Err(CacheError::EntryExceedsTierBudget {
                tier: entry.tier,
                bytes: entry.bytes,
                limit,
            });
        }

        let replaced_bytes = self
            .entries
            .get(&entry.key)
            .filter(|stored| stored.entry.tier == entry.tier)
            .map_or(0, |stored| stored.entry.bytes);
        let current_without_replaced = self.usage(entry.tier).saturating_sub(replaced_bytes);
        let bytes_to_free = current_without_replaced
            .saturating_add(entry.bytes)
            .saturating_sub(limit);
        let victims = self.eviction_candidates(entry.tier, Some(&entry.key));
        let mut freed = 0_u64;
        let mut selected = Vec::new();
        for key in victims {
            if freed >= bytes_to_free {
                break;
            }
            let stored = &self.entries[&key];
            freed = freed.saturating_add(stored.entry.bytes);
            selected.push(key);
        }
        if freed < bytes_to_free {
            return Err(CacheError::InsufficientEvictableSpace {
                tier: entry.tier,
                required: bytes_to_free,
                available: freed,
            });
        }

        for key in &selected {
            self.remove_entry(key);
        }
        self.remove_entry(&entry.key);
        self.clock = self.clock.saturating_add(1);
        self.usage[entry.tier.index()] = self.usage[entry.tier.index()].saturating_add(entry.bytes);
        self.entries.insert(
            entry.key.clone(),
            StoredEntry {
                entry,
                last_used: self.clock,
            },
        );
        Ok(selected)
    }

    pub fn touch(&mut self, key: &K) -> bool {
        let Some(stored) = self.entries.get_mut(key) else {
            return false;
        };
        self.clock = self.clock.saturating_add(1);
        stored.last_used = self.clock;
        true
    }

    pub fn contains(&self, key: &K) -> bool {
        self.entries.contains_key(key)
    }

    pub fn remove(&mut self, key: &K) -> Option<CacheEntry<K>> {
        self.remove_entry(key)
    }

    pub const fn usage(&self, tier: CacheTier) -> u64 {
        self.usage[tier.index()]
    }

    pub fn apply_pressure(
        &mut self,
        pressure: PressureLevel,
        current_generation: AssetGeneration,
    ) -> Vec<K> {
        let mut victims = self
            .entries
            .iter()
            .filter(|(_, stored)| should_reclaim(&stored.entry, pressure, current_generation))
            .map(|(key, stored)| (stored.last_used, key.clone()))
            .collect::<Vec<_>>();
        victims.sort_by_key(|(last_used, _)| *last_used);
        let victims = victims.into_iter().map(|(_, key)| key).collect::<Vec<_>>();
        for key in &victims {
            self.remove_entry(key);
        }
        victims
    }

    fn eviction_candidates(&self, tier: CacheTier, excluded: Option<&K>) -> Vec<K> {
        let mut candidates = self
            .entries
            .iter()
            .filter(|(key, stored)| {
                stored.entry.tier == tier
                    && stored.entry.priority != ResourcePriority::Visible
                    && stored.entry.rebuildable
                    && excluded != Some(*key)
            })
            .map(|(key, stored)| {
                (
                    stored.entry.priority.eviction_rank(),
                    stored.last_used,
                    key.clone(),
                )
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|(rank, last_used, _)| (*rank, *last_used));
        candidates.into_iter().map(|(_, _, key)| key).collect()
    }

    fn remove_entry(&mut self, key: &K) -> Option<CacheEntry<K>> {
        let stored = self.entries.remove(key)?;
        self.usage[stored.entry.tier.index()] =
            self.usage[stored.entry.tier.index()].saturating_sub(stored.entry.bytes);
        Some(stored.entry)
    }
}

fn should_reclaim<K>(
    entry: &CacheEntry<K>,
    pressure: PressureLevel,
    current_generation: AssetGeneration,
) -> bool {
    if !entry.rebuildable {
        return false;
    }
    match pressure {
        PressureLevel::Normal => entry.priority == ResourcePriority::Prefetch,
        PressureLevel::Warning => {
            entry.priority == ResourcePriority::Prefetch || entry.generation != current_generation
        }
        PressureLevel::Critical => {
            !(entry.generation == current_generation && entry.priority == ResourcePriority::Visible)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheError {
    InvalidBudget,
    EntryExceedsTierBudget {
        tier: CacheTier,
        bytes: u64,
        limit: u64,
    },
    InsufficientEvictableSpace {
        tier: CacheTier,
        required: u64,
        available: u64,
    },
}

impl fmt::Display for CacheError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBudget => formatter.write_str("cache tier budgets must be positive"),
            Self::EntryExceedsTierBudget { tier, bytes, limit } => {
                write!(
                    formatter,
                    "{tier:?} entry of {bytes} bytes exceeds {limit} byte budget"
                )
            }
            Self::InsufficientEvictableSpace {
                tier,
                required,
                available,
            } => write!(
                formatter,
                "{tier:?} needs {required} reclaimable bytes but only {available} are available"
            ),
        }
    }
}

impl Error for CacheError {}
