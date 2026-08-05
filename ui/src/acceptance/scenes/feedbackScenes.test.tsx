import { readFileSync } from 'node:fs'
import { render } from '@testing-library/react'
import { expect, it } from 'vitest'
import { ACCEPTANCE_STATE_DEFINITIONS } from '../acceptanceStateCatalog'
import { FEEDBACK_SCENES, navigateToLocalFeedbackFolder } from './feedbackScenes'

const acceptanceCss = readFileSync('src/acceptance/acceptance.css', 'utf8')

it('covers every feedback and accessibility state exactly once in ledger order', () => {
  expect(Object.keys(FEEDBACK_SCENES)).toEqual(
    ACCEPTANCE_STATE_DEFINITIONS.filter(({ sceneGroup }) => sceneGroup === 'feedback').map(
      ({ id }) => id,
    ),
  )
})

it('exercises a real stacked-notice state in RES-02', () => {
  const Scene = FEEDBACK_SCENES['RES-02']
  expect(Scene).toBeDefined()
  if (Scene === undefined) throw new Error('Missing RES-02 acceptance scene')

  render(<Scene request={{ id: 'RES-02', viewport: '1024x720', width: 1024, height: 720 }} />)

  expect(document.querySelectorAll('.global-notice-stack > .viewer-local-feedback')).toHaveLength(2)
})

it('drives RES-03 from the collapsed parent to its image workspace', () => {
  document.body.innerHTML = '<button aria-label="展开 衣服"></button>'

  expect(navigateToLocalFeedbackFolder(document)).toBe('expanding')
  expect(
    document.querySelector('[data-acceptance-action="expand-local-feedback-parent"]'),
  ).not.toBeNull()

  document.body.innerHTML = '<div role="treeitem" aria-label="衣服/A01"></div>'
  expect(navigateToLocalFeedbackFolder(document)).toBe('opening')
  expect(
    document.querySelector('[data-acceptance-action="open-local-feedback-folder"]'),
  ).not.toBeNull()
})

it('keeps RES-03 visible above the product workspace without leaking acceptance CSS', () => {
  expect(acceptanceCss).toContain('.acceptance-local-feedback-stage {')
  expect(acceptanceCss).toContain('bottom: 32px;')
  expect(acceptanceCss).toContain('left: 244px;')
  expect(acceptanceCss).toContain('position: fixed;')
  expect(acceptanceCss).toContain('z-index: 19;')
  expect(acceptanceCss).toContain('body:has(.task-bar) .acceptance-local-feedback-stage {')
  expect(acceptanceCss).toContain('bottom: 92px;')
})
