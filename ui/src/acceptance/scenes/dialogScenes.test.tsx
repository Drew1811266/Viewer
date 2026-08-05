import { render, screen, waitFor } from '@testing-library/react'
import { expect, it } from 'vitest'
import { defined } from '../../defined'
import type { AcceptanceRequest } from '../acceptanceRequest'
import { ACCEPTANCE_STATE_DEFINITIONS } from '../acceptanceStateCatalog'
import { DIALOG_SCENES } from './dialogScenes'

const request: AcceptanceRequest = {
  id: 'DIA-01',
  viewport: '1024x720',
  width: 1024,
  height: 720,
}

it('covers every dialog acceptance state exactly once in ledger order', () => {
  expect(Object.keys(DIALOG_SCENES)).toEqual(
    ACCEPTANCE_STATE_DEFINITIONS.filter(({ sceneGroup }) => sceneGroup === 'dialog').map(
      ({ id }) => id,
    ),
  )
})

it('grounds rename dialogs in valid atlas-equivalent content', async () => {
  const single = renderScene('DIA-02')
  await waitFor(() =>
    expect(screen.getByRole('textbox', { name: '新文件名' })).toHaveValue('A01-正面-01.jpg'),
  )
  expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  single.unmount()

  const batch = renderScene('DIA-03')
  await waitFor(() => expect(screen.getByRole('textbox', { name: '前缀' })).toHaveValue('精选-'))
  expect(await screen.findByText('衣服/A01/精选-01商品-01.jpg')).toBeVisible()
  batch.unmount()
})

it('uses the approved three-item trash confirmation fixture', () => {
  const rendered = renderScene('DIA-06')
  expect(screen.getByRole('dialog', { name: '将文件移到废纸篓？' })).toHaveTextContent(
    '将 3 项移入 macOS 废纸篓',
  )
  rendered.unmount()
})

function renderScene(id: string) {
  const Scene = defined(DIALOG_SCENES[id], `Missing ${id} acceptance scene`)
  return render(<Scene request={{ ...request, id }} />)
}
