use std::sync::Arc;
use viewer_infrastructure::operation::service::LocalFileCommandAdapter;

pub fn adapter_as_undo_port(
    adapter: Arc<LocalFileCommandAdapter>,
) -> Arc<dyn viewer_application::undo::UndoFilePort> {
    adapter
}
