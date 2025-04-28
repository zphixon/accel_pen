import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { Toaster } from 'react-hot-toast'
import { Route, Switch, useSearchParams } from 'wouter'

import Home from './elements/Home.tsx'
import MapView from './elements/map/MapView.tsx'
import NotFound from './elements/NotFound.tsx'
import MapUpload from './elements/map/MapUpload.tsx'

import * as api from './api.tsx';

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <Switch>
      <Route path="/"><Home /></Route>

      <Route nest path="/map">
        <Switch>
          <Route path="/upload"><MapUpload /></Route>
          <Route path="/:mapId">{params => <MapView mapId={params.mapId}/>}</Route>
          <Route><NotFound /></Route>
        </Switch>
      </Route>

      <Route path="/login" component={() => {
        let [searchParams, _] = useSearchParams();
        window.location.href = api.oauthStartUrl(searchParams.get("returnPath")).href;
        return null;
      }} />

      <Route><NotFound /></Route>
    </Switch>
    <Toaster />
  </StrictMode>,
)
