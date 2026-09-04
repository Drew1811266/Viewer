use crate::{
    AssetGeneration, CameraState, CommandId, RenderSessionId, ScenePatch, SceneRevision,
    SceneSnapshot,
};

#[derive(Clone, Debug, PartialEq)]
pub struct RenderEnvelope<T> {
    pub session_id: RenderSessionId,
    pub asset_generation: AssetGeneration,
    pub scene_revision: SceneRevision,
    pub command_id: CommandId,
    pub payload: T,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RenderCommand {
    OpenAsset { entity_id: String },
    SetCamera(CameraState),
    SetScene(SceneSnapshot),
    ApplyScenePatch(ScenePatch),
    Noop,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RevisionDecision {
    Apply,
    IgnoreDuplicate,
    IgnoreStale,
    RequireSnapshot {
        expected_generation: AssetGeneration,
        expected_revision: SceneRevision,
    },
}

#[derive(Clone, Debug)]
pub struct RevisionGate {
    session_id: RenderSessionId,
    asset_generation: AssetGeneration,
    scene_revision: SceneRevision,
    last_command_id: Option<CommandId>,
}

impl RevisionGate {
    pub const fn new(session_id: RenderSessionId, asset_generation: AssetGeneration) -> Self {
        Self {
            session_id,
            asset_generation,
            scene_revision: SceneRevision(0),
            last_command_id: None,
        }
    }

    pub const fn asset_generation(&self) -> AssetGeneration {
        self.asset_generation
    }

    pub const fn scene_revision(&self) -> SceneRevision {
        self.scene_revision
    }

    pub fn classify(&mut self, envelope: &RenderEnvelope<RenderCommand>) -> RevisionDecision {
        if envelope.session_id != self.session_id
            || envelope.asset_generation < self.asset_generation
        {
            return RevisionDecision::IgnoreStale;
        }

        if envelope.asset_generation > self.asset_generation {
            if matches!(envelope.payload, RenderCommand::OpenAsset { .. })
                && envelope.scene_revision == SceneRevision(0)
            {
                self.asset_generation = envelope.asset_generation;
                self.scene_revision = SceneRevision(0);
                self.last_command_id = Some(envelope.command_id);
                return RevisionDecision::Apply;
            }
            return RevisionDecision::RequireSnapshot {
                expected_generation: envelope.asset_generation,
                expected_revision: SceneRevision(0),
            };
        }

        if let Some(last) = self.last_command_id {
            if envelope.command_id == last {
                return RevisionDecision::IgnoreDuplicate;
            }
            if envelope.command_id < last {
                return RevisionDecision::IgnoreStale;
            }
        }

        let next_revision = match &envelope.payload {
            RenderCommand::SetScene(scene)
                if scene.revision() == envelope.scene_revision
                    && scene.revision() >= self.scene_revision =>
            {
                scene.revision()
            }
            RenderCommand::ApplyScenePatch(patch)
                if patch.base_revision() == self.scene_revision
                    && patch.revision() == envelope.scene_revision
                    && self.scene_revision.checked_next() == Some(patch.revision()) =>
            {
                patch.revision()
            }
            RenderCommand::SetCamera(_) | RenderCommand::Noop
                if envelope.scene_revision == self.scene_revision =>
            {
                self.scene_revision
            }
            _ if envelope.scene_revision < self.scene_revision => {
                return RevisionDecision::IgnoreStale;
            }
            _ => {
                return RevisionDecision::RequireSnapshot {
                    expected_generation: self.asset_generation,
                    expected_revision: self.scene_revision,
                };
            }
        };

        self.scene_revision = next_revision;
        self.last_command_id = Some(envelope.command_id);
        RevisionDecision::Apply
    }
}
