import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { defined } from '../../defined'
import type { AcceptanceRequest } from '../acceptanceRequest'
import { ACCEPTANCE_STATE_DEFINITIONS } from '../acceptanceStateCatalog'
import { infoInspectorRendered, VIEWING_SCENES } from './viewingScenes'

const request: AcceptanceRequest = {
  id: 'PRE-01',
  viewport: '1024x720',
  width: 1024,
  height: 720,
}

describe('Viewer viewing acceptance scenes', () => {
  it('covers every viewing catalog state exactly once in ledger order', () => {
    expect(Object.keys(VIEWING_SCENES)).toEqual(
      ACCEPTANCE_STATE_DEFINITIONS.filter(({ sceneGroup }) => sceneGroup === 'viewing').map(
        ({ id }) => id,
      ),
    )
  })

  it('renders PRE-01 through the formal fitted ImagePreview behavior', async () => {
    const Scene = defined(VIEWING_SCENES['PRE-01'], 'Missing PRE-01 acceptance scene')
    render(<Scene request={request} />)

    expect(screen.getByRole('dialog', { name: /图片预览 商品-02\.jpg/ })).toBeVisible()
    expect(screen.getByRole('button', { name: '适应窗口' })).toHaveAttribute('aria-pressed', 'true')
    expect(screen.getByRole('button', { name: '按 100% 显示' })).toBeVisible()
    expect(screen.getByRole('button', { name: '顺时针旋转' })).toBeVisible()
    expect(screen.getByRole('button', { name: '关闭预览' })).toBeVisible()
    const image = await screen.findByRole('img', { name: '商品-02.jpg' })
    expect(image).toBeVisible()
    expect(image.getAttribute('src')).toContain(encodeURIComponent('商品-02.jpg'))

    expect(screen.getByText('100%', { selector: '.preview-scale-label' })).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: '放大' }))
    expect(screen.getByText('125%', { selector: '.preview-scale-label' })).toBeVisible()
  })

  it.each(['RAD-01', 'RAD-02', 'RAD-03', 'RAD-04', 'RAD-05', 'RAD-06', 'RAD-07'])(
    '%s renders the formal radial menu over the loaded three-item workspace context',
    async (id) => {
      const rendered = renderScene(id)
      expect(screen.getByRole('menu', { name: '文件操作' })).toBeVisible()
      expect(screen.getAllByRole('menuitem').length).toBeGreaterThan(0)
      await waitFor(() => expect(document.querySelector('.viewer-shell')).not.toBeNull())
      expect(document.querySelector('.radial-menu-center')).toHaveTextContent(
        id === 'RAD-05' ? '1 个文件' : '3 个文件',
      )
      rendered.unmount()
    },
  )

  it.each([
    ['PRE-01', 'fit'],
    ['PRE-02', 'original'],
    ['PRE-03', 'zoom'],
    ['PRE-04', 'rotate'],
    ['PRE-07', 'navigation'],
  ])('%s renders a live formal image preview in %s state', async (id, state) => {
    const rendered = renderScene(id)
    const dialog = screen.getByRole('dialog', { name: /图片预览 商品-02\.jpg/ })
    const image = await within(dialog).findByRole('img', { name: '商品-02.jpg' })
    expect(image).toBeVisible()
    if (state === 'original') {
      await waitFor(() =>
        expect(screen.getByRole('button', { name: '按 100% 显示' })).toHaveAttribute(
          'aria-pressed',
          'true',
        ),
      )
    }
    if (state === 'zoom') {
      await waitFor(() =>
        expect(screen.getByText('156%', { selector: '.preview-scale-label' })).toBeVisible(),
      )
    }
    if (state === 'rotate') {
      await waitFor(() => expect(image.getAttribute('style')).toContain('rotate(90deg)'))
    }
    if (state === 'navigation') {
      expect(screen.getByRole('button', { name: '上一张' })).toBeEnabled()
      expect(screen.getByRole('button', { name: '下一张' })).toBeEnabled()
    }
    rendered.unmount()
  })

  it('renders bounded loading and error image preview states', async () => {
    const loading = renderScene('PRE-05')
    expect(screen.getByRole('status')).toHaveTextContent('正在载入图片')
    loading.unmount()

    const failed = renderScene('PRE-06')
    expect(await screen.findByRole('alert')).toHaveTextContent('无法显示这张图片')
    failed.unmount()
  })

  it('renders PRE-08 through the pressed real toolbar button and loaded original lens', async () => {
    const rendered = renderScene('PRE-08')

    await waitFor(() =>
      expect(screen.getByRole('button', { name: '放大镜' })).toHaveAttribute(
        'aria-pressed',
        'true',
      ),
    )
    const lens = await screen.findByTestId('image-magnifier')
    const source = screen.getByTestId('image-magnifier-source')
    await waitFor(() => expect(lens).toHaveAttribute('data-visible', 'true'))
    await waitFor(() =>
      expect(lens.closest('[data-acceptance-scene-ready]')).toHaveAttribute(
        'data-acceptance-scene-ready',
        'true',
      ),
    )
    expect(lens).toHaveAttribute('data-shape', 'circle')
    expect(lens).toHaveStyle({ width: '160px', height: '160px' })
    expect(lens.style.getPropertyValue('--magnifier-scale')).toBe('1.5')
    expect(lens.style.getPropertyValue('--magnifier-x')).toBe('418px')
    expect(lens.style.getPropertyValue('--magnifier-y')).toBe('338px')
    expect(lens.style.getPropertyValue('--magnifier-x')).not.toBe('320px')
    expect(getComputedStyle(lens.parentElement as HTMLElement).cursor).not.toBe('none')
    expect(source.getAttribute('src')).toContain(encodeURIComponent('商品-02.jpg'))
    expect(source.getAttribute('src')).toContain('representation=original100_percent')
    rendered.unmount()
  })

  it.each([
    ['COM-01', 2],
    ['COM-02', 3],
    ['COM-03', 4],
    ['COM-04', 8],
  ])('%s renders %d source files through CompareWorkspace', async (id, count) => {
    const rendered = renderScene(id)
    const workspace = screen.getByRole('region', { name: '图片对比' })
    await waitFor(() => expect(workspace).toHaveTextContent(`${count} 张图片`))
    expect(within(workspace).getByRole('toolbar', { name: '对比工具' })).toBeVisible()
    rendered.unmount()
  })

  it.each([
    ['DOC-01', 'Viewer 视觉验收'],
    ['DOC-02', 'Viewer 确定性的纯文本预览。'],
    ['DOC-03', '需要选择文本编码'],
    ['DOC-04', '内容已截断'],
  ])('%s renders its formal document state', async (id, visibleText) => {
    const rendered = renderScene(id)
    expect(screen.getByRole('toolbar', { name: '文本预览工具' })).toBeVisible()
    expect(await screen.findByText(visibleText, { exact: false })).toBeVisible()
    rendered.unmount()
  })

  it('renders independent dual text panes and both unsupported states', async () => {
    const dual = renderScene('DOC-05')
    await waitFor(() => expect(screen.getAllByTestId(/^text-pane-/)).toHaveLength(2))
    dual.unmount()

    const unsupported = renderScene('DOC-06')
    expect(screen.getByLabelText(/unsupported\.bin.*暂不支持预览/)).toBeVisible()
    unsupported.unmount()

    const unavailable = renderScene('DOC-07')
    expect(screen.getByLabelText(/unsupported\.bin.*文件已不可用/)).toBeVisible()
    unavailable.unmount()
  })

  it.each(['INF-01', 'INF-02'])(
    '%s renders the formal file inspector in the workspace',
    async (id) => {
      const rendered = renderScene(id)
      const inspector = screen.getByRole('complementary', { name: '文件信息' })
      expect(inspector).toBeVisible()
      await waitFor(() => expect(document.querySelector('.viewer-shell')).not.toBeNull())
      expect(infoInspectorRendered(document)).toBe(true)
      if (id === 'INF-01') expect(inspector).toHaveTextContent('商品-02.jpg')
      if (id === 'INF-02') {
        expect(inspector).toHaveTextContent('文件夹 1 · 图片 1 · 其它文件 1')
      }
      rendered.unmount()
    },
  )
})

function renderScene(id: string) {
  const Scene = defined(VIEWING_SCENES[id], `Missing ${id} acceptance scene`)
  const [width, height] = request.viewport.split('x').map(Number)
  return render(
    <Scene
      request={{
        ...request,
        id,
        width: width === 1024 ? 1024 : 1440,
        height: height === 720 ? 720 : 900,
      }}
    />,
  )
}
