use std::path::Path;
use viewer_application::{
    FileOperationError, VolumePort,
    rename::{
        RenameErrorCode, RenameRuleSet, RenameTarget, SequenceRule, SingleRenameRequest,
        preview_rename, preview_single_rename,
    },
};
use viewer_domain::{EntityId, OperationId, RelativePath};
use viewer_infrastructure::operation::rename::{RenameMapping, RenamePlanner, RenameStage};
use viewer_test_support::project_fixture::ProjectFixture;

fn target(path: &str) -> RenameTarget {
    RenameTarget {
        entity_id: EntityId::new(),
        relative_path: RelativePath::parse(path).unwrap(),
    }
}

fn rules() -> RenameRuleSet {
    RenameRuleSet {
        find: String::new(),
        replacement: String::new(),
        prefix: String::new(),
        suffix: String::new(),
        sequence: None,
    }
}

fn proposed_names(preview: &viewer_application::rename::RenamePreflight) -> Vec<&str> {
    preview
        .rows
        .iter()
        .map(|row| row.proposed_name.as_str())
        .collect()
}

#[test]
fn rules_compose_in_fixed_visible_order_and_preserve_the_last_extension() {
    let targets = [
        target("set/product.old.front.png"),
        target("set/.prompt"),
        target("set/说明.old.jpg"),
    ];
    let preview = preview_rename(
        &targets,
        &RenameRuleSet {
            find: ".old".into(),
            replacement: String::new(),
            prefix: "new-".into(),
            suffix: "-hero".into(),
            sequence: Some(SequenceRule {
                start: 7,
                digits: 3,
            }),
        },
    );

    assert_eq!(
        proposed_names(&preview),
        [
            "new-product.front-hero007.png",
            "new-.prompt-hero008",
            "new-说明-hero009.jpg",
        ]
    );
    assert!(preview.executable);
    assert_eq!(
        preview.rows[0].destination.as_ref().unwrap().as_str(),
        "set/new-product.front-hero007.png"
    );
}

#[test]
fn literal_replace_runs_on_the_full_name_before_suffix_and_sequence() {
    let preview = preview_rename(
        &[target("photo.jpg")],
        &RenameRuleSet {
            find: ".jpg".into(),
            replacement: ".png".into(),
            prefix: "new-".into(),
            suffix: "-hero".into(),
            sequence: Some(SequenceRule {
                start: 1,
                digits: 2,
            }),
        },
    );
    assert_eq!(proposed_names(&preview), ["new-photo-hero01.png"]);
    assert!(preview.executable);
}

#[test]
fn sequence_accepts_zero_and_the_maximum_with_one_to_six_padding_digits() {
    let first = preview_rename(
        &[target("a.jpg"), target("b.jpg")],
        &RenameRuleSet {
            prefix: "x-".into(),
            sequence: Some(SequenceRule {
                start: 0,
                digits: 1,
            }),
            ..rules()
        },
    );
    assert_eq!(proposed_names(&first), ["x-a0.jpg", "x-b1.jpg"]);
    assert!(first.executable);

    let maximum = preview_rename(
        &[target("a.jpg")],
        &RenameRuleSet {
            prefix: "x-".into(),
            sequence: Some(SequenceRule {
                start: 999_999,
                digits: 6,
            }),
            ..rules()
        },
    );
    assert_eq!(proposed_names(&maximum), ["x-a999999.jpg"]);
    assert!(maximum.executable);

    for digits in [0, 7] {
        let invalid = preview_rename(
            &[target("a.jpg")],
            &RenameRuleSet {
                prefix: "x-".into(),
                sequence: Some(SequenceRule { start: 0, digits }),
                ..rules()
            },
        );
        assert!(!invalid.executable);
        assert!(
            invalid.rows[0]
                .errors
                .contains(&RenameErrorCode::InvalidSequence)
        );
    }

    let overflow = preview_rename(
        &[target("a.jpg"), target("b.jpg")],
        &RenameRuleSet {
            prefix: "x-".into(),
            sequence: Some(SequenceRule {
                start: 999_999,
                digits: 1,
            }),
            ..rules()
        },
    );
    assert!(
        overflow.rows[1]
            .errors
            .contains(&RenameErrorCode::SequenceOutOfRange)
    );
    assert!(!overflow.executable);
}

#[test]
fn lexical_validation_returns_stable_row_codes_without_touching_disk() {
    let cases = [
        ("a", "", "", RenameErrorCode::EmptyName),
        ("a", "", ".", RenameErrorCode::DotName),
        ("a", "", "..", RenameErrorCode::DotName),
        ("a", "folder/", "", RenameErrorCode::ContainsSeparator),
        ("a", "", "\0", RenameErrorCode::ContainsNul),
        ("a", "", ".viewer", RenameErrorCode::ReservedName),
        (
            "a",
            "",
            ".viewer-rename-x.part",
            RenameErrorCode::TemporaryName,
        ),
    ];
    for (source, prefix, replacement, expected) in cases {
        let preview = preview_rename(
            &[target(source)],
            &RenameRuleSet {
                find: "a".into(),
                replacement: replacement.into(),
                prefix: prefix.into(),
                ..rules()
            },
        );
        assert!(!preview.executable, "accepted {expected:?}");
        assert!(
            preview.rows[0].errors.contains(&expected),
            "missing {expected:?} in {:?}",
            preview.rows[0].errors
        );
    }

    let no_op = preview_rename(&[target("notes/a.tar.gz")], &rules());
    assert!(!no_op.executable);
    assert!(no_op.rows[0].errors.contains(&RenameErrorCode::NoOp));
}

#[test]
fn single_rename_preserves_the_extension_unless_extension_editing_is_enabled() {
    let target = target("products/photo.final.png");
    let preserved = preview_single_rename(&SingleRenameRequest {
        target: target.clone(),
        requested_name: "hero".into(),
        edit_extension: false,
    });
    assert_eq!(proposed_names(&preserved), ["hero.png"]);
    assert!(preserved.executable);

    let edited = preview_single_rename(&SingleRenameRequest {
        target,
        requested_name: "hero.jpg".into(),
        edit_extension: true,
    });
    assert_eq!(proposed_names(&edited), ["hero.jpg"]);
    assert!(edited.executable);

    let project = ProjectFixture::new();
    project.create_file("products/photo.final.png", b"image");
    let prepared = RenamePlanner::preflight_preview(
        project.root(),
        &FakeVolume {
            case_sensitive: true,
            name_max: 255,
        },
        preserved,
    )
    .unwrap();
    assert!(prepared.plan.is_some());
}

#[derive(Clone, Copy)]
struct FakeVolume {
    case_sensitive: bool,
    name_max: usize,
}

impl VolumePort for FakeVolume {
    fn volume_id(&self, _path: &Path) -> Result<u64, FileOperationError> {
        Ok(1)
    }

    fn is_case_sensitive(&self, _path: &Path) -> Result<bool, FileOperationError> {
        Ok(self.case_sensitive)
    }

    fn name_max(&self, _path: &Path) -> Result<usize, FileOperationError> {
        Ok(self.name_max)
    }
}

#[test]
fn filesystem_preflight_enforces_name_limit_duplicates_case_and_occupancy() {
    let project = ProjectFixture::new();
    project.create_file("ab.jpg", b"ab");
    project.create_file("aab.jpg", b"aab");
    project.create_file("new-ab.jpg", b"occupied");
    let volume = FakeVolume {
        case_sensitive: true,
        name_max: 12,
    };

    let duplicate = RenamePlanner::preflight(
        project.root(),
        &volume,
        &[target("ab.jpg"), target("aab.jpg")],
        &RenameRuleSet {
            find: "a".into(),
            replacement: String::new(),
            prefix: "renamed-".into(),
            ..rules()
        },
    )
    .unwrap();
    assert!(!duplicate.preview.executable);
    assert!(
        duplicate
            .preview
            .rows
            .iter()
            .all(|row| row.errors.contains(&RenameErrorCode::DuplicateDestination))
    );
    assert!(
        duplicate
            .preview
            .rows
            .iter()
            .all(|row| row.errors.contains(&RenameErrorCode::NameTooLong))
    );

    let occupied = RenamePlanner::preflight(
        project.root(),
        &FakeVolume {
            case_sensitive: true,
            name_max: 255,
        },
        &[target("ab.jpg")],
        &RenameRuleSet {
            prefix: "new-".into(),
            ..rules()
        },
    )
    .unwrap();
    assert!(
        occupied.preview.rows[0]
            .errors
            .contains(&RenameErrorCode::DestinationOccupied)
    );

    let case_project = ProjectFixture::new();
    case_project.create_file("a.jpg", b"a");
    case_project.create_file("A.jpg", b"A");
    let collision = RenamePlanner::preflight(
        case_project.root(),
        &FakeVolume {
            case_sensitive: false,
            name_max: 255,
        },
        &[target("a.jpg"), target("A.jpg")],
        &RenameRuleSet {
            suffix: "-x".into(),
            ..rules()
        },
    )
    .unwrap();
    assert!(
        collision
            .preview
            .rows
            .iter()
            .all(|row| row.errors.contains(&RenameErrorCode::CaseCollision))
    );
    assert!(collision.plan.is_none());
}

#[cfg(unix)]
#[test]
fn filesystem_preflight_rejects_duplicate_sources_readonly_parents_and_symlink_escape() {
    use std::{fs, os::unix::fs::PermissionsExt, os::unix::fs::symlink};

    let project = ProjectFixture::new();
    project.create_file("readonly/a.jpg", b"a");
    let volume = FakeVolume {
        case_sensitive: true,
        name_max: 255,
    };
    let duplicate = RenamePlanner::preflight(
        project.root(),
        &volume,
        &[target("readonly/a.jpg"), target("readonly/a.jpg")],
        &RenameRuleSet {
            prefix: "x-".into(),
            ..rules()
        },
    )
    .unwrap();
    assert!(
        duplicate
            .preview
            .rows
            .iter()
            .all(|row| row.errors.contains(&RenameErrorCode::DuplicateSource))
    );

    let parent = project.root().join("readonly");
    let original_mode = fs::metadata(&parent).unwrap().permissions().mode();
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o555)).unwrap();
    let readonly = RenamePlanner::preflight(
        project.root(),
        &volume,
        &[target("readonly/a.jpg")],
        &RenameRuleSet {
            prefix: "x-".into(),
            ..rules()
        },
    )
    .unwrap();
    fs::set_permissions(&parent, fs::Permissions::from_mode(original_mode)).unwrap();
    assert!(
        readonly.preview.rows[0]
            .errors
            .contains(&RenameErrorCode::DestinationReadOnly)
    );

    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("outside.jpg"), b"outside").unwrap();
    symlink(outside.path(), project.root().join("escaped")).unwrap();
    let escaped = RenamePlanner::preflight(
        project.root(),
        &volume,
        &[target("escaped/outside.jpg")],
        &RenameRuleSet {
            prefix: "x-".into(),
            ..rules()
        },
    )
    .unwrap();
    assert!(
        escaped.preview.rows[0]
            .errors
            .contains(&RenameErrorCode::UnsafeParent)
    );
    assert!(escaped.plan.is_none());

    project.create_directory("other");
    let mut forged = preview_single_rename(&SingleRenameRequest {
        target: target("readonly/a.jpg"),
        requested_name: "b".into(),
        edit_extension: false,
    });
    forged.rows[0].destination = Some(RelativePath::parse("other/b.jpg").unwrap());
    let cross_parent = RenamePlanner::preflight_preview(project.root(), &volume, forged).unwrap();
    assert!(
        cross_parent.preview.rows[0]
            .errors
            .contains(&RenameErrorCode::UnsafeParent)
    );
}

#[test]
fn filesystem_preflight_accepts_unicode_and_produces_a_staged_plan() {
    let project = ProjectFixture::new();
    project.create_file("产品图.png", b"image");
    let prepared = RenamePlanner::preflight(
        project.root(),
        &FakeVolume {
            case_sensitive: true,
            name_max: 255,
        },
        &[target("产品图.png")],
        &RenameRuleSet {
            prefix: "已选-".into(),
            ..rules()
        },
    )
    .unwrap();

    assert!(prepared.preview.executable);
    assert_eq!(
        prepared.preview.rows[0]
            .destination
            .as_ref()
            .unwrap()
            .as_str(),
        "已选-产品图.png"
    );
    assert!(matches!(
        prepared.plan.unwrap().stages.as_slice(),
        [RenameStage::ToFinal { .. }]
    ));
}

#[test]
fn accepted_planner_still_stages_cycles_and_case_only_renames() {
    let project = ProjectFixture::new();
    project.create_file("A.jpg", b"A");
    project.create_file("B.jpg", b"B");
    let mapping = |source: &str, destination: &str| RenameMapping {
        operation_id: OperationId::new(),
        entity_id: EntityId::new(),
        source: source.into(),
        destination: destination.into(),
    };
    let cycle = RenamePlanner::plan(
        project.root(),
        true,
        &[mapping("A.jpg", "B.jpg"), mapping("B.jpg", "A.jpg")],
    )
    .unwrap();
    assert_eq!(
        cycle
            .stages
            .iter()
            .filter(|stage| matches!(stage, RenameStage::ToTemporary { .. }))
            .count(),
        2
    );

    let case_only =
        RenamePlanner::plan(project.root(), false, &[mapping("A.jpg", "a.jpg")]).unwrap();
    assert!(matches!(
        case_only.stages.as_slice(),
        [RenameStage::ToTemporary { .. }, RenameStage::ToFinal { .. }]
    ));
}
