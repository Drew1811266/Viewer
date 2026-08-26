use super::protocol::v1;
use viewer_application::{
    ReviewCatalog, ReviewProtocolVersion, ReviewRecordLocation, ReviewRepositoryError,
};
use viewer_domain::{ReviewRoundId, ReviewStreamId};

pub(crate) fn prepare_v2_catalog_migration<F>(
    mut catalog_document: ReviewCatalog,
    mut read_legacy_round: F,
) -> Result<ReviewCatalog, ReviewRepositoryError>
where
    F: FnMut(ReviewStreamId, ReviewRoundId) -> Result<Vec<u8>, ReviewRepositoryError>,
{
    for stream in &mut catalog_document.streams {
        for record in &mut stream.completed_rounds {
            match record.protocol_version {
                ReviewProtocolVersion::V2 => continue,
                ReviewProtocolVersion::V1
                    if record.location.as_str()
                        == legacy_round_location(record.review_round_id) => {}
                ReviewProtocolVersion::V1 => {
                    return Err(ReviewRepositoryError::RecoveryRequired);
                }
            }
            let bytes = read_legacy_round(stream.review_stream_id, record.review_round_id)
                .map_err(migration_error)?;
            let snapshot = v1::decode_completed(&bytes)
                .map_err(|_| ReviewRepositoryError::RecoveryRequired)?;
            if snapshot.project_id != catalog_document.project_id
                || snapshot.review_stream_id != stream.review_stream_id
                || snapshot.review_round_id != record.review_round_id
                || snapshot.production != stream.production
            {
                return Err(ReviewRepositoryError::RecoveryRequired);
            }
            let digest = *blake3::hash(&bytes).as_bytes();
            if record.blake3 != [0; 32] && record.blake3 != digest {
                return Err(ReviewRepositoryError::RecoveryRequired);
            }
            record.location =
                ReviewRecordLocation::new(legacy_round_location(record.review_round_id))
                    .map_err(|_| ReviewRepositoryError::RecoveryRequired)?;
            record.blake3 = digest;
        }
    }
    Ok(catalog_document)
}

pub(crate) fn legacy_round_location(round_id: ReviewRoundId) -> String {
    format!("rounds/{round_id}.json")
}

fn migration_error(error: ReviewRepositoryError) -> ReviewRepositoryError {
    match error {
        ReviewRepositoryError::LimitExceeded => ReviewRepositoryError::LimitExceeded,
        _ => ReviewRepositoryError::RecoveryRequired,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viewer_application::{ReviewRoundRecord, ReviewStreamHead};
    use viewer_domain::{ProjectId, ReviewStreamId};

    #[test]
    fn migration_refuses_noncanonical_legacy_locations_before_io() {
        let round_id = ReviewRoundId::from_u128(2);
        let catalog = ReviewCatalog {
            project_id: ProjectId::from_u128(1),
            streams: vec![ReviewStreamHead {
                review_stream_id: ReviewStreamId::from_u128(3),
                production: None,
                completed_rounds: vec![ReviewRoundRecord {
                    review_round_id: round_id,
                    protocol_version: ReviewProtocolVersion::V1,
                    location: ReviewRecordLocation::new("rounds/other.json").unwrap(),
                    blake3: [0; 32],
                }],
                latest_completed_round_id: Some(round_id),
            }],
        };
        let mut reads = 0;

        let result = prepare_v2_catalog_migration(catalog, |_, _| {
            reads += 1;
            Ok(Vec::new())
        });

        assert_eq!(result, Err(ReviewRepositoryError::RecoveryRequired));
        assert_eq!(reads, 0);
    }
}
