use tauri::{Runtime, WebviewWindow};
use viewer_render_core::InteractionMode;

use super::{
    DisplayTickSignal, InputExclusionRect, InputRect, MacDisplayLink, MacImageSurface,
    MacInputMonitor, NativeInputSink, SurfaceError, SurfaceLayout,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageRenderHostState {
    mounted: bool,
    visible: bool,
    display_link_running: bool,
    display_id: Option<u32>,
}

impl ImageRenderHostState {
    pub const fn mounted() -> Self {
        Self {
            mounted: true,
            visible: false,
            display_link_running: false,
            display_id: None,
        }
    }

    pub const fn is_mounted(self) -> bool {
        self.mounted
    }

    pub const fn display_link_running(self) -> bool {
        self.display_link_running
    }

    pub fn set_visible(&mut self, visible: bool) -> bool {
        if !self.mounted {
            return false;
        }
        self.visible = visible;
        true
    }

    pub fn start_display_link(&mut self) -> bool {
        self.start_display_link_for(None)
    }

    pub fn start_display_link_for(&mut self, display_id: Option<u32>) -> bool {
        if !self.mounted || self.display_link_running {
            return false;
        }
        self.display_link_running = true;
        self.display_id = display_id;
        true
    }

    pub fn rebind_display(&mut self, display_id: Option<u32>) -> bool {
        if !self.mounted || !self.display_link_running || self.display_id == display_id {
            return false;
        }
        self.display_id = display_id;
        true
    }

    pub const fn display_id(self) -> Option<u32> {
        self.display_id
    }

    pub fn stop_display_link(&mut self) -> bool {
        if !self.mounted || !self.display_link_running {
            return false;
        }
        self.display_link_running = false;
        self.display_id = None;
        true
    }

    pub fn unmount(&mut self) -> bool {
        if !self.mounted {
            return false;
        }
        self.display_link_running = false;
        self.display_id = None;
        self.visible = false;
        self.mounted = false;
        true
    }
}

pub struct MacImageRenderHost {
    surface: MacImageSurface,
    display_link: Option<MacDisplayLink>,
    display_signal: Option<DisplayTickSignal>,
    input_monitor: Option<MacInputMonitor>,
    state: ImageRenderHostState,
}

impl MacImageRenderHost {
    pub fn mount<R: Runtime>(
        window: &WebviewWindow<R>,
        layout: SurfaceLayout,
    ) -> Result<Self, SurfaceError> {
        Ok(Self {
            surface: MacImageSurface::mount(window, layout)?,
            display_link: None,
            display_signal: None,
            input_monitor: None,
            state: ImageRenderHostState::mounted(),
        })
    }

    pub fn set_layout(&mut self, layout: SurfaceLayout) -> Result<(), SurfaceError> {
        self.surface.set_layout(layout)?;
        if let Some(monitor) = self.input_monitor.as_ref() {
            monitor.update_geometry(input_rect(layout)?)?;
        }
        self.rebuild_display_link_if_needed()
    }

    pub fn set_visible(&mut self, visible: bool) -> Result<(), SurfaceError> {
        if !self.state.set_visible(visible) {
            return Err(SurfaceError::Unmounted);
        }
        self.surface.set_visible(visible)
    }

    pub fn start_display_link(&mut self, signal: DisplayTickSignal) -> Result<(), SurfaceError> {
        if self.state.display_link_running() {
            return Ok(());
        }
        if !self.state.is_mounted() {
            return Err(SurfaceError::Unmounted);
        }
        let display_id = self.surface.display_id()?;
        match MacDisplayLink::start_for_display(signal.clone(), display_id) {
            Ok(link) => {
                self.display_link = Some(link);
                self.display_signal = Some(signal);
                let started = self.state.start_display_link_for(display_id);
                debug_assert!(started, "validated stopped host must accept display link");
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    fn rebuild_display_link_if_needed(&mut self) -> Result<(), SurfaceError> {
        if !self.state.display_link_running() {
            return Ok(());
        }
        let display_id = self.surface.display_id()?;
        if self.state.display_id() == display_id {
            return Ok(());
        }
        let signal = self
            .display_signal
            .as_ref()
            .expect("running display link owns its signal")
            .clone();
        let replacement = MacDisplayLink::start_for_display(signal, display_id)?;
        let stop_result = self
            .display_link
            .replace(replacement)
            .map(|mut previous| previous.stop())
            .unwrap_or(Ok(()));
        let rebound = self.state.rebind_display(display_id);
        debug_assert!(rebound, "display change must update running host state");
        stop_result
    }

    pub fn stop_display_link(&mut self) -> Result<(), SurfaceError> {
        let result = self
            .display_link
            .take()
            .map(|mut link| link.stop())
            .unwrap_or(Ok(()));
        if let Some(signal) = self.display_signal.take() {
            signal.close();
        }
        self.state.stop_display_link();
        result
    }

    pub fn start_input_monitor(
        &mut self,
        exclusions: Vec<InputExclusionRect>,
        tool: InteractionMode,
        sink: NativeInputSink,
    ) -> Result<(), SurfaceError> {
        if !self.state.is_mounted() {
            return Err(SurfaceError::Unmounted);
        }
        if let Some(monitor) = self.input_monitor.as_ref() {
            monitor.set_exclusions(exclusions)?;
            monitor.set_tool(tool)?;
            return Ok(());
        }
        let window = self.surface.native_window()?;
        let monitor = MacInputMonitor::install(
            &window,
            self.surface.input_view()?,
            input_rect(self.surface.layout())?,
            exclusions,
            tool,
            sink,
        )?;
        self.input_monitor = Some(monitor);
        Ok(())
    }

    pub fn set_input_exclusions(
        &self,
        exclusions: Vec<InputExclusionRect>,
    ) -> Result<(), SurfaceError> {
        self.input_monitor
            .as_ref()
            .ok_or(SurfaceError::InputMonitorUnavailable)?
            .set_exclusions(exclusions)
    }

    pub fn set_input_tool(&self, tool: InteractionMode) -> Result<(), SurfaceError> {
        self.input_monitor
            .as_ref()
            .ok_or(SurfaceError::InputMonitorUnavailable)?
            .set_tool(tool)
    }

    pub fn stop_input_monitor(&mut self) -> Result<(), SurfaceError> {
        self.input_monitor
            .take()
            .map(|mut monitor| monitor.stop())
            .unwrap_or(Ok(()))
    }

    pub fn unmount(&mut self) -> Result<(), SurfaceError> {
        if !self.state.is_mounted() {
            return Ok(());
        }
        let input_result = self.stop_input_monitor();
        let display_result = self.stop_display_link();
        let surface_result = self.surface.unmount();
        self.state.unmount();
        input_result.and(display_result).and(surface_result)
    }

    pub const fn state(&self) -> ImageRenderHostState {
        self.state
    }

    pub fn surface(&self) -> &MacImageSurface {
        &self.surface
    }
}

fn input_rect(layout: SurfaceLayout) -> Result<InputRect, SurfaceError> {
    InputRect::new(layout.left, layout.top, layout.width, layout.height)
        .map_err(|_| SurfaceError::InvalidGeometry)
}

impl Drop for MacImageRenderHost {
    fn drop(&mut self) {
        let _ = self.unmount();
    }
}
