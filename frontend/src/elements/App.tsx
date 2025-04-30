import { Toaster } from 'react-hot-toast'
import { Route, Switch, useSearchParams } from 'wouter'

import Home from './Home.tsx'
import NotFound from './NotFound.tsx'
import MapUpload from './map/MapUpload.tsx'
import MapView from './map/MapView.tsx'
import NavBar from './NavBar.tsx'

import * as api from '../api.tsx';

function App() {
  return <>
    <NavBar />
    <Switch>
      <Route path="/"><Home /></Route>

      <Route nest path="/map">
        <Switch>
          <Route path="/upload"><MapUpload /></Route>
          <Route path="/:mapId">{params => <MapView mapId={params.mapId} />}</Route>
          <Route><NotFound /></Route>
        </Switch>
      </Route>

      <Route path="/login" component={() => {
        let [searchParams, _] = useSearchParams()
        window.location.href = api.oauthStartUrl(searchParams.get("returnPath")).href
        return null
      } } />

      <Route><NotFound /></Route>
    </Switch>
    <Toaster />
  </>;
}

export default App