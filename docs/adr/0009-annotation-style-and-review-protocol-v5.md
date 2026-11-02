# ADR 0009: Annotation style and review protocol v5

> Status: Active (0.3.0)
>
> Decision: Annotation style is an optional, per-target field introduced in
> `viewer.review/5`. 0.3 reads v3/v4/v5, keeps v3 until an extended anchor is
> written, promotes to v4 for the extended anchors that 0.2.5 already knows,
> and promotes to v5 as soon as a target carries a style. v5 is never written
> by 0.2.5 and is not downgraded.
>
> Date: 2026-11-01

## Context

Viewer 0.2.5 supports `viewer.review/3` and `/4`. Version 4 added the point,
arrow, and ellipse anchors; its `Target` object is a strict
`deny_unknown_fields` record with no `style` field. 0.2.5 can therefore read
v4, but it rejects a v4 document that contains an unknown `style` key.

The 0.3.0 plan originally proposed v5. During implementation the optional
`style` field was briefly carried inside v4, which looked like a smaller
change. It is not compatible: 0.2.5's v4 decoder would reject every 0.3 v4
record that stores a style, even though the document still says
`viewer.review/4`. A new version is required for a forward-compatible field.

0.3.0 also adds five tools on top of the v4 anchors. Text, line, and polygon
need new anchor variants that 0.2.5's strict v4 decoder does not know; brush
and rectangle already exist. The style belongs to the target (one opinion can
mark several regions with different tools), not to the feedback text.

## Decision

- **v5 is v4 plus the text/line/polygon anchors and the optional target
  style.** The wire shape is the same strict v3/v4 model; `Target` gains
  `style?: { tool, color, width, opacityPercent, dashed, textSize? }`.
  The field is omitted when absent.
- **Version selection is monotonic:**
  - A new stream starts at v3.
  - The first target with a v4-only anchor (point, arrow, ellipse) promotes
    the stream to v4.
  - The first target with a v5-only anchor (text, line, polygon) or a stored
    style promotes the stream to v5, regardless of whether the anchor is
    v3/v4-compatible.
  - A v5 stream never downgrades; withdrawing the styled target does not
    return it to v4.
- **Compatibility:**
  - 0.2.5 reads v3 and v4, and rejects v5 (unknown protocol version).
  - 0.3 reads v3, v4, and v5; v3/v4 targets without style render with the
    anchor's default style.
  - 0.3 writes v5 when a text/line/polygon anchor or a style is present. A
    0.2.5 reader therefore never sees a v4 document with an unknown anchor or
    `style` field from 0.3.
  - The 0.3 v4 decoder is lenient about an unexpected style on development
    data, but no 0.3 writer produces it. 0.2.5 remains the strict bound.
- **Domain invariants.** `AnnotationStyle::new` enforces tool/anchor
  compatibility, opacity `20..=100`, dashed only for tools that support it,
  highlighter opacity exactly `35` and never dashed, and text size only for
  the text tool. Wire values are canonical enum names, not hex colors.
- **Immutability and evidence.** A target revision's style participates in the
  state hash, archive snapshots, restore/history, and the annotated evidence
  PNG. A restored v3/v4 target gets the anchor default; a restored v5 target
  keeps its style.
- **Defaults.** Without a stored style: red, medium width (2px), 100%
  opacity, solid; notes 14px; highlighter 35% and thick width.

## Consequences

- The protocol grows to `viewer.review/5`; v3 and v4 remain readable and
  restorable. There is no in-place migration of old files: promotion happens
  on the next save that needs the newer version.
- 0.2.5 can still open a v3 or style-less v4 project; it cannot open a v5
  project. Users must not downgrade after a style save.
- The v4 JSON schemas remain valid for style-less documents; v5 schemas add
  the optional `style` object.
- Adding a future tool requires a new style enum value and default rule. It
  does not require a new protocol version unless the target shape changes
  incompatibly.
- The native renderer, Canvas/DOM fallback, and evidence PNG consume the same
  style values; there is no second source of truth.

## Alternatives considered

- **Keep style inside v4.** Rejected: 0.2.5's strict v4 decoder rejects the
  field; this silently breaks the documented v4 compatibility promise.
- **Style on the feedback instead of the target.** Rejected: multi-region
  opinions need per-region tools/colors.
- **Embed style inside the anchor.** Rejected: anchors are geometry and are
  hashed/compared as geometry; presentation does not belong there.
- **Derive everything from the tool at render time.** Rejected: users change
  color/width per annotation; the style must survive save, archive, and
  evidence generation.
- **A sidecar style file next to the stream.** Rejected: two atomicity
  domains, no archive/hash coverage, and a second source of truth.
