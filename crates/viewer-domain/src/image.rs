use crate::EntityId;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ImageFormat {
    Jpeg,
    Png,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ImageRepresentationKind {
    Thumbnail {
        max_pixels: u32,
        scale_milli: u16,
    },
    FitPreview {
        max_width: u32,
        max_height: u32,
        scale_milli: u16,
    },
    Original100Percent,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ImageProbe {
    pub format: ImageFormat,
    pub width: u32,
    pub height: u32,
    pub orientation: u8,
    pub has_alpha: bool,
    pub icc_profile_name: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodeBudget {
    max_bytes: u64,
    hard_pixel_limit: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompareImageBudgetRequest {
    pub entity_id: EntityId,
    pub proxy_width: u32,
    pub proxy_height: u32,
    pub requested_original: Option<(u32, u32)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompareBudgetPlan {
    proxy_panes: Vec<EntityId>,
    original_panes: Vec<EntityId>,
    downgraded_panes: Vec<EntityId>,
    reserved_bytes: u64,
}

impl CompareBudgetPlan {
    pub fn proxy_panes(&self) -> &[EntityId] {
        &self.proxy_panes
    }

    pub fn original_panes(&self) -> &[EntityId] {
        &self.original_panes
    }

    pub fn downgraded_panes(&self) -> &[EntityId] {
        &self.downgraded_panes
    }

    pub const fn reserved_bytes(&self) -> u64 {
        self.reserved_bytes
    }

    pub fn cancelled_panes(&self, next: &Self) -> Vec<EntityId> {
        let retained = next.proxy_panes.iter().copied().collect::<HashSet<_>>();
        self.proxy_panes
            .iter()
            .copied()
            .filter(|entity_id| !retained.contains(entity_id))
            .collect()
    }
}

impl DecodeBudget {
    pub const fn new(max_bytes: u64, hard_pixel_limit: u64) -> Self {
        Self {
            max_bytes,
            hard_pixel_limit,
        }
    }

    pub fn rgba_bytes(width: u32, height: u32) -> Option<u64> {
        u64::from(width)
            .checked_mul(u64::from(height))?
            .checked_mul(4)
    }

    pub fn allows_full_decode(&self, width: u32, height: u32, bytes_per_pixel: u8) -> bool {
        let Some(pixels) = u64::from(width).checked_mul(u64::from(height)) else {
            return false;
        };
        let Some(bytes) = pixels.checked_mul(u64::from(bytes_per_pixel)) else {
            return false;
        };

        pixels <= self.hard_pixel_limit && bytes <= self.max_bytes
    }

    pub fn allows_proxy_set(&self, dimensions: &[(u32, u32)], bytes_per_pixel: u8) -> bool {
        dimensions
            .iter()
            .try_fold(0_u64, |sum, (width, height)| {
                let pixels = u64::from(*width).checked_mul(u64::from(*height))?;
                let bytes = pixels.checked_mul(u64::from(bytes_per_pixel))?;
                sum.checked_add(bytes)
            })
            .is_some_and(|bytes| bytes <= self.max_bytes)
    }

    pub fn plan_compare(
        &self,
        requests: &[CompareImageBudgetRequest],
        bytes_per_pixel: u8,
    ) -> Option<CompareBudgetPlan> {
        if requests.len() > 4 || bytes_per_pixel == 0 {
            return None;
        }
        let unique = requests
            .iter()
            .map(|request| request.entity_id)
            .collect::<HashSet<_>>();
        if unique.len() != requests.len() {
            return None;
        }

        let mut reserved_bytes = 0_u64;
        for request in requests {
            let proxy = decode_size(request.proxy_width, request.proxy_height, bytes_per_pixel)?;
            reserved_bytes = reserved_bytes.checked_add(proxy.bytes)?;
        }
        if reserved_bytes > self.max_bytes {
            return None;
        }

        let mut original_panes = Vec::new();
        let mut downgraded_panes = Vec::new();
        for request in requests {
            let Some((width, height)) = request.requested_original else {
                continue;
            };
            let Some(original) = decode_size(width, height, bytes_per_pixel) else {
                downgraded_panes.push(request.entity_id);
                continue;
            };
            let next_bytes = reserved_bytes.checked_add(original.bytes);
            if original.pixels > self.hard_pixel_limit
                || next_bytes.is_none_or(|bytes| bytes > self.max_bytes)
            {
                downgraded_panes.push(request.entity_id);
                continue;
            }
            reserved_bytes = next_bytes?;
            original_panes.push(request.entity_id);
        }

        Some(CompareBudgetPlan {
            proxy_panes: requests.iter().map(|request| request.entity_id).collect(),
            original_panes,
            downgraded_panes,
            reserved_bytes,
        })
    }
}

#[derive(Clone, Copy)]
struct DecodeSize {
    pixels: u64,
    bytes: u64,
}

fn decode_size(width: u32, height: u32, bytes_per_pixel: u8) -> Option<DecodeSize> {
    if width == 0 || height == 0 || bytes_per_pixel == 0 {
        return None;
    }
    let pixels = u64::from(width).checked_mul(u64::from(height))?;
    let bytes = pixels.checked_mul(u64::from(bytes_per_pixel))?;
    Some(DecodeSize { pixels, bytes })
}

#[cfg(test)]
mod tests {
    use super::DecodeBudget;

    #[test]
    fn rgba_memory_is_width_times_height_times_four() {
        assert_eq!(DecodeBudget::rgba_bytes(12_000, 8_000), Some(384_000_000));
    }

    #[test]
    fn hard_pixel_limit_rejects_full_decode() {
        let budget = DecodeBudget::new(700_000_000, 100_000_000);
        assert!(!budget.allows_full_decode(20_000, 6_000, 4));
    }

    #[test]
    fn four_way_compare_reserves_all_visible_proxies() {
        let budget = DecodeBudget::new(700_000_000, 100_000_000);
        assert!(budget.allows_proxy_set(&[(3_000, 2_000); 4], 4));
    }

    #[test]
    fn overflowing_full_decode_size_is_rejected() {
        let budget = DecodeBudget::new(u64::MAX, u64::MAX);
        assert!(!budget.allows_full_decode(u32::MAX, u32::MAX, u8::MAX));
    }

    #[test]
    fn overflowing_proxy_set_size_is_rejected() {
        let budget = DecodeBudget::new(u64::MAX, u64::MAX);
        assert!(!budget.allows_proxy_set(&[(u32::MAX, u32::MAX)], u8::MAX));
    }
}
