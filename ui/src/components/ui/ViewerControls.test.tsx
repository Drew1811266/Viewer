import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import ViewerButton, { ViewerIconButton } from './ViewerButton'
import ViewerChoiceChip from './ViewerChoiceChip'
import ViewerField from './ViewerField'
import ViewerSegmentedControl from './ViewerSegmentedControl'
import ViewerStatusTag from './ViewerStatusTag'

describe('Viewer control primitives', () => {
  it('exposes button tone, active and loading state without changing its label', () => {
    const rendered = render(
      <ViewerButton tone="primary" active>
        完成
      </ViewerButton>,
    )
    const button = screen.getByRole('button', { name: '完成' })
    expect(button).toHaveAttribute('data-tone', 'primary')
    expect(button).toHaveAttribute('aria-pressed', 'true')

    rendered.rerender(
      <ViewerButton tone="primary" loading>
        完成
      </ViewerButton>,
    )
    expect(screen.getByRole('button', { name: '完成' })).toHaveAttribute('aria-busy', 'true')
    expect(screen.getByRole('button', { name: '完成' })).toBeDisabled()
  })

  it('requires a visible accessible name for icon-only buttons', () => {
    const action = vi.fn()
    const rendered = render(<ViewerIconButton icon="x" label="关闭筛选" onClick={action} />)
    const button = screen.getByRole('button', { name: '关闭筛选' })
    expect(button).toHaveClass('viewer-icon-button')
    expect(button.querySelector('img')).toHaveAttribute('aria-hidden', 'true')
    fireEvent.click(button)
    expect(action).toHaveBeenCalledOnce()

    rendered.rerender(<ViewerIconButton icon="x" label="关闭筛选" disabled onClick={action} />)
    fireEvent.click(screen.getByRole('button', { name: '关闭筛选' }))
    expect(action).toHaveBeenCalledOnce()
  })

  it('keeps choice chips as real multi-select checkboxes', () => {
    const change = vi.fn()
    render(
      <ViewerChoiceChip checked={false} onCheckedChange={change}>
        JPEG
      </ViewerChoiceChip>,
    )
    fireEvent.click(screen.getByRole('checkbox', { name: 'JPEG' }))
    expect(change).toHaveBeenCalledWith(true)
  })

  it('labels native fields and gives errors an alert role', () => {
    render(
      <ViewerField label="文件名" hint="保留扩展名" error="名称不能为空">
        <input defaultValue="" />
      </ViewerField>,
    )
    expect(screen.getByRole('textbox', { name: '文件名' })).toBeVisible()
    expect(screen.getByText('保留扩展名')).toBeVisible()
    expect(screen.getByRole('alert')).toHaveTextContent('名称不能为空')
  })

  it('renders non-color status text and a named segmented toolbar', () => {
    render(
      <>
        <ViewerStatusTag tone="warning">只读</ViewerStatusTag>
        <ViewerSegmentedControl label="图片显示控制">
          <ViewerButton active>适应窗口</ViewerButton>
          <ViewerButton>100%</ViewerButton>
        </ViewerSegmentedControl>
      </>,
    )
    expect(screen.getByText('只读')).toHaveAttribute('data-tone', 'warning')
    expect(screen.getByRole('toolbar', { name: '图片显示控制' })).toHaveClass(
      'viewer-segmented-control',
    )
  })
})
