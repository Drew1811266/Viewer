import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { defined } from '../../defined'
import type { AcceptanceRequest } from '../acceptanceRequest'
import { VIEWING_SCENES } from './viewingScenes'

const request: AcceptanceRequest = {
  id: 'PRE-01',
  viewport: '1024x720',
  width: 1024,
  height: 720,
}

describe('Viewer viewing acceptance scenes', () => {
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
})
