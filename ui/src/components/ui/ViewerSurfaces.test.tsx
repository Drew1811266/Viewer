import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import ViewerDialog from './ViewerDialog'
import ViewerEmptyState from './ViewerEmptyState'
import ViewerInspector from './ViewerInspector'
import ViewerLocalFeedback from './ViewerLocalFeedback'
import ViewerTaskSurface from './ViewerTaskSurface'
import ViewerToolbar from './ViewerToolbar'

describe('Viewer surface primitives', () => {
  it('renders a labelled inspector with a formal close control', () => {
    render(
      <ViewerInspector label="文件信息" title="信息" status="只读" onClose={vi.fn()}>
        内容
      </ViewerInspector>,
    )
    expect(screen.getByRole('complementary', { name: '文件信息' })).toHaveClass('viewer-inspector')
    expect(screen.getByRole('button', { name: '关闭信息' })).toBeVisible()
    expect(screen.getByText('只读')).toBeVisible()
  })

  it('renders a restrained empty state with an optional action', () => {
    render(
      <ViewerEmptyState
        title="此文件夹为空"
        description="这里还没有可查看的文件。"
        action={<button type="button">返回</button>}
      />,
    )
    expect(screen.getByRole('heading', { name: '此文件夹为空' })).toBeVisible()
    expect(screen.getByText('这里还没有可查看的文件。')).toBeVisible()
    expect(screen.getByRole('button', { name: '返回' })).toBeVisible()
  })

  it('exposes local feedback tone and non-color text meaning', () => {
    render(
      <ViewerLocalFeedback tone="danger" title="无法载入预览">
        文件已移动。
      </ViewerLocalFeedback>,
    )
    expect(screen.getByRole('alert')).toHaveAttribute('data-tone', 'danger')
    expect(screen.getByRole('alert')).toHaveTextContent('无法载入预览文件已移动。')
  })

  it('renders a named three-region toolbar', () => {
    render(<ViewerToolbar label="图片预览" leading="A.jpg" center="控制" actions="完成" />)
    const toolbar = screen.getByRole('toolbar', { name: '图片预览' })
    expect(toolbar).toHaveClass('viewer-toolbar')
    expect(toolbar).toHaveTextContent('A.jpg控制完成')
  })

  it('keeps native task progress semantics beside its visual line', () => {
    render(
      <ViewerTaskSurface label="缩略图生成进度" current={68} total={100}>
        正在生成缩略图
      </ViewerTaskSurface>,
    )
    expect(screen.getByRole('status', { name: '缩略图生成进度' })).toHaveStyle({
      '--viewer-progress': '0.68',
    })
    expect(screen.getByRole('progressbar', { name: '缩略图生成进度' })).toHaveAttribute(
      'value',
      '68',
    )
  })

  it('exposes dialog size, description and footer contracts', () => {
    render(
      <ViewerDialog
        title="选择目标文件夹"
        description="移动前先选择目标。"
        footer={<button type="button">继续</button>}
        onCancel={vi.fn()}
        size="large"
      >
        项目目录
      </ViewerDialog>,
    )
    const dialog = screen.getByRole('dialog', { name: '选择目标文件夹' })
    expect(dialog).toHaveAttribute('data-size', 'large')
    expect(dialog).toHaveAccessibleDescription('移动前先选择目标。')
    expect(dialog.querySelector('.viewer-dialog__footer')).toHaveTextContent('继续')
  })
})
