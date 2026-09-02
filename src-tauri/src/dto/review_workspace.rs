//! Desktop-owned wire adapters. Application/Domain types do not own JSON or IPC layout.
mod command;
mod error;
mod evidence;
mod history;
mod legacy;
mod plans;
mod recovery;
mod requests;
mod response;
mod selection;
mod view;
mod wire;

pub use error::*;
pub use evidence::{PreparedReviewAssetDto, ReviewEvidenceImageDto};
pub use requests::*;
pub use response::review_response;
pub use wire::ReviewWire;
pub type PreparedReviewCommandDto =
    ReviewWire<viewer_application::review_workspace::ReviewCommandEnvelope>;
pub type ReviewWorkspaceViewDto =
    ReviewWire<viewer_application::review_workspace::ReviewWorkspaceView>;
pub type ReviewHistoryViewDto = ReviewWire<viewer_application::review_workspace::HistoryView>;
pub type ReviewAuthoringApplyResultDto =
    ReviewWire<viewer_application::review_workspace::ReviewAuthoringApplyResult>;
pub type ReviewPublicationStatusDto =
    ReviewWire<viewer_application::review_workspace::ReviewPublicationStatus>;

macro_rules! remote_wire {
    ($domain:ty, $remote:ident) => {
        super::remote_wire!($domain, $remote, 100_000);
    };
    ($domain:ty, $remote:ident, $array_limit:expr) => {
        impl super::wire::WireValue for $domain {
            const ARRAY_LIMIT: usize = $array_limit;
            fn encode<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                $remote::serialize(self, serializer)
            }
            fn decode<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                $remote::deserialize(deserializer)
            }
        }
    };
}
pub(crate) use remote_wire;

macro_rules! remote_output {
    ($domain:ty, $remote:ident) => {
        impl super::wire::WireValue for $domain {
            fn encode<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                $remote::serialize(self, serializer)
            }
            fn decode<'de, D: serde::Deserializer<'de>>(_: D) -> Result<Self, D::Error> {
                Err(serde::de::Error::custom(
                    "review response cannot be supplied as a request",
                ))
            }
        }
    };
}
pub(crate) use remote_output;
