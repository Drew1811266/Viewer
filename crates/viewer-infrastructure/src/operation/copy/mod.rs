mod evidence;
mod file_reference;
mod local;
mod placement;
mod staged;

pub use local::LocalFileMutation;

pub(crate) use evidence::hash_file_sync;
#[cfg(not(target_os = "macos"))]
pub(crate) use local::create_temporary_sync;
pub(crate) use placement::sync_parent;

#[cfg(all(test, target_os = "macos"))]
use file_reference::{
    bind_file_reference, bind_open_file_reference_with_hook, file_snapshot, resolve_file_reference,
    unlink_file_reference,
};
#[cfg(all(test, target_os = "macos"))]
use local::{
    COPY_BUFFER_BYTES, MacStagedCopyLease, bound_copy_cleanup_error_with_sync,
    build_staged_copy_cancellable_verified_sync_with_hooks,
    create_copy_and_hash_cancellable_verified_sync_with_hooks,
    create_staged_copy_cancellable_verified_sync_with_hooks,
    place_bound_staged_copy_sync_with_hooks, rename_verified_sync_with_hook,
    rename_verified_sync_with_hooks,
};
#[cfg(all(test, unix))]
use local::{create_copy_and_hash_cancellable_verified_sync_with_hook, snapshot_sync};
#[cfg(all(test, unix))]
use std::fs;
#[cfg(all(test, target_os = "macos"))]
use std::{
    fs::{File, OpenOptions},
    path::Path,
};
#[cfg(all(test, unix))]
use viewer_application::{
    FileOperationError, file_commands::FileCommandCancellation, watcher::FileIdentity,
};

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::MetadataExt;

    #[test]
    fn bound_copy_keeps_the_created_leaf_open_when_its_name_is_replaced_by_a_hardlink() {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let source = root.join("source.bin");
        let temporary = root.join(".viewer-copy-test.part");
        let victim = root.join("victim.bin");
        fs::write(&source, b"selected bytes").unwrap();
        fs::write(&victim, b"victim sentinel").unwrap();
        let parent_metadata = fs::metadata(&root).unwrap();
        let parent_identity = FileIdentity {
            volume: parent_metadata.dev(),
            file: parent_metadata.ino(),
        };
        let source_snapshot = snapshot_sync(&source).unwrap();

        let evidence = create_copy_and_hash_cancellable_verified_sync_with_hook(
            &source,
            &temporary,
            &FileCommandCancellation::default(),
            &source_snapshot,
            parent_identity,
            parent_identity,
            || {
                fs::remove_file(&temporary).unwrap();
                fs::hard_link(&victim, &temporary).unwrap();
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(fs::read(&victim).unwrap(), b"victim sentinel");
        assert_eq!(fs::read(&temporary).unwrap(), b"victim sentinel");
        assert_ne!(snapshot_sync(&temporary).unwrap(), evidence.snapshot);
        assert_eq!(evidence.snapshot.len, b"selected bytes".len() as u64);
        assert_eq!(evidence.hash, *blake3::hash(b"selected bytes").as_bytes());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn identity_bound_cleanup_unlinks_the_created_leaf_after_its_name_is_swapped() {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let temporary = root.join(".viewer-copy-test.part");
        let parked = root.join("parked-created-temp.part");
        let replacement = root.join("replacement.bin");
        fs::write(&temporary, b"created temporary").unwrap();
        fs::write(&replacement, b"replacement sentinel").unwrap();
        let expected = snapshot_sync(&temporary).unwrap();
        let reference = bind_file_reference(&temporary, &expected).unwrap();

        fs::rename(&temporary, &parked).unwrap();
        fs::rename(&replacement, &temporary).unwrap();
        unlink_file_reference(&reference, &temporary).unwrap();

        assert_eq!(fs::read(&temporary).unwrap(), b"replacement sentinel");
        assert!(!parked.exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn open_temporary_binding_retries_when_parent_moves_between_fd_path_and_fsref() {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_root = fs::canonicalize(outside.path()).unwrap();
        let parent = root.join("destination");
        fs::create_dir(&parent).unwrap();
        let temporary = parent.join(".viewer-copy-test.part");
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&temporary)
            .unwrap();
        let expected = file_snapshot(&file).unwrap();
        let moved_parent = outside_root.join("moved-destination");

        let reference = bind_open_file_reference_with_hook(
            &file,
            &expected,
            &temporary,
            |attempt, _resolved| {
                if attempt == 0 {
                    fs::rename(&parent, &moved_parent).unwrap();
                }
            },
        )
        .unwrap();

        assert_eq!(
            resolve_file_reference(&reference, &temporary).unwrap(),
            moved_parent.join(".viewer-copy-test.part")
        );
        unlink_file_reference(&reference, &temporary).unwrap();
        assert!(!moved_parent.join(".viewer-copy-test.part").exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn dropping_an_unplaced_staged_copy_unlinks_its_identity_after_parent_reparent() {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_root = fs::canonicalize(outside.path()).unwrap();
        let destination_parent = root.join("destination");
        fs::create_dir(&destination_parent).unwrap();
        let source = root.join("source.bin");
        fs::write(&source, b"complete staged bytes").unwrap();
        let temporary = destination_parent.join(".viewer-copy-test.part");
        let moved_parent = outside_root.join("moved-destination");
        let source_metadata = fs::metadata(&root).unwrap();
        let destination_metadata = fs::metadata(&destination_parent).unwrap();
        let source_snapshot = snapshot_sync(&source).unwrap();
        let staged = create_staged_copy_cancellable_verified_sync_with_hooks(
            &source,
            &temporary,
            &FileCommandCancellation::default(),
            &source_snapshot,
            FileIdentity {
                volume: source_metadata.dev(),
                file: source_metadata.ino(),
            },
            FileIdentity {
                volume: destination_metadata.dev(),
                file: destination_metadata.ino(),
            },
            || Ok(()),
            || Ok(()),
            || Ok(()),
        )
        .unwrap();

        fs::rename(&destination_parent, &moved_parent).unwrap();
        drop(staged);

        assert!(!moved_parent.join(".viewer-copy-test.part").exists());
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn successful_staged_placement_disarms_cleanup_and_keeps_destination() {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let source = root.join("source.bin");
        let temporary = root.join(".viewer-copy-test.part");
        let destination = root.join("destination.bin");
        fs::write(&source, b"complete staged bytes").unwrap();
        let parent_metadata = fs::metadata(&root).unwrap();
        let parent_identity = FileIdentity {
            volume: parent_metadata.dev(),
            file: parent_metadata.ino(),
        };
        let source_snapshot = snapshot_sync(&source).unwrap();
        let staged = create_staged_copy_cancellable_verified_sync_with_hooks(
            &source,
            &temporary,
            &FileCommandCancellation::default(),
            &source_snapshot,
            parent_identity,
            parent_identity,
            || Ok(()),
            || Ok(()),
            || Ok(()),
        )
        .unwrap();

        staged.place(&destination, parent_identity).await.unwrap();

        assert_eq!(fs::read(&destination).unwrap(), b"complete staged bytes");
        assert!(!temporary.exists());
    }

    #[cfg(target_os = "macos")]
    fn staged_copy_lease_for_test(
        source: &Path,
        temporary: &Path,
        source_parent_identity: FileIdentity,
        temporary_parent_identity: FileIdentity,
    ) -> Box<MacStagedCopyLease> {
        let source_snapshot = snapshot_sync(source).unwrap();
        let build = build_staged_copy_cancellable_verified_sync_with_hooks(
            source,
            temporary,
            &FileCommandCancellation::default(),
            &source_snapshot,
            source_parent_identity,
            temporary_parent_identity,
            || Ok(()),
            || Ok(()),
            || Ok(()),
        )
        .unwrap();
        Box::new(build.lease)
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn post_rename_sync_error_keeps_the_verified_destination_for_recovery() {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let source = root.join("source.bin");
        let temporary = root.join(".viewer-copy-test.part");
        let destination = root.join("destination.bin");
        fs::write(&source, b"complete staged bytes").unwrap();
        let parent_metadata = fs::metadata(&root).unwrap();
        let parent_identity = FileIdentity {
            volume: parent_metadata.dev(),
            file: parent_metadata.ino(),
        };
        let lease =
            staged_copy_lease_for_test(&source, &temporary, parent_identity, parent_identity);
        let sync_error = FileOperationError::Io {
            action: "injected post-rename directory sync failure",
            path: destination.clone(),
            message: "injected failure".into(),
        };

        let result = place_bound_staged_copy_sync_with_hooks(
            lease,
            &destination,
            parent_identity,
            || Err(sync_error.clone()),
            || Ok(()),
        );

        assert_eq!(result.unwrap_err(), sync_error);
        assert_eq!(fs::read(&destination).unwrap(), b"complete staged bytes");
        assert!(!temporary.exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn post_rename_validation_error_keeps_the_verified_destination_for_recovery() {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let source = root.join("source.bin");
        let temporary = root.join(".viewer-copy-test.part");
        let destination = root.join("destination.bin");
        fs::write(&source, b"complete staged bytes").unwrap();
        let parent_metadata = fs::metadata(&root).unwrap();
        let parent_identity = FileIdentity {
            volume: parent_metadata.dev(),
            file: parent_metadata.ino(),
        };
        let lease =
            staged_copy_lease_for_test(&source, &temporary, parent_identity, parent_identity);

        let result = place_bound_staged_copy_sync_with_hooks(
            lease,
            &destination,
            parent_identity,
            || Ok(()),
            || Err(FileOperationError::OutsideProject),
        );

        assert_eq!(result.unwrap_err(), FileOperationError::OutsideProject);
        assert_eq!(fs::read(&destination).unwrap(), b"complete staged bytes");
        assert!(!temporary.exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn post_rename_parent_escape_still_cleans_the_bound_destination_identity() {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_root = fs::canonicalize(outside.path()).unwrap();
        let destination_parent = root.join("destination");
        fs::create_dir(&destination_parent).unwrap();
        let source = root.join("source.bin");
        let temporary = destination_parent.join(".viewer-copy-test.part");
        let destination = destination_parent.join("destination.bin");
        let moved_parent = outside_root.join("moved-destination");
        fs::write(&source, b"complete staged bytes").unwrap();
        let source_parent_metadata = fs::metadata(&root).unwrap();
        let destination_parent_metadata = fs::metadata(&destination_parent).unwrap();
        let source_parent_identity = FileIdentity {
            volume: source_parent_metadata.dev(),
            file: source_parent_metadata.ino(),
        };
        let destination_parent_identity = FileIdentity {
            volume: destination_parent_metadata.dev(),
            file: destination_parent_metadata.ino(),
        };
        let lease = staged_copy_lease_for_test(
            &source,
            &temporary,
            source_parent_identity,
            destination_parent_identity,
        );

        let result = place_bound_staged_copy_sync_with_hooks(
            lease,
            &destination,
            destination_parent_identity,
            || Ok(()),
            || {
                fs::rename(&destination_parent, &moved_parent).unwrap();
                Ok(())
            },
        );

        assert_eq!(result.unwrap_err(), FileOperationError::OutsideProject);
        assert!(!temporary.exists());
        assert!(!moved_parent.join("destination.bin").exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn post_rename_leaf_escape_cleans_the_bound_identity_and_preserves_its_replacement() {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_root = fs::canonicalize(outside.path()).unwrap();
        let source = root.join("source.bin");
        let temporary = root.join(".viewer-copy-test.part");
        let destination = root.join("destination.bin");
        let escaped_destination = outside_root.join("escaped-destination.bin");
        fs::write(&source, b"complete staged bytes").unwrap();
        let parent_metadata = fs::metadata(&root).unwrap();
        let parent_identity = FileIdentity {
            volume: parent_metadata.dev(),
            file: parent_metadata.ino(),
        };
        let lease =
            staged_copy_lease_for_test(&source, &temporary, parent_identity, parent_identity);

        let result = place_bound_staged_copy_sync_with_hooks(
            lease,
            &destination,
            parent_identity,
            || Ok(()),
            || {
                fs::rename(&destination, &escaped_destination).unwrap();
                fs::write(&destination, b"replacement sentinel").unwrap();
                Ok(())
            },
        );

        assert_eq!(result.unwrap_err(), FileOperationError::OutsideProject);
        assert_eq!(fs::read(&destination).unwrap(), b"replacement sentinel");
        assert!(!temporary.exists());
        assert!(!escaped_destination.exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn registered_copy_error_removes_the_partially_written_created_identity() {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let source = root.join("source.bin");
        let temporary = root.join(".viewer-copy-test.part");
        fs::write(&source, b"selected bytes").unwrap();
        let parent_metadata = fs::metadata(&root).unwrap();
        let parent_identity = FileIdentity {
            volume: parent_metadata.dev(),
            file: parent_metadata.ino(),
        };
        let source_snapshot = snapshot_sync(&source).unwrap();

        let result = create_copy_and_hash_cancellable_verified_sync_with_hook(
            &source,
            &temporary,
            &FileCommandCancellation::default(),
            &source_snapshot,
            parent_identity,
            parent_identity,
            || {
                fs::write(&temporary, b"partial bytes").unwrap();
                Err(FileOperationError::Cancelled)
            },
        );

        assert!(matches!(result, Err(FileOperationError::Cancelled)));
        assert!(!temporary.exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn registered_copy_stops_and_unlinks_when_destination_parent_is_reparented_outside_mid_copy() {
        use std::sync::{Arc, Barrier, Mutex};

        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_root = fs::canonicalize(outside.path()).unwrap();
        let destination_parent = root.join("destination");
        fs::create_dir(&destination_parent).unwrap();
        let source = root.join("source.bin");
        let source_len = (COPY_BUFFER_BYTES as u64) * 32;
        File::create(&source).unwrap().set_len(source_len).unwrap();
        let temporary = destination_parent.join(".viewer-copy-test.part");
        let moved_parent = outside_root.join("moved-destination");
        let source_metadata = fs::metadata(&root).unwrap();
        let destination_metadata = fs::metadata(&destination_parent).unwrap();
        let source_snapshot = snapshot_sync(&source).unwrap();
        let ready = Arc::new(Barrier::new(2));
        let observer_ready = Arc::clone(&ready);
        let observed_temporary = temporary.clone();
        let observed_parent = destination_parent.clone();
        let observed_moved_parent = moved_parent.clone();
        let reparent = Arc::new(Mutex::new(None));
        let reparent_slot = Arc::clone(&reparent);

        let result = create_copy_and_hash_cancellable_verified_sync_with_hook(
            &source,
            &temporary,
            &FileCommandCancellation::default(),
            &source_snapshot,
            FileIdentity {
                volume: source_metadata.dev(),
                file: source_metadata.ino(),
            },
            FileIdentity {
                volume: destination_metadata.dev(),
                file: destination_metadata.ino(),
            },
            || {
                let handle = std::thread::spawn(move || {
                    observer_ready.wait();
                    loop {
                        let copied_len = fs::metadata(&observed_temporary).unwrap().len();
                        if copied_len >= COPY_BUFFER_BYTES as u64 && copied_len < source_len {
                            fs::rename(&observed_parent, &observed_moved_parent).unwrap();
                            return copied_len;
                        }
                        if copied_len >= source_len {
                            return copied_len;
                        }
                        std::thread::yield_now();
                    }
                });
                *reparent_slot.lock().unwrap() = Some(handle);
                ready.wait();
                Ok(())
            },
        );
        let reparented_at = reparent.lock().unwrap().take().unwrap().join().unwrap();

        assert!(
            reparented_at < source_len,
            "the fixture must move the parent before copying completes"
        );
        assert!(matches!(result, Err(FileOperationError::OutsideProject)));
        assert!(!temporary.exists());
        assert!(!moved_parent.join(".viewer-copy-test.part").exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn registered_copy_unlinks_the_created_identity_when_parent_moves_outside_before_binding() {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_root = fs::canonicalize(outside.path()).unwrap();
        let destination_parent = root.join("destination");
        fs::create_dir(&destination_parent).unwrap();
        let source = root.join("source.bin");
        fs::write(&source, b"selected bytes").unwrap();
        let temporary = destination_parent.join(".viewer-copy-test.part");
        let moved_parent = outside_root.join("moved-destination");
        let source_metadata = fs::metadata(&root).unwrap();
        let destination_metadata = fs::metadata(&destination_parent).unwrap();
        let source_snapshot = snapshot_sync(&source).unwrap();

        let result = create_copy_and_hash_cancellable_verified_sync_with_hooks(
            &source,
            &temporary,
            &FileCommandCancellation::default(),
            &source_snapshot,
            FileIdentity {
                volume: source_metadata.dev(),
                file: source_metadata.ino(),
            },
            FileIdentity {
                volume: destination_metadata.dev(),
                file: destination_metadata.ino(),
            },
            || {
                fs::rename(&destination_parent, &moved_parent).unwrap();
                Ok(())
            },
            || Ok(()),
            || panic!("copy hook must not run after the parent leaves the project"),
        );

        assert!(matches!(result, Err(FileOperationError::OutsideProject)));
        assert!(!temporary.exists());
        assert!(!moved_parent.join(".viewer-copy-test.part").exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn registered_copy_prebind_name_swap_cleans_created_identity_and_preserves_replacement() {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let source = root.join("source.bin");
        let temporary = root.join(".viewer-copy-test.part");
        let parked = root.join("parked-created-temp.part");
        fs::write(&source, b"selected bytes").unwrap();
        let parent_metadata = fs::metadata(&root).unwrap();
        let parent_identity = FileIdentity {
            volume: parent_metadata.dev(),
            file: parent_metadata.ino(),
        };
        let source_snapshot = snapshot_sync(&source).unwrap();

        let result = create_copy_and_hash_cancellable_verified_sync_with_hooks(
            &source,
            &temporary,
            &FileCommandCancellation::default(),
            &source_snapshot,
            parent_identity,
            parent_identity,
            || {
                fs::rename(&temporary, &parked).unwrap();
                fs::write(&temporary, b"replacement sentinel").unwrap();
                Ok(())
            },
            || Ok(()),
            || panic!("copy hook must not run after setup failure"),
        );

        match result {
            Err(FileOperationError::RegisteredTemporaryCleanupRequired { primary, cleanup }) => {
                assert_eq!(*primary, FileOperationError::IdentityChanged);
                assert_eq!(*cleanup, FileOperationError::IdentityChanged);
            }
            other => panic!("expected replacement-preserving cleanup result, got {other:?}"),
        }
        assert_eq!(fs::read(&temporary).unwrap(), b"replacement sentinel");
        assert!(!parked.exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn registered_copy_bound_setup_error_cleans_identity_and_preserves_primary() {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let source = root.join("source.bin");
        let temporary = root.join(".viewer-copy-test.part");
        fs::write(&source, b"selected bytes").unwrap();
        let parent_metadata = fs::metadata(&root).unwrap();
        let parent_identity = FileIdentity {
            volume: parent_metadata.dev(),
            file: parent_metadata.ino(),
        };
        let source_snapshot = snapshot_sync(&source).unwrap();
        let setup_error = FileOperationError::Io {
            action: "injected directory sync failure",
            path: temporary.clone(),
            message: "injected failure".into(),
        };

        let result = create_copy_and_hash_cancellable_verified_sync_with_hooks(
            &source,
            &temporary,
            &FileCommandCancellation::default(),
            &source_snapshot,
            parent_identity,
            parent_identity,
            || Ok(()),
            || Err(setup_error.clone()),
            || panic!("copy hook must not run after bound setup failure"),
        );

        assert_eq!(result, Err(setup_error));
        assert!(!temporary.exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn registered_copy_error_never_removes_a_replacement_at_the_temporary_name() {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let source = root.join("source.bin");
        let temporary = root.join(".viewer-copy-test.part");
        let parked = root.join("parked-created-temp.part");
        let replacement = root.join("replacement.bin");
        fs::write(&source, b"selected bytes").unwrap();
        fs::write(&replacement, b"replacement sentinel").unwrap();
        let parent_metadata = fs::metadata(&root).unwrap();
        let parent_identity = FileIdentity {
            volume: parent_metadata.dev(),
            file: parent_metadata.ino(),
        };
        let source_snapshot = snapshot_sync(&source).unwrap();

        let result = create_copy_and_hash_cancellable_verified_sync_with_hook(
            &source,
            &temporary,
            &FileCommandCancellation::default(),
            &source_snapshot,
            parent_identity,
            parent_identity,
            || {
                fs::rename(&temporary, &parked).unwrap();
                fs::rename(&replacement, &temporary).unwrap();
                Err(FileOperationError::Cancelled)
            },
        );

        match result {
            Err(FileOperationError::RegisteredTemporaryCleanupRequired { primary, cleanup }) => {
                assert_eq!(*primary, FileOperationError::Cancelled);
                assert_eq!(*cleanup, FileOperationError::IdentityChanged);
            }
            other => panic!("expected replacement cleanup obligation, got {other:?}"),
        }
        assert_eq!(fs::read(&temporary).unwrap(), b"replacement sentinel");
        assert!(!parked.exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn registered_copy_cleanup_sync_failure_retains_the_recovery_obligation() {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let temporary = root.join(".viewer-copy-test.part");
        fs::write(&temporary, b"partial bytes").unwrap();
        let temporary_snapshot = snapshot_sync(&temporary).unwrap();
        let temporary_reference = bind_file_reference(&temporary, &temporary_snapshot).unwrap();
        let sync_error = FileOperationError::Io {
            action: "injected cleanup directory sync failure",
            path: root,
            message: "injected failure".into(),
        };

        let result = bound_copy_cleanup_error_with_sync(
            &temporary_reference,
            &temporary,
            FileOperationError::Cancelled,
            || Err(sync_error.clone()),
        );

        match result {
            FileOperationError::RegisteredTemporaryCleanupRequired { primary, cleanup } => {
                assert_eq!(*primary, FileOperationError::Cancelled);
                assert_eq!(*cleanup, sync_error);
            }
            other => panic!("expected durable cleanup obligation, got {other:?}"),
        }
        assert!(!temporary.exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn registered_copy_cleanup_failure_preserves_primary_and_owned_temporary() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let source = root.join("source.bin");
        let temporary = root.join(".viewer-copy-test.part");
        fs::write(&source, b"selected bytes").unwrap();
        let parent_metadata = fs::metadata(&root).unwrap();
        let parent_identity = FileIdentity {
            volume: parent_metadata.dev(),
            file: parent_metadata.ino(),
        };
        let source_snapshot = snapshot_sync(&source).unwrap();

        let result = create_copy_and_hash_cancellable_verified_sync_with_hook(
            &source,
            &temporary,
            &FileCommandCancellation::default(),
            &source_snapshot,
            parent_identity,
            parent_identity,
            || {
                fs::write(&temporary, b"partial bytes").unwrap();
                fs::set_permissions(&root, fs::Permissions::from_mode(0o500)).unwrap();
                Err(FileOperationError::Cancelled)
            },
        );
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();

        match result {
            Err(FileOperationError::RegisteredTemporaryCleanupRequired { primary, cleanup }) => {
                assert_eq!(*primary, FileOperationError::Cancelled);
                assert_ne!(*cleanup, FileOperationError::Cancelled);
            }
            other => panic!("expected structured cleanup failure, got {other:?}"),
        }
        assert_eq!(fs::read(&temporary).unwrap(), b"partial bytes");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn verified_rename_rolls_back_a_leaf_swap_after_the_validated_fd_is_opened() {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let source = root.join("source.bin");
        let destination = root.join("destination.bin");
        let parked = root.join("parked-original.bin");
        fs::write(&source, b"selected bytes").unwrap();
        let parent_metadata = fs::metadata(&root).unwrap();
        let parent_identity = FileIdentity {
            volume: parent_metadata.dev(),
            file: parent_metadata.ino(),
        };
        let expected = snapshot_sync(&source).unwrap();

        let result = rename_verified_sync_with_hook(
            &source,
            &destination,
            &expected,
            parent_identity,
            parent_identity,
            || {
                fs::rename(&source, &parked).unwrap();
                fs::write(&source, b"replacement").unwrap();
            },
        );

        assert_eq!(result, Err(FileOperationError::IdentityChanged));
        assert_eq!(fs::read(&source).unwrap(), b"replacement");
        assert_eq!(fs::read(&parked).unwrap(), b"selected bytes");
        assert!(!destination.exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn verified_rename_rejects_a_parent_fd_reparented_outside_after_leaf_validation() {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_root = fs::canonicalize(outside.path()).unwrap();
        let source_parent = root.join("source");
        let destination_parent = root.join("destination");
        fs::create_dir(&source_parent).unwrap();
        fs::create_dir(&destination_parent).unwrap();
        let source = source_parent.join("item.bin");
        let destination = destination_parent.join("item.bin");
        fs::write(&source, b"selected bytes").unwrap();
        let source_metadata = fs::metadata(&source_parent).unwrap();
        let destination_metadata = fs::metadata(&destination_parent).unwrap();
        let expected = snapshot_sync(&source).unwrap();
        let moved_parent = outside_root.join("moved-source");

        let result = rename_verified_sync_with_hook(
            &source,
            &destination,
            &expected,
            FileIdentity {
                volume: source_metadata.dev(),
                file: source_metadata.ino(),
            },
            FileIdentity {
                volume: destination_metadata.dev(),
                file: destination_metadata.ino(),
            },
            || {
                fs::rename(&source_parent, &moved_parent).unwrap();
                fs::create_dir(&source_parent).unwrap();
                fs::write(source_parent.join("item.bin"), b"replacement").unwrap();
            },
        );

        assert!(matches!(result, Err(FileOperationError::OutsideProject)));
        assert_eq!(
            fs::read(moved_parent.join("item.bin")).unwrap(),
            b"selected bytes"
        );
        assert_eq!(fs::read(&source).unwrap(), b"replacement");
        assert!(!destination.exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn verified_rename_rolls_back_when_a_parent_is_reparented_after_the_syscall() {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_root = fs::canonicalize(outside.path()).unwrap();
        let source_parent = root.join("source");
        let destination_parent = root.join("destination");
        fs::create_dir(&source_parent).unwrap();
        fs::create_dir(&destination_parent).unwrap();
        let source = source_parent.join("item.bin");
        let destination = destination_parent.join("item.bin");
        fs::write(&source, b"selected bytes").unwrap();
        let source_metadata = fs::metadata(&source_parent).unwrap();
        let destination_metadata = fs::metadata(&destination_parent).unwrap();
        let expected = snapshot_sync(&source).unwrap();
        let moved_parent = outside_root.join("moved-source");

        let result = rename_verified_sync_with_hooks(
            &source,
            &destination,
            &expected,
            FileIdentity {
                volume: source_metadata.dev(),
                file: source_metadata.ino(),
            },
            FileIdentity {
                volume: destination_metadata.dev(),
                file: destination_metadata.ino(),
            },
            || {},
            || fs::rename(&source_parent, &moved_parent).unwrap(),
        );

        assert!(matches!(result, Err(FileOperationError::OutsideProject)));
        assert_eq!(
            fs::read(moved_parent.join("item.bin")).unwrap(),
            b"selected bytes"
        );
        assert!(!destination.exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn verified_rename_keeps_a_recoverable_destination_when_reparent_rollback_is_blocked() {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_root = fs::canonicalize(outside.path()).unwrap();
        let source_parent = root.join("source");
        let destination_parent = root.join("destination");
        fs::create_dir(&source_parent).unwrap();
        fs::create_dir(&destination_parent).unwrap();
        let source = source_parent.join("item.bin");
        let destination = destination_parent.join("item.bin");
        fs::write(&source, b"selected bytes").unwrap();
        let source_metadata = fs::metadata(&source_parent).unwrap();
        let destination_metadata = fs::metadata(&destination_parent).unwrap();
        let expected = snapshot_sync(&source).unwrap();
        let moved_parent = outside_root.join("moved-source");

        let result = rename_verified_sync_with_hooks(
            &source,
            &destination,
            &expected,
            FileIdentity {
                volume: source_metadata.dev(),
                file: source_metadata.ino(),
            },
            FileIdentity {
                volume: destination_metadata.dev(),
                file: destination_metadata.ino(),
            },
            || {},
            || {
                fs::rename(&source_parent, &moved_parent).unwrap();
                fs::write(moved_parent.join("item.bin"), b"replacement").unwrap();
            },
        );

        assert!(matches!(result, Err(FileOperationError::OutsideProject)));
        assert_eq!(fs::read(&destination).unwrap(), b"selected bytes");
        assert_eq!(
            fs::read(moved_parent.join("item.bin")).unwrap(),
            b"replacement"
        );
    }
}
