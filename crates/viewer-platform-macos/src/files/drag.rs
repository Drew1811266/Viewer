use objc2::{AnyThread, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{
    NSApplication, NSDragOperation, NSDraggingContext, NSDraggingItem, NSDraggingSession,
    NSDraggingSource, NSEvent, NSEventModifierFlags, NSEventType, NSImage,
    NSImageNameMultipleDocuments, NSView,
};
use objc2_foundation::{NSArray, NSObject, NSObjectProtocol, NSRect, NSSize, NSURL};
use std::{
    cell::RefCell,
    ffi::{CStr, CString},
    fs::{self, File},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{ffi::OsStrExt, fs::MetadataExt},
    },
    path::{Path, PathBuf},
    ptr::NonNull,
};
use viewer_application::{FinderDragError, FinderDragPort, PreparedFinderDrag};
use viewer_domain::EntityId;

define_class!(
    // SAFETY: NSObject has no subclassing requirements and this class owns no
    // Rust resources that require Drop.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = ()]
    struct ViewerDraggingSource;

    // SAFETY: NSObjectProtocol has no additional safety requirements.
    unsafe impl NSObjectProtocol for ViewerDraggingSource {}

    // SAFETY: The method signatures match NSDraggingSource and all calls are
    // constrained to AppKit's main thread.
    unsafe impl NSDraggingSource for ViewerDraggingSource {
        #[unsafe(method(draggingSession:sourceOperationMaskForDraggingContext:))]
        fn operation_mask(
            &self,
            _session: &NSDraggingSession,
            _context: NSDraggingContext,
        ) -> NSDragOperation {
            NSDragOperation::Copy
        }
    }
);

impl ViewerDraggingSource {
    fn new(mtm: MainThreadMarker) -> objc2::rc::Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        // SAFETY: NSObject's init signature is correct for this subclass.
        unsafe { msg_send![super(this), init] }
    }
}

thread_local! {
    // AppKit does not document the dragging source as retained for the entire
    // session. Keep the tiny source alive until the next session/application exit.
    static ACTIVE_SOURCE: RefCell<Option<objc2::rc::Retained<ViewerDraggingSource>>> = const { RefCell::new(None) };
}

pub struct MacFinderDragPort<'a> {
    view: &'a NSView,
}

impl<'a> MacFinderDragPort<'a> {
    pub fn new(view: &'a NSView) -> Result<Self, FinderDragError> {
        MainThreadMarker::new().ok_or(FinderDragError::NativeUnavailable)?;
        Ok(Self { view })
    }
}

impl FinderDragPort for MacFinderDragPort<'_> {
    fn begin_drag(&self, selection: &PreparedFinderDrag) -> Result<(), FinderDragError> {
        let mtm = MainThreadMarker::new().ok_or(FinderDragError::NativeUnavailable)?;
        let application = NSApplication::sharedApplication(mtm);
        let window = self
            .view
            .window()
            .ok_or(FinderDragError::NativeUnavailable)?;
        let content_view = window
            .contentView()
            .ok_or(FinderDragError::NativeUnavailable)?;
        let mouse = window.mouseLocationOutsideOfEventStream();
        let current_event = application.currentEvent();
        let event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
            NSEventType::LeftMouseDragged,
            mouse,
            NSEventModifierFlags::empty(),
            drag_event_timestamp(current_event.as_deref()),
            window.windowNumber(),
            None,
            0,
            1,
            1.0,
        )
        .ok_or(FinderDragError::NativeUnavailable)?;
        let origin = content_view.convertPoint_fromView(mouse, None);
        // SAFETY: AppKit exports this process-lifetime image-name constant.
        let icon = unsafe { NSImage::imageNamed(NSImageNameMultipleDocuments) }
            .ok_or(FinderDragError::NativeUnavailable)?;
        let source = ViewerDraggingSource::new(mtm);
        with_bound_drag_publication(
            selection,
            |references| {
                let mut items = Vec::with_capacity(references.len());
                for reference in references {
                    let writer = objc2::runtime::ProtocolObject::from_ref(&*reference.url);
                    let item =
                        NSDraggingItem::initWithPasteboardWriter(NSDraggingItem::alloc(), writer);
                    // SAFETY: NSImage is an AppKit-supported dragging item content type.
                    unsafe {
                        item.setDraggingFrame_contents(
                            NSRect::new(origin, NSSize::new(64.0, 64.0)),
                            Some(&icon),
                        )
                    };
                    items.push(item);
                }
                Ok(NSArray::from_retained_slice(&items))
            },
            || {},
            |_, items| {
                content_view.beginDraggingSessionWithItems_event_source(
                    &items,
                    &event,
                    objc2::runtime::ProtocolObject::from_ref(&*source),
                );
                Ok(())
            },
        )?;
        ACTIVE_SOURCE.with(|active| *active.borrow_mut() = Some(source));
        Ok(())
    }
}

struct BoundDragReference {
    url: Retained<NSURL>,
    expected_entity_id: EntityId,
}

fn with_bound_drag_publication<T, Prepare, BeforePublication, Publish>(
    selection: &PreparedFinderDrag,
    prepare: Prepare,
    before_publication: BeforePublication,
    publish: Publish,
) -> Result<(), FinderDragError>
where
    Prepare: FnOnce(&[BoundDragReference]) -> Result<T, FinderDragError>,
    BeforePublication: FnOnce(),
    Publish: FnOnce(&[BoundDragReference], T) -> Result<(), FinderDragError>,
{
    let canonical_root = fs::canonicalize(selection.canonical_root())
        .map_err(|_| FinderDragError::ProjectRootUnavailable)?;
    if canonical_root != selection.canonical_root() {
        return Err(FinderDragError::ProjectRootUnavailable);
    }
    let mut references = Vec::with_capacity(selection.files().len());
    for index in 0..selection.files().len() {
        let (path, expected_entity_id) = selection.revalidate_file(index)?;
        references.push(bind_drag_reference(
            path,
            expected_entity_id,
            &canonical_root,
        )?);
    }
    let prepared = prepare(&references)?;
    before_publication();
    for reference in &references {
        verify_bound_drag_reference(reference, &canonical_root)?;
    }
    publish(&references, prepared)
}

fn bind_drag_reference(
    path: &Path,
    expected_entity_id: EntityId,
    canonical_root: &Path,
) -> Result<BoundDragReference, FinderDragError> {
    let encoded =
        CString::new(path.as_os_str().as_bytes()).map_err(|_| FinderDragError::EntityNotFound)?;
    let pointer = NonNull::new(encoded.as_ptr().cast_mut()).expect("CString pointer is non-null");
    let path_url = unsafe {
        NSURL::fileURLWithFileSystemRepresentation_isDirectory_relativeToURL(pointer, false, None)
    };
    let url = path_url
        .fileReferenceURL()
        .ok_or(FinderDragError::EntityNotFound)?;
    let reference = BoundDragReference {
        url,
        expected_entity_id,
    };
    verify_bound_drag_reference(&reference, canonical_root)?;
    Ok(reference)
}

fn verify_bound_drag_reference(
    reference: &BoundDragReference,
    canonical_root: &Path,
) -> Result<(), FinderDragError> {
    let current_root =
        fs::canonicalize(canonical_root).map_err(|_| FinderDragError::ProjectRootUnavailable)?;
    if current_root != canonical_root {
        return Err(FinderDragError::ProjectRootUnavailable);
    }
    let resolved = reference
        .url
        .filePathURL()
        .ok_or(FinderDragError::EntityNotFound)?;
    let resolved_path = unsafe { CStr::from_ptr(resolved.fileSystemRepresentation().as_ptr()) };
    let fd = unsafe {
        libc::open(
            resolved_path.as_ptr(),
            libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW_ANY,
        )
    };
    if fd < 0 {
        return Err(FinderDragError::EntityNotFound);
    }
    let file = unsafe { File::from_raw_fd(fd) };
    let metadata = file
        .metadata()
        .map_err(|_| FinderDragError::EntityNotFound)?;
    if !metadata.is_file() || entity_id(&metadata) != reference.expected_entity_id {
        return Err(FinderDragError::EntityNotFound);
    }
    if is_macos_alias_file(&file)? {
        return Err(FinderDragError::AliasNotAllowed);
    }
    let current_path = bound_file_path(&file)?;
    let relative = current_path
        .strip_prefix(canonical_root)
        .map_err(|_| FinderDragError::OutsideProject)?;
    if relative
        .components()
        .next()
        .and_then(|component| component.as_os_str().to_str())
        .is_some_and(|component| component.eq_ignore_ascii_case(".viewer"))
    {
        return Err(FinderDragError::OutsideProject);
    }
    Ok(())
}

fn bound_file_path(file: &File) -> Result<PathBuf, FinderDragError> {
    let mut path = vec![0_i8; libc::PATH_MAX as usize];
    let result = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETPATH, path.as_mut_ptr()) };
    if result < 0 {
        return Err(FinderDragError::EntityNotFound);
    }
    let current = unsafe { CStr::from_ptr(path.as_ptr()) };
    Ok(PathBuf::from(std::ffi::OsStr::from_bytes(
        current.to_bytes(),
    )))
}

fn entity_id(metadata: &fs::Metadata) -> EntityId {
    EntityId::from_u128((u128::from(metadata.dev()) << 64) | u128::from(metadata.ino()))
}

fn drag_event_timestamp(event: Option<&NSEvent>) -> f64 {
    event.map(NSEvent::timestamp).unwrap_or(0.0)
}

#[cfg(test)]
fn is_macos_alias(path: &Path) -> Result<bool, FinderDragError> {
    let path = fs::canonicalize(path).map_err(|_| FinderDragError::EntityNotFound)?;
    let path =
        CString::new(path.as_os_str().as_bytes()).map_err(|_| FinderDragError::EntityNotFound)?;
    let fd = unsafe {
        libc::open(
            path.as_ptr(),
            libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW_ANY,
        )
    };
    if fd < 0 {
        return Err(FinderDragError::EntityNotFound);
    }
    let file = unsafe { File::from_raw_fd(fd) };
    is_macos_alias_file(&file)
}

fn is_macos_alias_file(file: &File) -> Result<bool, FinderDragError> {
    let name = c"com.apple.FinderInfo";
    let mut finder_info = [0_u8; 32];
    // SAFETY: The file descriptor and attribute name are valid and
    // finder_info is a writable buffer used only for this read-only query.
    let read = unsafe {
        libc::fgetxattr(
            file.as_raw_fd(),
            name.as_ptr(),
            finder_info.as_mut_ptr().cast(),
            finder_info.len(),
            0,
            0,
        )
    };
    if read < 0 {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ENOATTR) {
            return Ok(false);
        }
        return Err(FinderDragError::EntityNotFound);
    }
    Ok(read >= 10 && u16::from_be_bytes([finder_info[8], finder_info[9]]) & 0x8000 != 0)
}

#[cfg(test)]
mod tests {
    use super::{drag_event_timestamp, is_macos_alias, with_bound_drag_publication};
    use std::{
        cell::Cell,
        ffi::{CStr, CString},
        fs,
        os::unix::{ffi::OsStrExt, fs::MetadataExt},
        path::{Path, PathBuf},
    };
    use tempfile::tempdir;
    use viewer_application::{FinderDragError, PreparedFinderDrag, prepare_finder_drag};
    use viewer_domain::{
        EntityId, RelativePath,
        file::{FileKind, FileNode},
    };
    use viewer_infrastructure::search::index::SessionIndex;

    #[test]
    fn a_missing_current_event_uses_a_valid_synthetic_timestamp() {
        assert_eq!(drag_event_timestamp(None), 0.0);
    }

    #[test]
    fn finder_info_alias_bit_is_detected() {
        let project = tempdir().expect("project");
        let file = project.path().join("alias.jpg");
        fs::write(&file, b"image").expect("file");
        let mut finder_info = [0_u8; 32];
        finder_info[8..10].copy_from_slice(&0x8000_u16.to_be_bytes());
        let path = CString::new(file.as_os_str().as_bytes()).expect("path");
        let name = c"com.apple.FinderInfo";
        // SAFETY: Inputs are valid C strings and finder_info is readable.
        let result = unsafe {
            libc::setxattr(
                path.as_ptr(),
                name.as_ptr(),
                finder_info.as_ptr().cast(),
                finder_info.len(),
                0,
                0,
            )
        };
        assert_eq!(result, 0);
        assert!(is_macos_alias(&file).unwrap());
    }

    #[test]
    fn drag_publication_uses_the_bound_reference_after_an_outside_symlink_swap() {
        use std::os::unix::fs::symlink;

        let project = tempdir().unwrap();
        let root = fs::canonicalize(project.path()).unwrap();
        let outside = tempdir().unwrap();
        let selected = root.join("selected.jpg");
        let parked = root.join("parked-selected.jpg");
        let outside_file = outside.path().join("outside.jpg");
        fs::write(&selected, b"selected bytes").unwrap();
        fs::write(&outside_file, b"outside bytes").unwrap();
        let selection = prepared_selection(&root, &selected);
        let published = Cell::new(false);

        with_bound_drag_publication(
            &selection,
            |_| Ok(()),
            || {
                fs::rename(&selected, &parked).unwrap();
                symlink(&outside_file, &selected).unwrap();
            },
            |references, ()| {
                published.set(true);
                let resolved = references[0]
                    .url
                    .filePathURL()
                    .ok_or(FinderDragError::EntityNotFound)?;
                let path = unsafe { CStr::from_ptr(resolved.fileSystemRepresentation().as_ptr()) };
                assert_eq!(
                    fs::read(PathBuf::from(std::ffi::OsStr::from_bytes(path.to_bytes()))).unwrap(),
                    b"selected bytes"
                );
                Ok(())
            },
        )
        .unwrap();

        assert!(published.get());
        assert_eq!(fs::read(&selected).unwrap(), b"outside bytes");
        assert_eq!(fs::read(&outside_file).unwrap(), b"outside bytes");
    }

    #[test]
    fn drag_publication_rejects_a_bound_identity_moved_outside_the_project() {
        let project = tempdir().unwrap();
        let root = fs::canonicalize(project.path()).unwrap();
        let outside = tempdir().unwrap();
        let selected = root.join("selected.jpg");
        let moved = outside.path().join("moved-selected.jpg");
        fs::write(&selected, b"selected bytes").unwrap();
        let selection = prepared_selection(&root, &selected);
        let published = Cell::new(false);

        let result = with_bound_drag_publication(
            &selection,
            |_| Ok(()),
            || fs::rename(&selected, &moved).unwrap(),
            |_, ()| {
                published.set(true);
                Ok(())
            },
        );

        assert_eq!(result, Err(FinderDragError::OutsideProject));
        assert!(!published.get());
        assert_eq!(fs::read(&moved).unwrap(), b"selected bytes");
    }

    fn prepared_selection(root: &Path, selected: &Path) -> PreparedFinderDrag {
        let metadata = fs::metadata(selected).unwrap();
        let entity_id =
            EntityId::from_u128((u128::from(metadata.dev()) << 64) | u128::from(metadata.ino()));
        let index_directory = tempdir().unwrap();
        let index = SessionIndex::open(index_directory.path().join("session.sqlite")).unwrap();
        index
            .upsert_batch(&[FileNode {
                entity_id,
                relative_path: RelativePath::parse(selected.file_name().unwrap().to_str().unwrap())
                    .unwrap(),
                kind: FileKind::Jpeg,
                size: metadata.len(),
                modified_ns: 1,
            }])
            .unwrap();
        prepare_finder_drag(root, &index, &[entity_id]).unwrap()
    }
}
