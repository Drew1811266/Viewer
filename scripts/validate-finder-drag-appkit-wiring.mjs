const SESSION_API = 'beginDraggingSessionWithItems_event_source'
const EXACT_WIRING = `
  content_view.beginDraggingSessionWithItems_event_source(
    &items,
    &event,
    objc2::runtime::ProtocolObject::from_ref(&*source),
  );
`

export function validateFinderDragAppKitWiring(adapter) {
  if (typeof adapter !== 'string') {
    throw new Error('exact AppKit drag wiring requires Rust source text')
  }
  const callCount = adapter.match(new RegExp(`\\b${SESSION_API}\\b`, 'g'))?.length ?? 0
  if (callCount !== 1 || !compact(adapter).includes(compact(EXACT_WIRING))) {
    throw new Error(
      'exact AppKit drag wiring must publish &items and the synthetic &event from content_view',
    )
  }
}

function compact(value) {
  return value.replace(/\s+/g, '')
}
