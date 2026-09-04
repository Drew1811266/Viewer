use viewer_render_core::SceneRevision;

pub const FRAMES_IN_FLIGHT: usize = 3;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FrameReasons {
    pub camera: bool,
    pub scene: bool,
    pub resource: bool,
    pub surface: bool,
}

impl FrameReasons {
    pub const fn is_dirty(self) -> bool {
        self.camera || self.scene || self.resource || self.surface
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameRequest {
    pub scene_revision: SceneRevision,
    pub reasons: FrameReasons,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameReceipt {
    pub frame_index: u64,
    pub scene_revision: SceneRevision,
    pub cpu_time_ns: u64,
    pub gpu_time_ns: u64,
    pub gpu_resource_bytes: u64,
    pub presented: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameState {
    reasons: FrameReasons,
    scene_revision: SceneRevision,
}

impl Default for FrameState {
    fn default() -> Self {
        Self {
            reasons: FrameReasons::default(),
            scene_revision: SceneRevision(0),
        }
    }
}

impl FrameState {
    pub fn invalidate_camera(&mut self) {
        self.reasons.camera = true;
    }

    pub fn invalidate_scene(&mut self, revision: SceneRevision) {
        self.scene_revision = self.scene_revision.max(revision);
        self.reasons.scene = true;
    }

    pub fn invalidate_resource(&mut self) {
        self.reasons.resource = true;
    }

    pub fn invalidate_surface(&mut self) {
        self.reasons.surface = true;
    }

    pub fn take_request(&mut self) -> Option<FrameRequest> {
        if !self.reasons.is_dirty() {
            return None;
        }
        let request = FrameRequest {
            scene_revision: self.scene_revision,
            reasons: self.reasons,
        };
        self.reasons = FrameReasons::default();
        Some(request)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameSlot(pub usize);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameRing {
    busy: [bool; FRAMES_IN_FLIGHT],
    cursor: usize,
}

impl Default for FrameRing {
    fn default() -> Self {
        Self {
            busy: [false; FRAMES_IN_FLIGHT],
            cursor: 0,
        }
    }
}

impl FrameRing {
    pub fn acquire(&mut self) -> Option<FrameSlot> {
        for offset in 0..FRAMES_IN_FLIGHT {
            let index = (self.cursor + offset) % FRAMES_IN_FLIGHT;
            if !self.busy[index] {
                self.busy[index] = true;
                self.cursor = (index + 1) % FRAMES_IN_FLIGHT;
                return Some(FrameSlot(index));
            }
        }
        None
    }

    pub fn complete(&mut self, slot: FrameSlot) -> bool {
        let Some(busy) = self.busy.get_mut(slot.0) else {
            return false;
        };
        let was_busy = *busy;
        *busy = false;
        was_busy
    }
}
