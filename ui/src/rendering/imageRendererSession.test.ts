import { describe, expect, it } from 'vitest'
import {
  assertProtocolInteger,
  ImageRendererReplayState,
  numericSessionId,
  OrderedCommandLane,
} from './imageRendererSession'
import type { ImageRendererAnnotation, ImageRendererCommand } from './imageRendererTypes'

const node = (id: string, selected = false): ImageRendererAnnotation => ({
  id,
  ordinal: 1,
  geometry: { type: 'point', position: { x: 0.2, y: 0.3 } },
  style: { color: [1, 0, 0, 1], lineWidthPx: 2, dashed: false },
  selected,
  draft: false,
  visible: true,
})

const command = (
  value: ImageRendererCommand['command'],
  sceneRevision = 1,
): ImageRendererCommand => ({
  sceneRevision,
  command: value,
})

describe('image renderer session primitives', () => {
  it('validates positive protocol integers and decimal session ids', () => {
    expect(numericSessionId('41')).toBe(41)
    expect(() => numericSessionId('0')).toThrow('positive')
    expect(() => numericSessionId('x')).toThrow('positive')
    expect(() => assertProtocolInteger(-1, 'revision')).toThrow('safe protocol integer')
    expect(() => assertProtocolInteger(0, 'revision', false)).toThrow('safe protocol integer')
  })

  it('serializes a rejecting command lane before accepting the next command', async () => {
    const lane = new OrderedCommandLane()
    const events: string[] = []
    await expect(
      lane.enqueue(async () => {
        events.push('first')
        throw new Error('first failed')
      }),
    ).rejects.toThrow('first failed')
    await lane.enqueue(async () => {
      events.push('second')
    })
    expect(events).toEqual(['first', 'second'])
  })

  it('replays the latest surface, exclusions, tool, scene, camera and magnifier', () => {
    const replay = new ImageRendererReplayState()
    replay.record(
      command({
        type: 'set_surface',
        surface: { left: 0, top: 0, width: 10, height: 10, scaleFactor: 2 },
      }),
    )
    replay.record(command({ type: 'set_input_exclusions', exclusions: [] }))
    replay.record(command({ type: 'set_tool', tool: 'arrow' }))
    replay.record(
      command({ type: 'set_scene', scene: { annotations: [node('one')], draft: null } }),
    )
    replay.record(
      command({
        type: 'camera',
        camera: { mode: 'free', zoom: 2, rotation: 'deg90', offset: { x: 1, y: 2 } },
      }),
    )
    replay.record(command({ type: 'set_magnifier', magnifier: null }))

    expect(replay.commands()).toHaveLength(6)
    expect(replay.commands().every((entry) => entry.sceneRevision === 1)).toBe(true)
  })

  it('applies every scene patch deterministically and preserves the highest revision', () => {
    const replay = new ImageRendererReplayState()
    replay.record(
      command({ type: 'set_scene', scene: { annotations: [node('one')], draft: null } }, 2),
    )
    replay.record(
      command(
        {
          type: 'apply_scene_patch',
          patch: { type: 'upsert', baseRevision: 2, node: node('two') },
        },
        3,
      ),
    )
    replay.record(
      command(
        { type: 'apply_scene_patch', patch: { type: 'set_selection', baseRevision: 3, id: 'two' } },
        4,
      ),
    )
    replay.record(
      command(
        {
          type: 'apply_scene_patch',
          patch: { type: 'set_draft', baseRevision: 4, node: node('draft') },
        },
        5,
      ),
    )
    replay.record(
      command(
        { type: 'apply_scene_patch', patch: { type: 'remove', baseRevision: 5, id: 'one' } },
        6,
      ),
    )
    replay.record(
      command(
        {
          type: 'apply_scene_patch',
          patch: {
            type: 'replace_all',
            baseRevision: 6,
            annotations: [node('final')],
            draft: null,
          },
        },
        7,
      ),
    )

    const scene = replay.commands().find((entry) => entry.command.type === 'set_scene')
    expect(scene).toEqual({
      sceneRevision: 7,
      command: { type: 'set_scene', scene: { annotations: [node('final')], draft: null } },
    })
  })
})
