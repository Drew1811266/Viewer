use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, SeqAccess, Visitor},
    ser::SerializeSeq,
};
use std::{fmt, marker::PhantomData, str::FromStr};
use viewer_domain::{
    review::{FeedbackAnchor, ImageStroke, NormalizedPoint, NormalizedRect, ProductionId},
    *,
};

pub trait WireValue: Sized {
    const ARRAY_LIMIT: usize = 100_000;
    fn encode<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error>;
    fn decode<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewWire<T>(pub T);
impl<T> From<T> for ReviewWire<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}
impl<T: WireValue> Serialize for ReviewWire<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.encode(serializer)
    }
}
impl<'de, T: WireValue> Deserialize<'de> for ReviewWire<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        T::decode(deserializer).map(Self)
    }
}
pub fn serialize<T: WireValue, S: Serializer>(value: &T, serializer: S) -> Result<S::Ok, S::Error> {
    value.encode(serializer)
}
pub fn deserialize<'de, T: WireValue, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<T, D::Error> {
    T::decode(deserializer)
}

// `with = "wire"` makes nullable fields required instead of silently defaulting missing keys.
impl<T: WireValue> WireValue for Option<T> {
    fn encode<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Some(value) => serializer.serialize_some(&Ref(value)),
            None => serializer.serialize_none(),
        }
    }
    fn decode<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Option::<ReviewWire<T>>::deserialize(deserializer).map(|v| v.map(|v| v.0))
    }
}
struct Ref<'a, T>(&'a T);
impl<T: WireValue> Serialize for Ref<'_, T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.encode(serializer)
    }
}
impl<T: WireValue> WireValue for Vec<T> {
    fn encode<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.len()))?;
        for value in self {
            seq.serialize_element(&Ref(value))?;
        }
        seq.end()
    }
    fn decode<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Bounded<T>(PhantomData<T>);
        impl<'de, T: WireValue> Visitor<'de> for Bounded<T> {
            type Value = Vec<T>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a bounded review array")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(value) = seq.next_element::<ReviewWire<T>>()? {
                    if values.len() >= T::ARRAY_LIMIT {
                        return Err(de::Error::custom("review array limit exceeded"));
                    }
                    values.push(value.0);
                }
                Ok(values)
            }
        }
        deserializer.deserialize_seq(Bounded(PhantomData))
    }
}
impl<T: WireValue> WireValue for std::sync::Arc<T> {
    fn encode<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.as_ref().encode(s)
    }
    fn decode<'de, D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        T::decode(d).map(std::sync::Arc::new)
    }
}
macro_rules! ids { ($($ty:ty),+ $(,)?) => { $(impl WireValue for $ty {
    fn encode<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> { s.serialize_str(&self.to_string()) }
    fn decode<'de, D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = String::deserialize(d)?;
        let parsed = Self::from_str(&value).map_err(|_| de::Error::custom("invalid review identity"))?;
        if value.len() != 36 || parsed.to_string() != value { return Err(de::Error::custom("noncanonical review identity")); }
        Ok(parsed)
    }
})+ }; }
ids!(
    ProjectId,
    SessionId,
    EntityId,
    AssetVersionId,
    FeedbackId,
    ReviewStreamId,
    ReviewRoundId,
    ReviewSnapshotId,
    ReviewCommandId,
    ReviewArchiveId,
    ReviewUsageId,
    ReviewTargetId,
    ReviewTargetRevisionId,
    ReviewTextRevisionId
);

impl WireValue for [u8; 32] {
    fn encode<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut hex = String::with_capacity(64);
        use std::fmt::Write;
        for byte in self {
            write!(hex, "{byte:02x}").expect("writing to string");
        }
        s.serialize_str(&hex)
    }
    fn decode<'de, D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = String::deserialize(d)?;
        if value.len() != 64
            || !value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(de::Error::custom("invalid lowercase BLAKE3 digest"));
        }
        Ok(std::array::from_fn(|i| {
            u8::from_str_radix(&value[2 * i..2 * i + 2], 16).expect("checked hex")
        }))
    }
}
impl WireValue for String {
    fn encode<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self)
    }
    fn decode<'de, D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = String::deserialize(d)?;
        if value.len() > 65_536 {
            return Err(de::Error::custom("review text limit exceeded"));
        }
        Ok(value)
    }
}
impl WireValue for RelativePath {
    fn encode<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
    fn decode<'de, D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = String::decode(d)?;
        if value.len() > 4096
            || value.contains('\\')
            || value
                .split('/')
                .any(|v| v.is_empty() || v == "." || v == ".." || v == ".viewer")
        {
            return Err(de::Error::custom("unsafe project-relative review path"));
        }
        Self::parse(&value).map_err(|_| de::Error::custom("unsafe project-relative review path"))
    }
}
impl WireValue for ProductionId {
    fn encode<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
    fn decode<'de, D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::parse(&String::decode(d)?)
            .map_err(|_| de::Error::custom("invalid production identity"))
    }
}
macro_rules! integers { ($($ty:ty),+ $(,)?) => { $(impl WireValue for $ty {
    fn encode<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if i128::from(*self).abs() > 9_007_199_254_740_991 { return Err(serde::ser::Error::custom("integer is not JavaScript-exact")); }
        self.serialize(s)
    }
    fn decode<'de, D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = Self::deserialize(d)?;
        if i128::from(value).abs() > 9_007_199_254_740_991 { return Err(de::Error::custom("integer is not JavaScript-exact")); }
        Ok(value)
    }
})+ }; }
integers!(u32, u64, i64);

impl WireValue for i128 {
    fn encode<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
    fn decode<'de, D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = String::deserialize(d)?;
        let n = value
            .parse::<i128>()
            .map_err(|_| de::Error::custom("invalid timestamp"))?;
        if n.to_string() != value {
            return Err(de::Error::custom("noncanonical timestamp"));
        }
        Ok(n)
    }
}
impl WireValue for std::sync::Arc<str> {
    fn encode<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self)
    }
    fn decode<'de, D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::decode(d).map(Into::into)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum Anchor {
    Asset {},
    ImageRect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
    ImageStroke {
        #[serde(with = "self")]
        points: Vec<Point>,
    },
    VideoPoint {
        #[serde(with = "self")]
        position_us: u64,
    },
    VideoRange {
        #[serde(with = "self")]
        start_us: u64,
        #[serde(with = "self")]
        end_us: u64,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Point {
    x: f64,
    y: f64,
}
impl WireValue for Point {
    const ARRAY_LIMIT: usize = 2048;
    fn encode<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.serialize(s)
    }
    fn decode<'de, D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::deserialize(d)
    }
}
impl WireValue for FeedbackAnchor {
    fn encode<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        // Legacy unknown-duration video may contain any u64; the old DTO has no JS-exact guard.
        match self {
            Self::VideoPoint { position_us } => Anchor::VideoPoint {
                position_us: *position_us,
            }
            .serialize(s),
            Self::VideoRange { start_us, end_us } => Anchor::VideoRange {
                start_us: *start_us,
                end_us: *end_us,
            }
            .serialize(s),
            _ => crate::dto::ReviewAnchorDto::from(self.clone()).serialize(s),
        }
    }
    fn decode<'de, D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let invalid = |_| de::Error::custom("invalid review anchor");
        match Anchor::deserialize(d)? {
            Anchor::Asset {} => Ok(Self::Asset),
            Anchor::ImageRect {
                x,
                y,
                width,
                height,
            } => NormalizedRect::new(x, y, width, height)
                .map(Self::ImageRect)
                .map_err(invalid),
            Anchor::ImageStroke { points } => {
                let points = points
                    .into_iter()
                    .map(|p| NormalizedPoint::new(p.x, p.y))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(invalid)?;
                ImageStroke::new(points)
                    .map(Self::ImageStroke)
                    .map_err(invalid)
            }
            Anchor::VideoPoint { position_us } => Ok(Self::VideoPoint { position_us }),
            Anchor::VideoRange { start_us, end_us } if start_us < end_us => {
                Ok(Self::VideoRange { start_us, end_us })
            }
            _ => Err(de::Error::custom("invalid review anchor")),
        }
    }
}
