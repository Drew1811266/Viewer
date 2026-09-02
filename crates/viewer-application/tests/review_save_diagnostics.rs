#[path = "support/continuous_review.rs"]
mod support;

use std::sync::{Arc, Mutex};

use support::*;
use viewer_application::{ReviewTaskCancellation, review_workspace::*};
use viewer_domain::{review::FeedbackAnchor, *};

#[derive(Default)]
struct RecordingSaveObserver(Mutex<Vec<ReviewSaveStage>>);

impl RecordingSaveObserver {
    fn stages(&self) -> Vec<ReviewSaveStage> {
        self.0.lock().unwrap().clone()
    }
}

impl ReviewSaveObserverPort for RecordingSaveObserver {
    fn record(&self, measurement: ReviewSaveMeasurement) {
        self.0.lock().unwrap().push(measurement.stage);
    }
}

fn save(asset_version_id: AssetVersionId, text: &str) -> ReviewWorkspaceCommand {
    ReviewWorkspaceCommand::SaveFeedback {
        feedback_id: None,
        text: text.into(),
        targets: vec![TargetEdit::Add {
            asset_version_id,
            anchor: FeedbackAnchor::Asset,
        }],
    }
}

#[tokio::test]
async fn synchronous_baseline_reports_expensive_work_before_reply() {
    let fixture = Fixture::new();
    let observer = Arc::new(RecordingSaveObserver::default());
    let service = fixture.service().with_save_observer(observer.clone());
    let first = service
        .prepare_assets(
            &[EntityId::from_u128(10)],
            ReviewTaskCancellation::default(),
        )
        .await
        .unwrap()
        .remove(0);

    let envelope = service
        .prepare(
            ReviewCommandId::from_u128(70),
            None,
            save(first.id, "收紧左袖口"),
        )
        .await
        .unwrap();
    service.apply(envelope).await.unwrap();

    assert_eq!(
        observer.stages(),
        vec![
            ReviewSaveStage::EvidenceMaterialization,
            ReviewSaveStage::PublicV3Publish,
            ReviewSaveStage::WorkspaceRefresh,
        ]
    );
}
