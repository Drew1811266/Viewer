use serde::{Deserialize, Serialize};
use std::fmt;

use super::error::{PlanError, PlanErrorCode};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PlanId(String);

impl PlanId {
    pub const MAX_LEN: usize = 64;

    pub fn parse(value: &str) -> Result<Self, PlanError> {
        let mut characters = value.chars();
        let valid = !value.is_empty()
            && value.len() <= Self::MAX_LEN
            && characters.next().is_some_and(|first| first.is_ascii_alphanumeric())
            && value
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
        if valid {
            Ok(Self(value.to_owned()))
        } else {
            Err(PlanError::new(
                "planId",
                PlanErrorCode::InvalidSchema,
                "planId must match ^[A-Za-z0-9][A-Za-z0-9._-]*$ and be at most 64 characters",
            ))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PlanId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for PlanId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for PlanId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Revision(u32);

impl Revision {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AssetId([u8; 32]);

impl AssetId {
    pub const HEX_LEN: usize = 64;

    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn from_hex(value: &str) -> Result<Self, PlanError> {
        if value.len() != Self::HEX_LEN || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(PlanError::new(
                "references",
                PlanErrorCode::InvalidSchema,
                "assetId must be 64 lowercase hexadecimal characters",
            ));
        }
        let mut bytes = [0u8; 32];
        for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
            let pair = std::str::from_utf8(chunk).map_err(|_| {
                PlanError::new(
                    "references",
                    PlanErrorCode::InvalidSchema,
                    "assetId must be hexadecimal",
                )
            })?;
            bytes[index] = u8::from_str_radix(pair, 16).map_err(|_| {
                PlanError::new(
                    "references",
                    PlanErrorCode::InvalidSchema,
                    "assetId must be hexadecimal",
                )
            })?;
        }
        Ok(Self(bytes))
    }

    pub fn as_hex(&self) -> String {
        let mut out = String::with_capacity(Self::HEX_LEN);
        for byte in self.0 {
            out.push_str(&format!("{byte:02x}"));
        }
        out
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.as_hex())
    }
}

impl Serialize for AssetId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.as_hex())
    }
}

impl<'de> Deserialize<'de> for AssetId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::from_hex(&value).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_id_rejects_invalid_shapes() {
        for invalid in ["", "-lead", "has space", "has/slash", &"a".repeat(65)] {
            assert!(PlanId::parse(invalid).is_err(), "accepted {invalid:?}");
        }
        assert_eq!(PlanId::parse("autumn-hero_1").unwrap().as_str(), "autumn-hero_1");
    }

    #[test]
    fn asset_id_round_trips_between_bytes_and_hex() {
        let hex = "9f2c".to_owned() + &"ab".repeat(30);
        let id = AssetId::from_hex(&hex).unwrap();
        assert_eq!(id.as_hex(), hex);
        assert_eq!(AssetId::from_hex(&hex.to_uppercase()).unwrap(), id);
    }

    #[test]
    fn asset_id_rejects_wrong_length_or_non_hex() {
        for invalid in ["", "abc", &"z".repeat(64), &"a".repeat(63)] {
            assert!(AssetId::from_hex(invalid).is_err(), "accepted {invalid:?}");
        }
    }
}
