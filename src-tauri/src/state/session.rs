use super::*;

impl DesktopRuntime {
    fn prepare_review_services(
        &self,
        active: &ActiveProject,
        index: Arc<SessionIndex>,
        image: Arc<dyn ImagePort>,
    ) -> Result<
        (
            Arc<viewer_application::ReviewSessionService>,
            viewer_infrastructure::review::ReviewChangeLedger,
        ),
        CommandError,
    > {
        let review_changes = viewer_infrastructure::review::ReviewChangeLedger::default();
        let browse_index: Arc<dyn BrowseIndexPort> = index;
        let asset_catalog: Arc<dyn viewer_application::ReviewAssetCatalogPort> = Arc::new(
            viewer_infrastructure::review::IndexedReviewAssetCatalog::new(
                &active.root,
                browse_index,
                image,
                Arc::clone(&self.video_probe),
                review_changes.clone(),
            )
            .map_err(|_| {
                CommandError::from(viewer_application::ReviewSessionError::AssetUnavailable)
            })?,
        );
        let repositories: Arc<dyn viewer_application::ReviewRepositoryProviderPort> = Arc::new(
            viewer_infrastructure::review::ProjectReviewRepositoryProvider::new_with_access(
                &active.root,
                active.project_id,
                active.access,
            ),
        );
        Ok((
            Arc::new(viewer_application::ReviewSessionService::new(
                active.project_id,
                asset_catalog,
                repositories,
                Arc::clone(&self.clock),
            )),
            review_changes,
        ))
    }

    async fn prepare_organization_services(
        &self,
        active: &ActiveProject,
        index: Arc<SessionIndex>,
        portable_store: Option<Arc<PortableMarkerStore>>,
        portable_database_path: Option<&Path>,
        marker_lock: Arc<Mutex<()>>,
        undo_stack: Arc<StdMutex<UndoStack>>,
    ) -> Result<OrganizationServices, CommandError> {
        if active.access != ProjectAccess::ReadWrite {
            return Ok(OrganizationServices {
                file_undo_port: Arc::new(UnavailableFileUndoPort),
                operations: None,
                expected_changes: ExpectedChangeLedger::default(),
                recovery_report: None,
            });
        }
        let store = portable_store.ok_or_else(|| {
            CommandError::new(
                "portable_metadata_unavailable",
                ErrorCategory::Consistency,
                "项目审阅数据不可用，请重新打开项目。",
                true,
            )
        })?;
        let database_path = portable_database_path.ok_or_else(|| {
            CommandError::new(
                "portable_metadata_unavailable",
                ErrorCategory::Consistency,
                "项目审阅数据不可用，请重新打开项目。",
                true,
            )
        })?;
        let journal = Arc::new(OperationJournal::open(database_path).map_err(|_| {
            CommandError::new(
                "operation_journal_unavailable",
                ErrorCategory::Consistency,
                "文件操作记录不可用，请重新打开项目。",
                true,
            )
        })?);
        let browse_index: Arc<dyn BrowseIndexPort> = index.clone();
        let projection: Arc<dyn viewer_application::metadata::OperationProjectionPort> = index;
        let metadata: Arc<dyn PortableMetadataPort> = store;
        let volume: Arc<dyn viewer_application::VolumePort> = Arc::new(MacVolumePort);
        let commits = Arc::new(DesktopOperationCommitPort::new(
            active.root.clone(),
            active.generation,
            Arc::clone(&browse_index),
            Arc::clone(&projection),
            Arc::clone(&metadata),
            Arc::clone(&volume),
            Arc::clone(&self.clock),
            Arc::clone(&journal),
        ));
        let recovery_commits = Arc::new(DesktopOperationCommitPort::for_recovery(
            active.root.clone(),
            active.generation,
            Arc::clone(&browse_index),
            projection,
            metadata,
            Arc::clone(&volume),
            Arc::clone(&self.clock),
            Arc::clone(&journal),
        ));
        let trash: Arc<dyn viewer_application::TrashPort> =
            Arc::new(MacTrashPort::new(&active.root).map_err(|_| operation_backend_unavailable())?);
        let recovery = RecoveryService::new(
            &active.root,
            Arc::clone(&journal),
            Arc::new(LocalFileMutation),
            Arc::clone(&trash),
            Arc::clone(&self.clock),
            recovery_commits,
        )
        .map_err(|_| operation_backend_unavailable())?
        .recover_project()
        .await
        .map_err(|_| operation_backend_unavailable())?;
        let adapter = Arc::new(
            LocalFileCommandAdapter::new(
                &active.root,
                browse_index,
                journal,
                Arc::new(LocalFileMutation),
                trash,
                volume,
                Arc::clone(&self.clock),
                commits,
            )
            .map_err(|_| operation_backend_unavailable())?,
        );
        let expected_changes = adapter.expected_change_ledger();
        let port: Arc<dyn viewer_application::ports::LocalFileCommandPort> = adapter.clone();
        let service = Arc::new(FileCommandService::new_with_undo(
            active.session_id,
            active.access,
            Arc::clone(&self.coordinator),
            port,
            marker_lock,
            undo_stack,
        ));
        let operations = Arc::new(OperationRuntime::new(service, Arc::clone(&self.events)));
        Ok(OrganizationServices {
            file_undo_port: adapter_as_undo_port(adapter),
            operations: Some(operations),
            expected_changes,
            recovery_report: Some(RecoveryReportDto {
                recovered: u32::try_from(recovery.actions.len()).unwrap_or(u32::MAX),
                needs_user_review: u32::try_from(recovery.needs_user_review.len())
                    .unwrap_or(u32::MAX),
            })
            .filter(|report| report.recovered > 0 || report.needs_user_review > 0),
        })
    }

    pub async fn open_project(&self, root: &Path) -> Result<ProjectSnapshot, CommandError> {
        let mut session = self.session.lock().await;
        if session.is_some() {
            return Err(CommandError::from(ProjectOpenError::AlreadyOpen));
        }
        let prepared = self
            .project_service
            .prepare_open(root)
            .map_err(CommandError::from)?;
        let portable_metadata = match PortableProjectMetadata::open(
            &prepared.root,
            prepared.access,
            self.clock.unix_millis(),
        ) {
            Ok(metadata) => metadata,
            Err(error) => {
                let _ = self.project_service.abort_open();
                return Err(error.into());
            }
        };
        let portable_database_path = portable_metadata.database_path().map(Path::to_path_buf);
        let portable_store = match portable_metadata.database_path() {
            Some(path) => match PortableMarkerStore::open(path, portable_metadata.is_writable()) {
                Ok(store) => Some(Arc::new(store)),
                Err(error) => {
                    let _ = self.project_service.abort_open();
                    return Err(error.into());
                }
            },
            None => None,
        };
        let active = match self
            .project_service
            .activate_prepared(prepared, portable_metadata.project_id())
        {
            Ok(active) => active,
            Err(error) => {
                let _ = self.project_service.abort_open();
                return Err(error.into());
            }
        };
        let cache = match SessionCache::create_in(&self.cache_base, active.session_id) {
            Ok(cache) => Arc::new(cache),
            Err(error) => {
                let _ = self.project_service.close();
                return Err(error.into());
            }
        };
        let index = match SessionIndex::open(cache.index_path()) {
            Ok(index) => Arc::new(index),
            Err(error) => {
                let _ = cache.cleanup();
                let _ = self.project_service.close();
                return Err(error.into());
            }
        };
        let image = match self.image_factory.create(&cache.image_root()) {
            Ok(image) => image,
            Err(error) => {
                drop(index);
                let _ = cache.cleanup();
                let _ = self.project_service.close();
                return Err(error.into());
            }
        };
        let video_index = Arc::new(VideoIndexRuntime::new(
            active.clone(),
            Arc::clone(&self.coordinator),
            Arc::clone(&index),
            Arc::clone(&self.video_probe),
            Arc::clone(&self.derived_scheduler),
            Arc::clone(&self.events),
        ));
        let marker_projection = self.marker_projection_factory.create(Arc::clone(&index));
        let mut snapshot = ProjectSnapshot::from(&active);
        let active_session_id = active.session_id;
        let scan_task_id = TaskId::new();
        let marker_lock = Arc::new(Mutex::new(()));
        let undo_stack = Arc::new(StdMutex::new(UndoStack::new(active_session_id)));
        let (review, review_changes) =
            match self.prepare_review_services(&active, Arc::clone(&index), Arc::clone(&image)) {
                Ok(services) => services,
                Err(error) => {
                    cancel_video_worker_before_session_teardown(video_index.as_ref()).await;
                    image.cancel_session(active.session_id).await;
                    drop(marker_projection);
                    drop(index);
                    drop(portable_store);
                    let _ = cache.cleanup();
                    let _ = self.project_service.close();
                    return Err(error);
                }
            };
        let _ = review.inspect().await;
        let organization = match self
            .prepare_organization_services(
                &active,
                Arc::clone(&index),
                portable_store.clone(),
                portable_database_path.as_deref(),
                Arc::clone(&marker_lock),
                Arc::clone(&undo_stack),
            )
            .await
        {
            Ok(services) => services,
            Err(error) => {
                review.shutdown().await;
                cancel_video_worker_before_session_teardown(video_index.as_ref()).await;
                image.cancel_session(active.session_id).await;
                drop(marker_projection);
                drop(index);
                drop(portable_store);
                let _ = cache.cleanup();
                let _ = self.project_service.close();
                return Err(error);
            }
        };
        snapshot.recovery_report = organization.recovery_report;
        let OrganizationServices {
            file_undo_port,
            operations,
            expected_changes,
            ..
        } = organization;
        let (scan_ready, scan_ready_rx) = tokio::sync::watch::channel(false);
        let watcher_result = (|| {
            let case_sensitive = MacVolumePort
                .is_case_sensitive(&active.root)
                .map_err(|_| operation_backend_unavailable())?;
            let marker_metadata = portable_store
                .clone()
                .map(|store| store as Arc<dyn PortableMetadataPort>);
            let reconciler = Arc::new(
                ProjectReconciler::new(
                    &active.root,
                    Arc::clone(&self.coordinator),
                    Arc::clone(&index),
                    marker_metadata,
                    Arc::clone(&self.clock),
                    Arc::clone(&marker_lock),
                    case_sensitive,
                )
                .map_err(|_| operation_backend_unavailable())?,
            );
            WatcherRuntime::start(
                active.root.clone(),
                active.session_id,
                active.generation,
                Arc::new(MacWatcherPort),
                expected_changes,
                reconciler,
                Arc::clone(&self.clock),
                Arc::clone(&self.events),
                scan_ready_rx,
                Some(WatcherDerivedServices {
                    active: active.clone(),
                    coordinator: Arc::clone(&self.coordinator),
                    index: Arc::clone(&index),
                    image: Arc::clone(&image),
                    events: Arc::clone(&self.events),
                    scheduler: Arc::clone(&self.derived_scheduler),
                    video_index: Arc::clone(&video_index),
                    review_changes: review_changes.clone(),
                }),
            )
            .map_err(|_| operation_backend_unavailable())
        })();
        let watcher = match watcher_result {
            Ok(watcher) => Some(watcher),
            Err(error) => {
                review.shutdown().await;
                cancel_video_worker_before_session_teardown(video_index.as_ref()).await;
                drop(operations);
                drop(file_undo_port);
                image.cancel_session(active.session_id).await;
                drop(marker_projection);
                drop(index);
                drop(portable_store);
                let _ = cache.cleanup();
                let _ = self.project_service.close();
                return Err(error);
            }
        };
        let scan_task = tokio::spawn(run_scan(
            active.clone(),
            scan_task_id,
            ScanServices {
                scanner: Arc::clone(&self.scanner),
                coordinator: Arc::clone(&self.coordinator),
                index: Arc::clone(&index),
                image: Arc::clone(&image),
                events: Arc::clone(&self.events),
                portable_store: portable_store.clone(),
                marker_lock: Arc::clone(&marker_lock),
                scan_ready,
                derived_scheduler: Arc::clone(&self.derived_scheduler),
                video_index: Arc::clone(&video_index),
            },
        ));
        *session = Some(DesktopSession {
            active,
            snapshot: snapshot.clone(),
            cache,
            index,
            portable_store,
            marker_projection,
            marker_lock,
            undo_stack,
            file_undo_port,
            operations,
            watcher,
            search_revision: Arc::new(AtomicU64::new(0)),
            image,
            scan_task_id,
            scan_task: Some(scan_task),
            video_index,
            review,
            review_changes,
        });
        self.active_image_session.set(Some(active_session_id));
        Ok(snapshot)
    }

    pub async fn close_project(&self) -> Result<(), CommandError> {
        let _video_project_close = self.video_project_gate.write().await;
        let Some(mut session) = self.session.lock().await.take() else {
            return Ok(());
        };
        // Invalidate the session before the first cleanup await. Native actions
        // guarded by this token must not start after close has taken ownership.
        self.coordinator.cancel_session(session.active.session_id);
        self.active_image_session.set(None);
        session.review.shutdown().await;
        session.review_changes.clear();
        let video_lifecycle = self
            .video_lifecycle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        let playback_result = match video_lifecycle {
            Some(lifecycle) => lifecycle.close_video().await,
            None => Ok(()),
        };
        self.image_registry
            .remove_session(session.active.session_id);
        if let Some(operations) = session.operations.take() {
            operations.cancel_and_wait_active().await;
        }
        let video_result = session.video_index.cancel_and_wait().await;
        if let Some(mut watcher) = session.watcher.take() {
            watcher.stop().await;
        }
        // Taking `self.session` is the close commit point. Cleanup failures
        // after this point must never resurrect a frontend session whose
        // backend resources are already being dismantled.
        let _ = self.project_service.close();
        let marker_lock = Arc::clone(&session.marker_lock);
        let _marker_guard = marker_lock.lock().await;
        session
            .undo_stack
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .close_session(session.active.session_id);
        session
            .image
            .cancel_session(session.active.session_id)
            .await;
        if let Some(scan_task) = session.scan_task.take() {
            scan_task.abort();
            let _ = scan_task.await;
        }
        drop(session.marker_projection);
        drop(session.portable_store.take());
        drop(session.index);
        // The session is already in its closed terminal state, but a normal
        // close must still report a cache cleanup failure. Callers deciding
        // whether an application exit may proceed must not treat retained
        // previews and the temporary SQLite index as a successful shutdown.
        let cache_result = session.cache.cleanup().map_err(|_| {
            CommandError::new(
                "project_closed_cache_cleanup_failed",
                ErrorCategory::Environment,
                "项目已关闭，但临时缓存未能清除；退出 Viewer 后将重试。",
                true,
            )
        });
        cache_result?;
        video_result?;
        playback_result.map_err(CommandError::from)
    }

    /// Finalizes shutdown when the operating system has committed to ending
    /// the process.
    ///
    /// macOS can begin terminating the application without first delivering a
    /// preventable Tauri `ExitRequested` event. The final `RunEvent::Exit`
    /// therefore closes any active session and then performs a last-chance
    /// sweep of Viewer-owned session directories.
    pub async fn finalize_process_exit(&self) -> Result<(), CommandError> {
        let close_result = self.close_project().await;
        let sweep_result = self.cleanup_session_caches_for_process_exit().map(|_| ());
        close_result?;
        sweep_result
    }

    pub fn cleanup_session_caches_for_process_exit(&self) -> Result<usize, CommandError> {
        SessionCache::cleanup_stale(&self.cache_base, None).map_err(Into::into)
    }

    pub async fn request_close(
        &self,
        choice: Option<CloseChoice>,
    ) -> Result<CloseRequestOutcome, CommandError> {
        self.request_close_for(choice, CloseTarget::Project).await
    }

    pub async fn request_close_for(
        &self,
        choice: Option<CloseChoice>,
        target: CloseTarget,
    ) -> Result<CloseRequestOutcome, CommandError> {
        let active = {
            let session = self.session.lock().await;
            session.as_ref().and_then(|session| {
                session.operations.as_ref().and_then(|operations| {
                    operations.active_batch().map(|batch_id| {
                        (
                            Arc::clone(operations),
                            session.active.session_id,
                            session.active.generation,
                            batch_id,
                        )
                    })
                })
            })
        };
        let Some((operations, session_id, generation, batch_id)) = active else {
            self.close_project().await?;
            return Ok(CloseRequestOutcome::Closed);
        };
        match choice {
            Some(CloseChoice::Wait) => {
                let _ = operations.wait(batch_id).await;
                self.close_project().await?;
                Ok(CloseRequestOutcome::Closed)
            }
            Some(CloseChoice::CancelPending) => {
                let _ = operations.cancel(batch_id);
                let _ = operations.wait(batch_id).await;
                self.close_project().await?;
                Ok(CloseRequestOutcome::Closed)
            }
            None | Some(CloseChoice::Stay) => {
                self.events
                    .emit_close_blocked(session_id, generation, batch_id, target);
                Ok(CloseRequestOutcome::Stayed)
            }
        }
    }

    pub async fn wait_for_scan(&self) -> Result<(), CommandError> {
        let scan_task = {
            let mut session = self.session.lock().await;
            let Some(session) = session.as_mut() else {
                return Err(CommandError::new(
                    "project_not_open",
                    crate::error::ErrorCategory::Conflict,
                    "请先打开一个项目。",
                    false,
                ));
            };
            session.scan_task.take()
        };
        let Some(scan_task) = scan_task else {
            return Ok(());
        };
        match scan_task.await {
            Ok(result) => result,
            Err(error) if error.is_cancelled() => Ok(()),
            Err(_) => Err(CommandError::new(
                "scan_worker_failed",
                crate::error::ErrorCategory::Internal,
                "项目扫描意外终止，请重新打开项目。",
                true,
            )),
        }
    }

    pub async fn cancel_task(&self, task_id: &str) -> bool {
        let scan_task = {
            let mut session = self.session.lock().await;
            let Some(session) = session.as_mut() else {
                return false;
            };
            if session.scan_task_id.to_string() != task_id {
                return false;
            }
            session.scan_task.take()
        };
        let Some(scan_task) = scan_task else {
            return false;
        };
        scan_task.abort();
        let _ = scan_task.await;
        true
    }

    pub async fn snapshot(&self) -> Option<ProjectSnapshot> {
        self.session
            .lock()
            .await
            .as_ref()
            .map(|session| session.snapshot.clone())
    }

    pub async fn reveal_project_in_file_manager(&self) -> Result<(), CommandError> {
        let root = self
            .session
            .lock()
            .await
            .as_ref()
            .map(|session| session.active.root.clone())
            .ok_or_else(project_not_open)?;
        tokio::task::spawn_blocking(move || viewer_platform_macos::reveal_in_file_manager(&root))
            .await
            .map_err(|_| internal_command_error())?
            .map_err(|_| {
                CommandError::new(
                    "file_manager_unavailable",
                    ErrorCategory::Environment,
                    "无法在文件管理器中显示当前项目。",
                    true,
                )
            })
    }

    pub(super) async fn ensure_project_current(
        &self,
        expected_session: SessionId,
        expected_generation: Generation,
    ) -> Result<(), CommandError> {
        let session = self.session.lock().await;
        let session = session.as_ref().ok_or_else(stale_project_session)?;
        validate_project_request(&session.active, expected_session, expected_generation)
    }

    pub(super) async fn ensure_session_active(
        &self,
        expected: SessionId,
    ) -> Result<(), CommandError> {
        let session = self.session.lock().await;
        if session
            .as_ref()
            .is_some_and(|session| session.active.session_id == expected)
        {
            Ok(())
        } else {
            Err(CommandError::new(
                "text_preview_cancelled",
                ErrorCategory::Conflict,
                "文本预览已取消。",
                true,
            ))
        }
    }

    pub async fn resources_ready(&self) -> bool {
        let session = self.session.lock().await;
        let Some(session) = session.as_ref() else {
            return false;
        };
        session.active.root.is_dir()
            && session.cache.root().is_dir()
            && session.index.directory_children(None).is_ok()
    }
}
