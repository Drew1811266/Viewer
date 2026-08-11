CREATE TABLE IF NOT EXISTS video_metadata (
    node_id TEXT PRIMARY KEY NOT NULL REFERENCES nodes(entity_id) ON DELETE CASCADE,
    duration_us INTEGER,
    display_width INTEGER,
    display_height INTEGER,
    rotation_degrees INTEGER NOT NULL DEFAULT 0,
    frame_rate_millihertz INTEGER,
    video_codec TEXT,
    audio_codec TEXT,
    probe_status INTEGER NOT NULL,
    failure_kind INTEGER,
    updated_generation INTEGER NOT NULL
);
