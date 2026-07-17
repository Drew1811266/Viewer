use std::collections::HashMap;
use viewer_domain::RelativePath;
pub use viewer_domain::operation::{
    RenameErrorCode, RenamePreflight, RenamePreviewRow, RenameRuleSet, RenameTarget, SequenceRule,
};

const MAX_SEQUENCE: u32 = 999_999;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SingleRenameRequest {
    pub target: RenameTarget,
    pub requested_name: String,
    pub edit_extension: bool,
}

pub fn preview_rename(targets: &[RenameTarget], rules: &RenameRuleSet) -> RenamePreflight {
    let sequence_valid = rules.sequence.is_none_or(|sequence| {
        (1..=6).contains(&sequence.digits) && sequence.start <= MAX_SEQUENCE
    });
    let mut rows = targets
        .iter()
        .enumerate()
        .map(|(index, target)| preview_row(target, rules, index, sequence_valid))
        .collect::<Vec<_>>();

    mark_duplicate_sources(&mut rows);
    mark_duplicate_destinations(&mut rows);
    let mut preview = RenamePreflight {
        rows,
        executable: false,
    };
    preview.refresh_executable();
    preview
}

pub fn preview_single_rename(request: &SingleRenameRequest) -> RenamePreflight {
    let source_name = file_name(&request.target.relative_path);
    let (_, extension) = split_extension(source_name);
    let proposed_name = if request.edit_extension {
        request.requested_name.clone()
    } else {
        format!("{}{}", request.requested_name, extension)
    };
    let row = finish_row(&request.target, proposed_name, Vec::new());
    let mut preview = RenamePreflight {
        rows: vec![row],
        executable: false,
    };
    preview.refresh_executable();
    preview
}

fn preview_row(
    target: &RenameTarget,
    rules: &RenameRuleSet,
    index: usize,
    sequence_valid: bool,
) -> RenamePreviewRow {
    let source_name = file_name(&target.relative_path);
    let replaced = if rules.find.is_empty() {
        source_name.to_owned()
    } else {
        source_name.replace(&rules.find, &rules.replacement)
    };
    let (stem, extension) = split_extension(&replaced);
    let mut transformed = stem.to_owned();
    transformed.insert_str(0, &rules.prefix);
    transformed.push_str(&rules.suffix);

    let mut errors = Vec::new();
    if let Some(sequence) = rules.sequence {
        if !sequence_valid {
            errors.push(RenameErrorCode::InvalidSequence);
        } else if let Ok(offset) = u32::try_from(index) {
            match sequence.start.checked_add(offset) {
                Some(value) if value <= MAX_SEQUENCE => {
                    transformed.push_str(&format!(
                        "{value:0width$}",
                        width = usize::from(sequence.digits)
                    ));
                }
                _ => errors.push(RenameErrorCode::SequenceOutOfRange),
            }
        } else {
            errors.push(RenameErrorCode::SequenceOutOfRange);
        }
    }
    transformed.push_str(extension);

    finish_row(target, transformed, errors)
}

fn finish_row(
    target: &RenameTarget,
    proposed_name: String,
    mut errors: Vec<RenameErrorCode>,
) -> RenamePreviewRow {
    validate_name(&proposed_name, &mut errors);
    let destination_string = target.relative_path.as_str().rsplit_once('/').map_or_else(
        || proposed_name.clone(),
        |(parent, _)| format!("{parent}/{proposed_name}"),
    );
    let destination = RelativePath::parse(&destination_string).ok();
    if destination.is_none() && errors.is_empty() {
        errors.push(RenameErrorCode::UnsafeParent);
    }
    if destination.as_ref() == Some(&target.relative_path) {
        errors.push(RenameErrorCode::NoOp);
    }

    RenamePreviewRow {
        entity_id: target.entity_id,
        source: target.relative_path.clone(),
        destination,
        proposed_name,
        errors,
    }
}

fn file_name(path: &RelativePath) -> &str {
    path.as_str()
        .rsplit_once('/')
        .map_or(path.as_str(), |(_, name)| name)
}

fn split_extension(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(index) if index > 0 => (&name[..index], &name[index..]),
        _ => (name, ""),
    }
}

fn validate_name(name: &str, errors: &mut Vec<RenameErrorCode>) {
    if name.is_empty() {
        errors.push(RenameErrorCode::EmptyName);
    }
    if matches!(name, "." | "..") {
        errors.push(RenameErrorCode::DotName);
    }
    if name.contains('/') {
        errors.push(RenameErrorCode::ContainsSeparator);
    }
    if name.contains('\0') {
        errors.push(RenameErrorCode::ContainsNul);
    }
    if name.eq_ignore_ascii_case(".viewer") {
        errors.push(RenameErrorCode::ReservedName);
    }
    let lowercase = name.to_ascii_lowercase();
    if [".viewer-copy-", ".viewer-rename-", ".viewer-replace-"]
        .iter()
        .any(|prefix| lowercase.starts_with(prefix))
    {
        errors.push(RenameErrorCode::TemporaryName);
    }
}

fn mark_duplicate_sources(rows: &mut [RenamePreviewRow]) {
    let mut positions: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, row) in rows.iter().enumerate() {
        positions
            .entry(row.source.as_str().to_owned())
            .or_default()
            .push(index);
    }
    for duplicates in positions.values().filter(|positions| positions.len() > 1) {
        for index in duplicates {
            rows[*index].push_error(RenameErrorCode::DuplicateSource);
        }
    }
}

fn mark_duplicate_destinations(rows: &mut [RenamePreviewRow]) {
    let mut positions: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, row) in rows.iter().enumerate() {
        if let Some(destination) = row.destination.as_ref() {
            positions
                .entry(destination.as_str().to_owned())
                .or_default()
                .push(index);
        }
    }
    for duplicates in positions.values().filter(|positions| positions.len() > 1) {
        for index in duplicates {
            rows[*index].push_error(RenameErrorCode::DuplicateDestination);
        }
    }
}
