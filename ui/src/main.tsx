import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import App from './App'
import { defined } from './defined'
import './styles/tokens.css'
import './styles/primitives.css'
import './styles/app.css'
import './styles/filePreviewExtensions.css'
import './styles/adaptiveOtherFilePanel.css'

createRoot(defined(document.getElementById('root'), 'Missing root element')).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
