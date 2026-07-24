# Biome baseline exceptions

The UI keeps Biome 2.5.5's `recommended` rule set enabled. The narrowly scoped rules
below are a recorded baseline, not a permanent waiver. Adding a rule requires updating
the repository policy test and an architecture review.

## React dependency lifecycle

- `correctness/useExhaustiveDependencies` is off because the existing hooks deliberately
  use identity-specific dependency keys and refs to control request cancellation, focus,
  drag cleanup, and image revision lifetimes. Converting them to Biome's suggested
  dependency arrays changes when those effects run. Existing component tests, TypeScript
  checking, and request-revision/cancellation guards protect the current contract. Review
  and remove this exception after the planned React decomposition makes each effect's
  runtime ownership independently testable.

## Existing DOM interaction contracts

- `a11y/noNoninteractiveTabindex` is off because current composite preview/workspace
  surfaces retain focus for their existing keyboard interaction contract. Removing focus
  changes that contract. Existing interaction tests cover the relevant keyboard paths.
- `a11y/noRedundantRoles` is off because the current explicit landmark and list roles are
  asserted by the existing UI contract. Removing them changes the tested DOM semantics.
- `a11y/noStaticElementInteractions` is off because native `summary`, pointer-driven
  radial-menu, and rendered-markdown surfaces have established event handling. Changing
  their roles changes DOM semantics and requires dedicated behavior tests.
- `a11y/useAriaPropsSupportedByRole` is off because the current controlled `summary` and
  menu structures expose existing state attributes. Correcting the role/markup requires a
  separate accessibility design and behavior-test change.
- `a11y/useKeyWithClickEvents` is off because the current Markdown click surface's event
  contract needs a dedicated keyboard design, not an automatic handler addition.
- `a11y/useSemanticElements` is off because replacing the current group and text-entry
  structures changes DOM semantics and styling behavior. Handle it in an independently
  designed, behavior-tested accessibility change.

## Markdown rendering boundary

- `security/noDangerouslySetInnerHtml` is off because Markdown HTML is already sanitized
  by the backend rendering contract before it reaches this UI. Replacing the JSX rendering
  path changes the established DOM timing and requires dedicated security and behavior
  tests. The backend sanitizer, IPC validation, and Markdown-preview tests remain the
  current protection. Revisit this exception with a separately designed Markdown rendering
  boundary and tests.

Every a11y/DOM/Markdown exception above must be reconsidered only with an independent
design and behavior-test change; it is not permission to weaken the rule set globally.
