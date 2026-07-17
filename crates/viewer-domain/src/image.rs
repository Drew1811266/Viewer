use serde::{Deserialize, Serialize};

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
