# Biome baseline exceptions

The UI keeps Biome 2.5.5's recommended rule set enabled. The seven rules below are the
measured baseline, not a permanent waiver. Adding or removing one requires updating the
configuration, the bidirectional repository policy test, this measurement, and an
architecture review.

## React dependency lifecycle

- `correctness/useExhaustiveDependencies` has 23 diagnostics in App, ComparePane,
  CompareWorkspace, ContentBrowser, ImagePreview, RadialFileMenu, TextPreview, VirtualGrid,
  and the organization pointer-drag hook. These effects deliberately use identity keys and
  refs to control request cancellation, focus, drag cleanup, and image revision lifetimes;
  changing the arrays changes when they run. Request epochs, cancellation guards, cleanup
  tests, component tests, and TypeScript checking protect the current contract. Remove the
  exception only after React decomposition gives each effect independent lifecycle tests
  and the measured count reaches zero.

## Existing DOM interaction contracts

- `a11y/noNoninteractiveTabindex` has two diagnostics: the ComparePane article and
  CompareWorkspace section retain focus for their composite keyboard contracts. Existing
  focus, shortcut, pan, and zoom tests protect those paths. Remove the exception after an
  accessibility design assigns semantic interactive containers without changing focus
  return behavior and both diagnostics disappear.
- `a11y/noStaticElementInteractions` has four diagnostics across the ContentBrowser file
  export surfaces, the pointer-driven RadialFileMenu root, and rendered Markdown in
  TextPreview. Drag, radial-menu, and Markdown link tests protect the existing handlers.
  Remove the exception after each surface has a behavior-tested semantic interaction
  design and the measured count reaches zero.
- `a11y/useAriaPropsSupportedByRole` has nine diagnostics across CompareWorkspace,
  ContentBrowser, FolderFilmstripRow, FolderTree, RadialFileMenu, SearchToolbar, and the
  review-shortcut fixture. Component accessibility queries and interaction tests protect
  the exposed names and checked state. Remove the exception after compatible roles or
  elements preserve those accessible contracts and all nine diagnostics are eliminated.
- `a11y/useKeyWithClickEvents` has four diagnostics on the two ContentBrowser option
  families, FolderTree rows, and TextPreview Markdown. Parent composite-keyboard handling,
  selection tests, folder tests, and Markdown link tests protect current behavior. Remove
  the exception only after dedicated keyboard behavior is implemented and tested on every
  surface without duplicate activation.
- `a11y/useSemanticElements` has seven diagnostics: two App role surfaces, the
  FolderFilmstripRow region/list/items, and two SearchToolbar controls. DOM-role assertions,
  layout styling tests, and component interaction tests protect the current structure.
  Remove the exception after semantic replacements preserve roles, layout, and events and
  the measured count reaches zero.

## Markdown rendering boundary

- `security/noDangerouslySetInnerHtml` has one diagnostic at the TextPreview sanitized HTML
  boundary. The backend sanitizer, IPC validation, and Markdown preview/link tests protect
  the current rendering contract. Remove the exception after a separately designed
  rendering boundary proves equivalent sanitization, DOM timing, and link behavior without
  this sink.

Every exception must be reconsidered only with an independent design and behavior-test
change; none permits weakening an entire rule domain.
