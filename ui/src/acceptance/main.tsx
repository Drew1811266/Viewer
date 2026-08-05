import { createRoot } from 'react-dom/client'
import { defined } from '../defined'
import '../styles/tokens.css'
import '../styles/primitives.css'
import '../styles/app.css'
import '../styles/filePreviewExtensions.css'
import '../styles/adaptiveOtherFilePanel.css'
import AcceptanceApp from './AcceptanceApp'
import { parseAcceptanceRequest } from './acceptanceRequest'
import { ACCEPTANCE_SCENES } from './scenes'

const root = createRoot(defined(document.getElementById('root'), 'Missing acceptance root element'))

try {
  root.render(
    <AcceptanceApp
      request={parseAcceptanceRequest(window.location.search)}
      sceneRegistry={ACCEPTANCE_SCENES}
    />,
  )
} catch (caught: unknown) {
  const message = caught instanceof Error ? caught.message : String(caught)
  root.render(
    <main data-acceptance-bootstrap-error role="alert">
      {message}
    </main>,
  )
}
