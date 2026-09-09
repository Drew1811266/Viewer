use viewer_render_core::{
    AssetGeneration, CameraState, CommandId, RenderCommand, RenderEnvelope, RenderSessionId,
    RevisionDecision, RevisionGate, Rotation, SceneRevision,
};

fn envelope(
    generation: u64,
    revision: u64,
    command_id: u64,
    payload: RenderCommand,
) -> RenderEnvelope<RenderCommand> {
    RenderEnvelope {
        session_id: RenderSessionId(7),
        asset_generation: AssetGeneration(generation),
        scene_revision: SceneRevision(revision),
        command_id: CommandId(command_id),
        payload,
    }
}

#[test]
fn a_command_from_the_previous_asset_generation_is_ignored() {
    let mut gate = RevisionGate::new(RenderSessionId(7), AssetGeneration(3));
    let stale = envelope(
        2,
        0,
        44,
        RenderCommand::SetCamera(CameraState::fit(Rotation::Deg0)),
    );

    assert_eq!(gate.classify(&stale), RevisionDecision::IgnoreStale);
}

#[test]
fn a_retried_command_is_idempotently_ignored() {
    let mut gate = RevisionGate::new(RenderSessionId(7), AssetGeneration(3));
    let command = envelope(
        3,
        0,
        1,
        RenderCommand::SetCamera(CameraState::fit(Rotation::Deg0)),
    );

    assert_eq!(gate.classify(&command), RevisionDecision::Apply);
    assert_eq!(gate.classify(&command), RevisionDecision::IgnoreDuplicate);
}

#[test]
fn a_revision_gap_requires_a_full_snapshot() {
    let mut gate = RevisionGate::new(RenderSessionId(7), AssetGeneration(3));
    let gap = envelope(3, 2, 1, RenderCommand::Noop);

    assert_eq!(
        gate.classify(&gap),
        RevisionDecision::RequireSnapshot {
            expected_generation: AssetGeneration(3),
            expected_revision: SceneRevision(0),
        }
    );
}

#[test]
fn opening_a_new_generation_resets_scene_ordering() {
    let mut gate = RevisionGate::new(RenderSessionId(7), AssetGeneration(3));
    let open = envelope(
        4,
        0,
        9,
        RenderCommand::OpenAsset {
            entity_id: "entity-4".into(),
        },
    );

    assert_eq!(gate.classify(&open), RevisionDecision::Apply);
    assert_eq!(gate.asset_generation(), AssetGeneration(4));
    assert_eq!(gate.scene_revision(), SceneRevision(0));
}

#[test]
fn another_session_cannot_mutate_the_gate() {
    let mut gate = RevisionGate::new(RenderSessionId(7), AssetGeneration(3));
    let mut foreign = envelope(3, 0, 1, RenderCommand::Noop);
    foreign.session_id = RenderSessionId(8);

    assert_eq!(gate.classify(&foreign), RevisionDecision::IgnoreStale);
    assert_eq!(gate.scene_revision(), SceneRevision(0));
}
