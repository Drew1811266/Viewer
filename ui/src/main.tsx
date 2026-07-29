import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import App from './App'
import { defined } from './defined'
import './styles/app.css'
import './styles/adaptiveOtherFilePanel.css'

createRoot(defined(document.getElementById('root'), 'Missing root element')).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
