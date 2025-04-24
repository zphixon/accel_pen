import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { BrowserRouter, Route, Routes } from 'react-router'
import { Toaster } from 'react-hot-toast'

import Home from './elements/Home.tsx'
import MapView from './elements/map/MapView.tsx'
import NotFound from './elements/NotFound.tsx'
import MapUpload from './elements/map/MapUpload.tsx'
import LoginPage from './elements/LoginPage.tsx'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <BrowserRouter>
      <Routes>
        <Route index element={<Home />} />

        <Route path="map">
          <Route path=":mapId" element={<MapView />} />
          <Route path="upload" element={<MapUpload />} />
        </Route>

        <Route path="login">
          <Route path="" element={<LoginPage oauthRedirect={false} />} />
          <Route path="oauth" element={<LoginPage oauthRedirect={true} />}/>
        </Route>

        <Route path="*" element={<NotFound />}/>
      </Routes>
    </BrowserRouter>
    <Toaster />
  </StrictMode>,
)
