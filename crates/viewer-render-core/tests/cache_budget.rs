use viewer_render_core::{
    AssetGeneration, BudgetedLru, CacheEntry, CacheError, CacheTier, MemoryBudget, PressureLevel,
    ResourcePriority,
};

fn budget() -> MemoryBudget {
    MemoryBudget::new(100, 200, 300).unwrap()
}

fn entry(
    key: &'static str,
    tier: CacheTier,
    bytes: u64,
    generation: u64,
    priority: ResourcePriority,
) -> CacheEntry<&'static str> {
    CacheEntry {
        key,
        tier,
        bytes,
        generation: AssetGeneration(generation),
        priority,
        rebuildable: true,
    }
}

#[test]
fn baseline_budget_reserves_256_mib_for_gpu_images() {
    let budget = MemoryBudget::baseline_8gb();

    assert_eq!(budget.gpu_texture_bytes, 256 * 1024 * 1024);
    assert_eq!(
        budget.single_texture_limit_bytes(),
        102 * 1024 * 1024 + 419_430
    );
}

#[test]
fn tiers_are_accounted_and_bounded_independently() {
    let mut cache = BudgetedLru::new(budget());
    cache
        .insert(entry(
            "gpu-visible",
            CacheTier::GpuTexture,
            180,
            1,
            ResourcePriority::Visible,
        ))
        .unwrap();
    cache
        .insert(entry(
            "cpu-prefetch",
            CacheTier::CpuStaging,
            90,
            1,
            ResourcePriority::Prefetch,
        ))
        .unwrap();

    assert_eq!(cache.usage(CacheTier::GpuTexture), 180);
    assert_eq!(cache.usage(CacheTier::CpuStaging), 90);
    assert_eq!(cache.usage(CacheTier::DiskDerived), 0);
    assert_eq!(
        cache.insert(entry(
            "cpu-too-large",
            CacheTier::CpuStaging,
            101,
            1,
            ResourcePriority::Visible,
        )),
        Err(CacheError::EntryExceedsTierBudget {
            tier: CacheTier::CpuStaging,
            bytes: 101,
            limit: 100,
        })
    );
    assert!(cache.contains(&"gpu-visible"));
}

#[test]
fn insertion_evicts_lru_prefetch_but_keeps_visible_resources_pinned() {
    let mut cache = BudgetedLru::new(budget());
    cache
        .insert(entry(
            "visible",
            CacheTier::GpuTexture,
            120,
            1,
            ResourcePriority::Visible,
        ))
        .unwrap();
    cache
        .insert(entry(
            "prefetch-old",
            CacheTier::GpuTexture,
            60,
            1,
            ResourcePriority::Prefetch,
        ))
        .unwrap();

    let evicted = cache
        .insert(entry(
            "prefetch-new",
            CacheTier::GpuTexture,
            70,
            1,
            ResourcePriority::Prefetch,
        ))
        .unwrap();

    assert_eq!(evicted, vec!["prefetch-old"]);
    assert!(cache.contains(&"visible"));
    assert!(cache.contains(&"prefetch-new"));
    assert_eq!(cache.usage(CacheTier::GpuTexture), 190);
}

#[test]
fn pressure_levels_apply_progressively_stronger_reclamation() {
    let mut cache = BudgetedLru::new(MemoryBudget::new(1_000, 1_000, 1_000).unwrap());
    for item in [
        entry(
            "visible-current",
            CacheTier::GpuTexture,
            100,
            2,
            ResourcePriority::Visible,
        ),
        entry(
            "resident-current",
            CacheTier::GpuTexture,
            100,
            2,
            ResourcePriority::Resident,
        ),
        entry(
            "prefetch-current",
            CacheTier::GpuTexture,
            100,
            2,
            ResourcePriority::Prefetch,
        ),
        entry(
            "visible-old",
            CacheTier::GpuTexture,
            100,
            1,
            ResourcePriority::Visible,
        ),
    ] {
        cache.insert(item).unwrap();
    }

    assert_eq!(
        cache.apply_pressure(PressureLevel::Normal, AssetGeneration(2)),
        vec!["prefetch-current"]
    );
    assert_eq!(
        cache.apply_pressure(PressureLevel::Warning, AssetGeneration(2)),
        vec!["visible-old"]
    );
    assert_eq!(
        cache.apply_pressure(PressureLevel::Critical, AssetGeneration(2)),
        vec!["resident-current"]
    );
    assert!(cache.contains(&"visible-current"));
    assert_eq!(cache.usage(CacheTier::GpuTexture), 100);
}

#[test]
fn critical_pressure_preserves_non_rebuildable_entries() {
    let mut cache = BudgetedLru::new(MemoryBudget::new(1_000, 1_000, 1_000).unwrap());
    let mut source = entry(
        "source",
        CacheTier::CpuStaging,
        100,
        1,
        ResourcePriority::Resident,
    );
    source.rebuildable = false;
    cache.insert(source).unwrap();

    assert!(
        cache
            .apply_pressure(PressureLevel::Critical, AssetGeneration(2))
            .is_empty()
    );
    assert!(cache.contains(&"source"));
}

#[test]
fn explicit_removal_updates_only_the_target_tier_accounting() {
    let mut cache = BudgetedLru::new(budget());
    cache
        .insert(entry(
            "first",
            CacheTier::DiskDerived,
            100,
            1,
            ResourcePriority::Resident,
        ))
        .unwrap();
    cache
        .insert(entry(
            "second",
            CacheTier::DiskDerived,
            120,
            1,
            ResourcePriority::Resident,
        ))
        .unwrap();

    let removed = cache.remove(&"first").unwrap();

    assert_eq!(removed.key, "first");
    assert!(!cache.contains(&"first"));
    assert!(cache.contains(&"second"));
    assert_eq!(cache.usage(CacheTier::DiskDerived), 120);
}
