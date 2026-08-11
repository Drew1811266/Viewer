use super::*;

impl DesktopRuntime {
    pub async fn set_review_state(
        &self,
        expected_session: SessionId,
        expected_generation: Generation,
        entity_ids: &[EntityId],
        review_state: Option<ReviewState>,
    ) -> Result<MarkerBatchResultDto, CommandError> {
        let review = review_state.map_or(ReviewPatch::Clear, ReviewPatch::Set);
        self.apply_marker_patch(
            expected_session,
            expected_generation,
            entity_ids,
            MarkerPatch {
                review,
                favorite: FavoritePatch::Unchanged,
            },
        )
        .await
    }

    pub async fn toggle_favorite(
        &self,
        expected_session: SessionId,
        expected_generation: Generation,
        entity_ids: &[EntityId],
    ) -> Result<MarkerBatchResultDto, CommandError> {
        self.apply_marker_patch(
            expected_session,
            expected_generation,
            entity_ids,
            MarkerPatch {
                review: ReviewPatch::Unchanged,
                favorite: FavoritePatch::Toggle,
            },
        )
        .await
    }

    pub async fn selection_info(
        &self,
        expected_session: SessionId,
        expected_generation: Generation,
        entity_ids: &[EntityId],
    ) -> Result<SelectionInfoDto, CommandError> {
        let index = {
            let session = self.session.lock().await;
            let session = session.as_ref().ok_or_else(project_not_open)?;
            validate_project_request(&session.active, expected_session, expected_generation)?;
            Arc::clone(&session.index)
        };
        BrowseService::new(index.as_ref())
            .selection_info(entity_ids)
            .map(SelectionInfoDto::from)
            .map_err(CommandError::from)
    }

    async fn apply_marker_patch(
        &self,
        expected_session: SessionId,
        expected_generation: Generation,
        entity_ids: &[EntityId],
        patch: MarkerPatch,
    ) -> Result<MarkerBatchResultDto, CommandError> {
        let (active, index, store, projection, marker_lock, undo_stack) = {
            let session = self.session.lock().await;
            let session = session.as_ref().ok_or_else(project_not_open)?;
            validate_project_request(&session.active, expected_session, expected_generation)?;
            if session.active.access != ProjectAccess::ReadWrite {
                return Err(CommandError::new(
                    "project_read_only",
                    ErrorCategory::Conflict,
                    "当前项目为只读，无法保存标记。",
                    false,
                ));
            }
            let store = session.portable_store.clone().ok_or_else(|| {
                CommandError::new(
                    "portable_metadata_unavailable",
                    ErrorCategory::Consistency,
                    "项目审阅数据不可用，请重新打开项目。",
                    true,
                )
            })?;
            (
                session.active.clone(),
                Arc::clone(&session.index),
                store,
                Arc::clone(&session.marker_projection),
                Arc::clone(&session.marker_lock),
                Arc::clone(&session.undo_stack),
            )
        };
        let _marker_guard = marker_lock.lock().await;
        self.ensure_project_current(active.session_id, active.generation)
            .await?;
        let ids = entity_ids.to_vec();
        let active_for_work = active.clone();
        let index_for_work = Arc::clone(&index);
        let updated_at_ms = self.clock.unix_millis();
        let changes = tokio::task::spawn_blocking(move || {
            let targets = ids
                .into_iter()
                .map(|entity_id| {
                    let indexed = index_for_work
                        .indexed_node(entity_id)
                        .map_err(CommandError::from)?
                        .ok_or_else(selection_not_found)?;
                    validated_marker_target(&active_for_work, &indexed.node)
                        .ok_or_else(selection_not_found)
                })
                .collect::<Result<Vec<_>, CommandError>>()?;
            MarkerService::new(store.as_ref(), projection.as_ref(), true)
                .apply_with_undo(
                    &targets,
                    patch,
                    updated_at_ms,
                    &mut undo_stack
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner()),
                )
                .map_err(CommandError::from)
        })
        .await
        .map_err(|_| internal_command_error())??;
        self.ensure_project_current(active.session_id, active.generation)
            .await?;
        Ok(changes.into())
    }
}

fn validated_marker_target(
    active: &ActiveProject,
    node: &viewer_domain::file::FileNode,
) -> Option<MarkerTarget> {
    let candidate = active.root.join(node.relative_path.as_str());
    let metadata = std::fs::symlink_metadata(&candidate).ok()?;
    if metadata.file_type().is_symlink() || is_macos_alias(&candidate) {
        return None;
    }
    let kind_matches = match node.kind {
        FileKind::Directory => metadata.is_dir(),
        FileKind::Jpeg
        | FileKind::Png
        | FileKind::Markdown
        | FileKind::Text
        | FileKind::UnsupportedImage
        | FileKind::Other
        | FileKind::Video => metadata.is_file(),
    };
    if !kind_matches || entity_id_for_metadata(&metadata, &node.relative_path) != node.entity_id {
        return None;
    }
    let canonical = std::fs::canonicalize(&candidate).ok()?;
    if !canonical.starts_with(&active.root) {
        return None;
    }
    Some(MarkerTarget {
        entity_id: node.entity_id,
        relative_path: node.relative_path.clone(),
        kind: node.kind,
        size: if node.kind == FileKind::Directory {
            0
        } else {
            metadata.len()
        },
        modified_ns: modified_ns(&metadata),
    })
}
