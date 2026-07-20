use objc2::{AnyThread, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSApplication, NSDragOperation, NSDraggingContext, NSDraggingItem, NSDraggingSession,
    NSDraggingSource, NSEvent, NSEventModifierFlags, NSEventType, NSView, NSWorkspace,
};
use objc2_foundation::{NSArray, NSObject, NSObjectProtocol, NSRect, NSSize, NSString, NSURL};
use std::{cell::RefCell, ffi::CString, os::unix::ffi::OsStrExt, path::Path};
use viewer_application::{FinderDragError, FinderDragPort, PreparedFinderDrag};

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
        let workspace = NSWorkspace::sharedWorkspace();
        let mut items = Vec::with_capacity(selection.files().len());
        for index in 0..selection.files().len() {
            // Resolve and verify the indexed device/inode immediately before
            // handing this path to AppKit. A file replaced after preparation
            // must never be exported under the stale entity id.
            let path = selection.revalidate_file(index)?;
            if is_macos_alias(path) {
                return Err(FinderDragError::AliasNotAllowed);
            }
            let path = NSString::from_str(path.to_str().ok_or(FinderDragError::NativeUnavailable)?);
            let url = NSURL::fileURLWithPath(&path);
            let writer = objc2::runtime::ProtocolObject::from_ref(&*url);
            let item = NSDraggingItem::initWithPasteboardWriter(NSDraggingItem::alloc(), writer);
            let icon = workspace.iconForFile(&path);
            // SAFETY: NSImage is an AppKit-supported dragging item content type.
            unsafe {
                item.setDraggingFrame_contents(
                    NSRect::new(origin, NSSize::new(64.0, 64.0)),
                    Some(&icon),
                )
            };
            items.push(item);
        }
        let items = NSArray::from_retained_slice(&items);
        let source = ViewerDraggingSource::new(mtm);
        content_view.beginDraggingSessionWithItems_event_source(
            &items,
            &event,
            objc2::runtime::ProtocolObject::from_ref(&*source),
        );
        ACTIVE_SOURCE.with(|active| *active.borrow_mut() = Some(source));
        Ok(())
    }
}

fn drag_event_timestamp(event: Option<&NSEvent>) -> f64 {
    event.map(NSEvent::timestamp).unwrap_or(0.0)
}

fn is_macos_alias(path: &Path) -> bool {
    let Ok(path) = CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    let name = c"com.apple.FinderInfo";
    let mut finder_info = [0_u8; 32];
    // SAFETY: Both C strings are NUL terminated and finder_info is a valid
    // writable buffer used only for this read-only extended-attribute query.
    let read = unsafe {
        libc::getxattr(
            path.as_ptr(),
            name.as_ptr(),
            finder_info.as_mut_ptr().cast(),
            finder_info.len(),
            0,
            0,
        )
    };
    read >= 10 && u16::from_be_bytes([finder_info[8], finder_info[9]]) & 0x8000 != 0
}

#[cfg(test)]
mod tests {
    use super::{drag_event_timestamp, is_macos_alias};
    use std::{ffi::CString, fs, os::unix::ffi::OsStrExt};
    use tempfile::tempdir;

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
        assert!(is_macos_alias(&file));
    }
}
