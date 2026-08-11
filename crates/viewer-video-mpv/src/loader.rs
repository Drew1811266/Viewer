use std::{ffi::c_ulong, path::PathBuf, sync::Arc};

use libloading::Library;
use thiserror::Error;

use crate::{
    ffi::{MPV_CLIENT_API_MAJOR, MpvApi, MpvRenderApi},
    runtime_manifest::{ManifestError, RuntimeLayout, RuntimeManifest},
};

#[derive(Debug, Error)]
pub enum MpvLoadError {
    #[error("the bundled libmpv path must be absolute")]
    LibraryPathMustBeAbsolute,
    #[error("the bundled libmpv path does not match the runtime layout")]
    UnexpectedLibraryPath,
    #[error("the bundled video runtime manifest path does not match the runtime layout")]
    UnexpectedManifestPath,
    #[error("the bundled video runtime manifest could not be accepted")]
    RuntimeManifest(#[from] ManifestError),
    #[error("the bundled video runtime manifest does not describe the approved mpv build")]
    UnapprovedBuild,
    #[error("the bundled libmpv library could not be loaded: {0}")]
    Load(#[source] libloading::Error),
    #[error("the bundled libmpv library is missing a required symbol: {0}")]
    MissingSymbol(#[source] libloading::Error),
    #[error("the bundled libmpv client API major version is unsupported: {actual}")]
    UnsupportedClientApi { actual: c_ulong },
}

pub(crate) struct LoadedLibrary {
    _library: Library,
}

pub struct MpvLibrary {
    api: MpvApi,
    render_api: MpvRenderApi,
    library: Arc<LoadedLibrary>,
    path: PathBuf,
}

impl MpvLibrary {
    pub fn load(layout: &RuntimeLayout) -> Result<Self, MpvLoadError> {
        if !layout.libmpv.is_absolute() {
            return Err(MpvLoadError::LibraryPathMustBeAbsolute);
        }
        if layout.libmpv != layout.root.join("lib/libmpv.2.dylib") {
            return Err(MpvLoadError::UnexpectedLibraryPath);
        }
        if layout.manifest != layout.root.join("runtime.lock.json") {
            return Err(MpvLoadError::UnexpectedManifestPath);
        }
        let manifest = RuntimeManifest::load(&layout.manifest)?;
        verify_approved_build(&manifest)?;

        let library = unsafe { Library::new(&layout.libmpv) }.map_err(MpvLoadError::Load)?;
        let api = unsafe { load_client_api(&library) }?;
        let actual = unsafe { (api.client_api_version)() };
        if actual >> 16 != MPV_CLIENT_API_MAJOR {
            return Err(MpvLoadError::UnsupportedClientApi { actual });
        }
        let render_api = unsafe { load_render_api(&library) }?;

        Ok(Self {
            api,
            render_api,
            library: Arc::new(LoadedLibrary { _library: library }),
            path: layout.libmpv.clone(),
        })
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub(crate) fn api(&self) -> MpvApi {
        self.api
    }

    pub(crate) fn render_api(&self) -> MpvRenderApi {
        self.render_api
    }

    pub(crate) fn library(&self) -> Arc<LoadedLibrary> {
        Arc::clone(&self.library)
    }
}

fn verify_approved_build(manifest: &RuntimeManifest) -> Result<(), MpvLoadError> {
    if manifest.mpv.tag != "v0.41.0"
        || manifest.mpv.commit != "41f6a64"
        || ![
            ("gpl", "false"),
            ("cplayer", "false"),
            ("libmpv", "true"),
            ("javascript", "disabled"),
            ("lua", "disabled"),
            ("cplugins", "disabled"),
        ]
        .into_iter()
        .all(|(name, value)| {
            manifest.mpv.meson_options.get(name).map(String::as_str) == Some(value)
        })
        || !manifest
            .ffmpeg
            .configure_options
            .iter()
            .any(|option| option == "--disable-network")
    {
        return Err(MpvLoadError::UnapprovedBuild);
    }
    Ok(())
}

unsafe fn symbol<T: Copy>(library: &Library, name: &[u8]) -> Result<T, MpvLoadError> {
    unsafe { library.get::<T>(name) }
        .map(|symbol| *symbol)
        .map_err(MpvLoadError::MissingSymbol)
}

unsafe fn load_client_api(library: &Library) -> Result<MpvApi, MpvLoadError> {
    Ok(MpvApi {
        client_api_version: unsafe { symbol(library, b"mpv_client_api_version\0") }?,
        create: unsafe { symbol(library, b"mpv_create\0") }?,
        initialize: unsafe { symbol(library, b"mpv_initialize\0") }?,
        terminate_destroy: unsafe { symbol(library, b"mpv_terminate_destroy\0") }?,
        set_option_string: unsafe { symbol(library, b"mpv_set_option_string\0") }?,
        command: unsafe { symbol(library, b"mpv_command\0") }?,
        set_property: unsafe { symbol(library, b"mpv_set_property\0") }?,
        get_property: unsafe { symbol(library, b"mpv_get_property\0") }?,
        free: unsafe { symbol(library, b"mpv_free\0") }?,
        observe_property: unsafe { symbol(library, b"mpv_observe_property\0") }?,
        wait_event: unsafe { symbol(library, b"mpv_wait_event\0") }?,
    })
}

unsafe fn load_render_api(library: &Library) -> Result<MpvRenderApi, MpvLoadError> {
    Ok(MpvRenderApi {
        create: unsafe { symbol(library, b"mpv_render_context_create\0") }?,
        free: unsafe { symbol(library, b"mpv_render_context_free\0") }?,
        set_update_callback: unsafe {
            symbol(library, b"mpv_render_context_set_update_callback\0")
        }?,
        update: unsafe { symbol(library, b"mpv_render_context_update\0") }?,
        render: unsafe { symbol(library, b"mpv_render_context_render\0") }?,
        report_swap: unsafe { symbol(library, b"mpv_render_context_report_swap\0") }?,
    })
}
