import { describe, expect, it } from 'vitest'
import type { RadialMenuContext } from './radialMenuModel'
import { buildRadialMenuModel } from './radialMenuModel'

function context(overrides: Partial<RadialMenuContext> = {}): RadialMenuContext {
  return {
    selectedCount: 1,
    selectedImageCount: 1,
    readOnly: false,
    busy: false,
    compareContextAvailable: true,
    commonReview: null,
    commonFavorite: false,
    ...overrides,
  }
}

describe('buildRadialMenuModel', () => {
  it('keeps the six primary positions stable', () => {
    expect(buildRadialMenuModel(context()).map((item) => item.id)).toEqual([
      'preview',
      'mark',
      'organize',
      'trash',
      'compare',
      'info',
    ])
  })

  it('builds the confirmed local fan submenus', () => {
    const model = buildRadialMenuModel(context())
    expect(model[1]?.children?.map((item) => item.id)).toEqual([
      'mark.keep',
      'mark.pending',
      'mark.reject',
      'mark.clear',
      'mark.favorite',
    ])
    expect(model[2]?.children?.map((item) => item.id)).toEqual([
      'organize.rename',
      'organize.copy',
      'organize.move',
    ])
  })

  it('gates preview and compare by selection shape without moving them', () => {
    const single = buildRadialMenuModel(context())
    expect(single[0]).toMatchObject({ id: 'preview', disabled: false })
    expect(single[4]).toMatchObject({ id: 'compare', disabled: true })

    const threeImages = buildRadialMenuModel(context({ selectedCount: 3, selectedImageCount: 3 }))
    expect(threeImages[0]).toMatchObject({ id: 'preview', disabled: true })
    expect(threeImages[4]).toMatchObject({ id: 'compare', disabled: false })

    const mixed = buildRadialMenuModel(context({ selectedCount: 3, selectedImageCount: 2 }))
    expect(mixed[4]).toMatchObject({ id: 'compare', disabled: true })
  })

  it('explains an unavailable compare context separately from an invalid selection shape', () => {
    const unavailable = buildRadialMenuModel(
      context({
        selectedCount: 2,
        selectedImageCount: 2,
        compareContextAvailable: false,
      }),
    )

    expect(unavailable[4]).toMatchObject({
      id: 'compare',
      disabled: true,
      disabledReason: '请先返回文件夹内容，再选择图片进行对比',
    })
  })

  it('disables writes in read-only and all competing actions while busy', () => {
    const readOnly = buildRadialMenuModel(context({ readOnly: true }))
    expect(readOnly[1]).toMatchObject({ disabled: true, disabledReason: '只读项目不可标记' })
    expect(readOnly[2]).toMatchObject({ disabled: true, disabledReason: '只读项目不可整理' })
    expect(readOnly[3]).toMatchObject({ disabled: true, disabledReason: '只读项目不可删除' })
    expect(readOnly[5]).toMatchObject({ disabled: false })

    const busy = buildRadialMenuModel(context({ busy: true }))
    expect(
      busy
        .filter((item) => ['mark', 'organize', 'trash', 'compare'].includes(item.id))
        .every((item) => item.disabled),
    ).toBe(true)
    expect(busy[0]).toMatchObject({ disabled: false })
    expect(busy[5]).toMatchObject({ disabled: false })
  })

  it('keeps preview limited to exactly one selection even while busy', () => {
    const busySingle = buildRadialMenuModel(context({ busy: true }))
    expect(busySingle[0]).toMatchObject({ id: 'preview', disabled: false })

    const busyMultiple = buildRadialMenuModel(
      context({ selectedCount: 3, selectedImageCount: 3, busy: true }),
    )
    expect(busyMultiple[0]).toMatchObject({
      id: 'preview',
      disabled: true,
      disabledReason: '预览仅适用于单个文件',
    })
  })

  it('reflects common marker state and batch rename copy', () => {
    const model = buildRadialMenuModel(
      context({
        selectedCount: 2,
        selectedImageCount: 2,
        commonReview: 'pending',
        commonFavorite: true,
      }),
    )
    expect(model[1]?.children?.find((item) => item.id === 'mark.pending')).toMatchObject({
      checked: true,
    })
    expect(model[1]?.children?.find((item) => item.id === 'mark.favorite')).toMatchObject({
      label: '取消收藏',
      checked: true,
    })
    expect(model[2]?.children?.[0]).toMatchObject({ label: '批量重命名' })
  })

  it('labels a mixed favorite selection as a toggle without promising set-favorite', () => {
    const favorite = buildRadialMenuModel(
      context({
        selectedCount: 2,
        selectedImageCount: 2,
        commonFavorite: 'mixed',
      }),
    )[1]?.children?.find((item) => item.id === 'mark.favorite')

    expect(favorite).toMatchObject({
      label: '切换收藏',
      checked: 'mixed',
    })
  })
})
