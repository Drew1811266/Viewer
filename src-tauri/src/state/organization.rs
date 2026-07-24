use super::*;

impl DesktopRuntime {
    pub async fn preview_rename(
        &self,
        expected_session: SessionId,
        expected_generation: Generation,
        entity_ids: &[EntityId],
        rules: RenameRuleSet,
    ) -> Result<RenamePreflight, CommandError> {
        let (active, index) = {
            let session = self.session.lock().await;
            let session = session.as_ref().ok_or_else(project_not_open)?;
            validate_project_request(&session.active, expected_session, expected_generation)?;
            if session.active.access != ProjectAccess::ReadWrite {
                return Err(project_read_only_operation());
            }
            (session.active.clone(), Arc::clone(&session.index))
        };
        let ids = entity_ids.to_vec();
        let prepared = tokio::task::spawn_blocking(move || {
            let targets = ids
                .into_iter()
                .map(|entity_id| {
                    index
                        .node(entity_id)
                        .map_err(CommandError::from)?
                        .filter(|node| node.kind != FileKind::Directory)
                        .map(|node| RenameTarget {
                            entity_id: node.entity_id,
                            relative_path: node.relative_path,
                        })
                        .ok_or_else(selection_not_found)
                })
                .collect::<Result<Vec<_>, CommandError>>()?;
            RenamePlanner::preflight(&active.root, &MacVolumePort, &targets, &rules)
                .map(|prepared| prepared.preview)
                .map_err(|_| invalid_rename_preview())
        })
        .await
        .map_err(|_| internal_command_error())??;
        self.ensure_project_current(expected_session, expected_generation)
            .await?;
        Ok(prepared)
    }

    pub async fn prepare_finder_drag(
        &self,
        expected_session: SessionId,
        expected_generation: Generation,
        entity_ids: &[EntityId],
    ) -> Result<PreparedFinderDrag, CommandError> {
        let (root, index) = {
            let session = self.session.lock().await;
            let session = session.as_ref().ok_or_else(project_not_open)?;
            validate_project_request(&session.active, expected_session, expected_generation)?;
            (session.active.root.clone(), Arc::clone(&session.index))
        };
        let ids = entity_ids.to_vec();
        let prepared = tokio::task::spawn_blocking(move || {
            viewer_application::prepare_finder_drag(&root, index.as_ref(), &ids)
        })
        .await
        .map_err(|_| internal_command_error())?
        .map_err(CommandError::from)?;
        self.ensure_project_current(expected_session, expected_generation)
            .await?;
        Ok(prepared)
    }

    pub fn run_if_project_current<T>(
        &self,
        expected_session: SessionId,
        expected_generation: Generation,
        action: impl FnOnce() -> T,
    ) -> Result<T, CommandError> {
        self.coordinator
            .run_if_current(expected_session, expected_generation, action)
            .ok_or_else(stale_project_session)
    }

    pub async fn execute_file_command(
        &self,
        expected_session: SessionId,
        expected_generation: Generation,
        kind: FileCommandKind,
        items: Vec<FileCommandItem>,
        conflicts: Vec<ConflictResolution>,
    ) -> Result<OperationStarted, CommandError> {
        let operations = self
            .active_operations(expected_session, expected_generation)
            .await?;
        operations
            .start(
                expected_session,
                expected_generation,
                kind,
                items,
                conflicts,
            )
            .await
            .map_err(operation_runtime_error)
    }

    pub async fn preflight_file_command(
        &self,
        command: FileCommand,
    ) -> Result<FileCommandPreflight, CommandError> {
        let operations = self
            .active_operations(command.session_id, command.generation)
            .await?;
        operations
            .preflight(command)
            .await
            .map_err(operation_runtime_error)
    }

    pub async fn operation_status(
        &self,
        expected_session: SessionId,
        expected_generation: Generation,
        batch_id: BatchId,
    ) -> Result<BatchProgress, CommandError> {
        self.active_operations(expected_session, expected_generation)
            .await?
            .status(batch_id)
            .map_err(operation_runtime_error)
    }

    pub async fn operation_results(
        &self,
        expected_session: SessionId,
        expected_generation: Generation,
        batch_id: BatchId,
        offset: usize,
        limit: usize,
    ) -> Result<BatchResultPage, CommandError> {
        let operations = self
            .active_operations(expected_session, expected_generation)
            .await?;
        operations
            .wait(batch_id)
            .await
            .map_err(operation_runtime_error)?;
        operations
            .results(batch_id, offset, limit)
            .map_err(operation_runtime_error)
    }

    pub async fn cancel_operation(
        &self,
        expected_session: SessionId,
        expected_generation: Generation,
        batch_id: BatchId,
    ) -> Result<bool, CommandError> {
        self.active_operations(expected_session, expected_generation)
            .await?
            .cancel(batch_id)
            .map_err(operation_runtime_error)
    }

    pub async fn wait_for_operation(&self, batch_id: BatchId) -> Result<(), CommandError> {
        let operations = {
            let session = self.session.lock().await;
            let session = session.as_ref().ok_or_else(project_not_open)?;
            session
                .operations
                .clone()
                .ok_or_else(project_read_only_operation)?
        };
        operations
            .wait(batch_id)
            .await
            .map_err(operation_runtime_error)
    }

    pub async fn undo_last_operation(
        &self,
        expected_session: SessionId,
        expected_generation: Generation,
    ) -> Result<Option<UndoReceipt>, CommandError> {
        self.undo_last(expected_session, expected_generation).await
    }

    async fn active_operations(
        &self,
        expected_session: SessionId,
        expected_generation: Generation,
    ) -> Result<Arc<OperationRuntime>, CommandError> {
        let session = self.session.lock().await;
        let session = session.as_ref().ok_or_else(project_not_open)?;
        validate_project_request(&session.active, expected_session, expected_generation)?;
        session
            .operations
            .clone()
            .ok_or_else(project_read_only_operation)
    }

    pub async fn undo_last(
        &self,
        expected_session: SessionId,
        expected_generation: Generation,
    ) -> Result<Option<UndoReceipt>, CommandError> {
        let (active, store, projection, undo_stack, write_lane, file_undo_port) = {
            let session = self.session.lock().await;
            let session = session.as_ref().ok_or_else(project_not_open)?;
            validate_project_request(&session.active, expected_session, expected_generation)?;
            if session.active.access != ProjectAccess::ReadWrite {
                return Err(project_read_only_operation());
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
                store,
                Arc::clone(&session.marker_projection),
                Arc::clone(&session.undo_stack),
                Arc::clone(&session.marker_lock),
                Arc::clone(&session.file_undo_port),
            )
        };
        let service = UndoService::new(
            active.session_id,
            active.access,
            active.root.clone(),
            Arc::new(LocalFileMutation),
            file_undo_port,
            store,
            projection,
            Arc::clone(&self.clock),
            undo_stack,
            write_lane,
        );
        let outcome = service
            .undo_last(expected_session)
            .await
            .map_err(CommandError::from)?;
        self.ensure_project_current(expected_session, expected_generation)
            .await?;
        Ok(outcome)
    }
}
