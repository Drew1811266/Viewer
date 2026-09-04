import type {
  ImageRendererCommand,
  ImageRendererScene,
  ImageRendererScenePatch,
} from './imageRendererTypes'

export function assertProtocolInteger(value: number, name: string, allowZero = true): void {
  if (!Number.isSafeInteger(value) || value < (allowZero ? 0 : 1)) {
    throw new Error(`${name} must be a safe protocol integer`)
  }
}

export function numericSessionId(sessionId: string): number {
  if (!/^[1-9]\d*$/.test(sessionId)) throw new Error('sessionId must be a positive decimal integer')
  const numeric = Number(sessionId)
  assertProtocolInteger(numeric, 'sessionId', false)
  return numeric
}

export class OrderedCommandLane {
  private tail: Promise<void> = Promise.resolve()

  enqueue<T>(operation: () => Promise<T>): Promise<T> {
    const result = this.tail.then(operation, operation)
    this.tail = result.then(
      () => undefined,
      () => undefined,
    )
    return result
  }
}

/** A bounded semantic checkpoint used only to hydrate the legacy renderer. */
export class ImageRendererReplayState {
  private revision = 0
  private surface?: ImageRendererCommand
  private exclusions?: ImageRendererCommand
  private tool?: ImageRendererCommand
  private scene?: ImageRendererScene
  private camera?: ImageRendererCommand
  private magnifier?: ImageRendererCommand

  record(command: ImageRendererCommand): void {
    this.revision = Math.max(this.revision, command.sceneRevision)
    switch (command.command.type) {
      case 'set_surface':
        this.surface = command
        break
      case 'set_input_exclusions':
        this.exclusions = command
        break
      case 'set_tool':
        this.tool = command
        break
      case 'set_scene':
        this.scene = command.command.scene
        break
      case 'apply_scene_patch':
        this.scene = applyScenePatch(
          this.scene ?? { annotations: [], draft: null },
          command.command.patch,
        )
        break
      case 'camera':
        this.camera = command
        break
      case 'set_magnifier':
        this.magnifier = command
        break
    }
  }

  commands(): ImageRendererCommand[] {
    const revision = this.revision
    const commandAtRevision = (entry: ImageRendererCommand | undefined) =>
      entry === undefined ? [] : [{ ...entry, sceneRevision: revision }]
    return [
      ...commandAtRevision(this.surface),
      ...commandAtRevision(this.exclusions),
      ...commandAtRevision(this.tool),
      ...(this.scene === undefined
        ? []
        : [
            { sceneRevision: revision, command: { type: 'set_scene' as const, scene: this.scene } },
          ]),
      ...commandAtRevision(this.camera),
      ...commandAtRevision(this.magnifier),
    ]
  }
}

function applyScenePatch(
  scene: ImageRendererScene,
  patch: ImageRendererScenePatch,
): ImageRendererScene {
  switch (patch.type) {
    case 'upsert': {
      const existing = scene.annotations.findIndex((annotation) => annotation.id === patch.node.id)
      const annotations = [...scene.annotations]
      if (existing === -1) annotations.push(patch.node)
      else annotations[existing] = patch.node
      return { ...scene, annotations }
    }
    case 'remove':
      return {
        ...scene,
        annotations: scene.annotations.filter((annotation) => annotation.id !== patch.id),
      }
    case 'replace_all':
      return { annotations: patch.annotations, draft: patch.draft }
    case 'set_selection':
      return {
        ...scene,
        annotations: scene.annotations.map((annotation) => ({
          ...annotation,
          selected: annotation.id === patch.id,
        })),
      }
    case 'set_draft':
      return { ...scene, draft: patch.node }
  }
}
