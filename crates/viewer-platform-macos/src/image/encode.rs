use objc2_core_foundation::{CFString, CFURL};
use objc2_core_graphics::{CGDataConsumer, CGDataConsumerCallbacks, CGImage};
use objc2_image_io::CGImageDestination;
use std::{
    ffi::c_void,
    fs::{self, File},
    io::Write,
    path::Path,
    ptr::NonNull,
    sync::{Arc, Mutex},
};
use viewer_application::ImageError;

pub(super) struct EncodedPng {
    pub bytes: Vec<u8>,
    pub digest: [u8; 32],
    pub size_bytes: u64,
}

struct StreamingPngSink {
    file: File,
    hasher: blake3::Hasher,
    bytes: Vec<u8>,
    size_bytes: u64,
    limit: u64,
    failed: bool,
    limit_exceeded: bool,
}

unsafe extern "C-unwind" fn put_png_bytes(
    info: *mut c_void,
    bytes: NonNull<c_void>,
    count: usize,
) -> usize {
    if info.is_null() {
        return 0;
    }
    // SAFETY: `info` is an Arc raw pointer retained until Core Graphics invokes
    // `release_png_sink`; Image I/O guarantees `bytes` contains `count` readable bytes.
    let sink = unsafe { &*info.cast::<Mutex<StreamingPngSink>>() };
    let Ok(mut sink) = sink.lock() else {
        return 0;
    };
    let Some(next_size) = sink.size_bytes.checked_add(count as u64) else {
        sink.failed = true;
        sink.limit_exceeded = true;
        return 0;
    };
    if sink.failed || next_size > sink.limit || sink.bytes.try_reserve(count).is_err() {
        sink.failed = true;
        sink.limit_exceeded = true;
        return 0;
    }
    // SAFETY: guaranteed by the CGDataConsumer callback contract for this call.
    let chunk = unsafe { std::slice::from_raw_parts(bytes.as_ptr().cast::<u8>(), count) };
    if sink.file.write_all(chunk).is_err() {
        sink.failed = true;
        return 0;
    }
    sink.hasher.update(chunk);
    sink.bytes.extend_from_slice(chunk);
    sink.size_bytes = next_size;
    count
}

unsafe extern "C-unwind" fn release_png_sink(info: *mut c_void) {
    if !info.is_null() {
        // SAFETY: exactly one Arc raw pointer is transferred to CGDataConsumer::new below.
        drop(unsafe { Arc::from_raw(info.cast::<Mutex<StreamingPngSink>>()) });
    }
}

pub(super) fn encode_png_streaming(
    image: &CGImage,
    file: File,
    limit: u64,
) -> Result<EncodedPng, ImageError> {
    let sink = Arc::new(Mutex::new(StreamingPngSink {
        file,
        hasher: blake3::Hasher::new(),
        bytes: Vec::new(),
        size_bytes: 0,
        limit,
        failed: false,
        limit_exceeded: false,
    }));
    let info = Arc::into_raw(sink.clone()).cast_mut().cast::<c_void>();
    let callbacks = CGDataConsumerCallbacks {
        putBytes: Some(put_png_bytes),
        releaseConsumer: Some(release_png_sink),
    };
    // SAFETY: `info` owns one Arc reference released by `release_png_sink`; callbacks and their
    // context remain valid for the retained consumer's complete lifetime.
    let Some(consumer) = (unsafe { CGDataConsumer::new(info, &callbacks) }) else {
        // No consumer exists to invoke the release callback.
        drop(unsafe { Arc::from_raw(info.cast::<Mutex<StreamingPngSink>>()) });
        return Err(ImageError::Io("create streaming PNG consumer".into()));
    };
    let png_type = CFString::from_str("public.png");
    // SAFETY: the typed consumer, PNG UTI, image count, and absent options satisfy Image I/O.
    let destination =
        unsafe { CGImageDestination::with_data_consumer(&consumer, &png_type, 1, None) }
            .ok_or_else(|| ImageError::Io("create streaming PNG destination".into()))?;
    // SAFETY: `image` is retained for this call and no untyped properties are supplied.
    unsafe { destination.add_image(image, None) };
    // SAFETY: the destination contains the one image declared at construction.
    let finalized = unsafe { destination.finalize() };
    drop(destination);
    drop(consumer);

    let sink = Arc::try_unwrap(sink)
        .map_err(|_| ImageError::Io("release streaming PNG consumer".into()))?
        .into_inner()
        .map_err(|_| ImageError::Io("access streaming PNG result".into()))?;
    if sink.limit_exceeded {
        return Err(ImageError::BudgetExceeded);
    }
    if !finalized || sink.failed || sink.size_bytes == 0 {
        return Err(ImageError::Io("finalize streaming PNG artifact".into()));
    }
    sink.file
        .sync_all()
        .map_err(|error| ImageError::Io(format!("sync streaming PNG artifact: {error}")))?;
    Ok(EncodedPng {
        bytes: sink.bytes,
        digest: *sink.hasher.finalize().as_bytes(),
        size_bytes: sink.size_bytes,
    })
}

pub(super) fn encode_png(image: &CGImage, destination: &Path) -> Result<(), ImageError> {
    if destination.exists() {
        fs::remove_file(destination).map_err(|error| {
            ImageError::Io(format!(
                "remove existing image artifact {}: {error}",
                destination.display()
            ))
        })?;
    }

    let url = CFURL::from_file_path(destination).ok_or_else(|| {
        ImageError::Io(format!(
            "image artifact path cannot be represented as a file URL: {}",
            destination.display()
        ))
    })?;
    let png_type = CFString::from_str("public.png");
    // SAFETY: `url`, `png_type`, the image count, and the absent options have
    // the Core Foundation types required by CGImageDestination.
    let image_destination = unsafe { CGImageDestination::with_url(&url, &png_type, 1, None) }
        .ok_or_else(|| {
            ImageError::Io(format!(
                "create PNG image destination at {}",
                destination.display()
            ))
        })?;

    // SAFETY: `image` is retained for this call and no untyped properties are
    // passed to Image I/O.
    unsafe { image_destination.add_image(image, None) };
    // SAFETY: The destination contains exactly the single image declared at
    // construction and is not used after finalization.
    if !unsafe { image_destination.finalize() } {
        let _ = fs::remove_file(destination);
        return Err(ImageError::Io(format!(
            "finalize PNG image artifact at {}",
            destination.display()
        )));
    }

    Ok(())
}
