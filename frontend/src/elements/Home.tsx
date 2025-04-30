import { Link } from "wouter";
import { Suspense, use } from "react";

import * as api from "../api";
import UserSummary from "./UserSummary";
import NandoString from "./NandoString";

import "./Home.css";

interface FavoriteMapsProps {
  favoriteMapsPromise: ReturnType<typeof api.favoriteMaps>,
}
function FavoriteMaps({ favoriteMapsPromise }: FavoriteMapsProps) {
  let favoriteMaps = use(favoriteMapsPromise);

  if (!Array.isArray(favoriteMaps)) {
    return <>Could not load favorite maps: {favoriteMaps.message}</>;
  } else {
    let faves = [];
    for (let fave of favoriteMaps) {
      // TODO MapSummary component
      faves.push(<div key={fave.uid}>
        <NandoString string={fave.name} /> by <UserSummary user={fave.author} />
      </div>);
    }

    return <>{faves}</>;
  }
}

interface MyMapsProps {
  favoriteMapsPromise: ReturnType<typeof api.allMapsBy>,
}
function MyMaps({ favoriteMapsPromise }: MyMapsProps) {
  let allMaps = use(favoriteMapsPromise);

  if (allMaps.type == 'TsApiError') {
    return <>Could not load favorite maps: {allMaps.message}</>;
  } else {
    let myMaps = [];
    for (let map of allMaps.maps) {
      myMaps.push(<div key={map.uid}>
        <Link to={`~/map/${map.map_id}`}>
          <NandoString string={map.name} /> by <UserSummary user={map.author} />
        </Link>
      </div>);
    }

    return <>{myMaps}</>;
  }
}

function Home() {
  let user = api.useLoggedInUser();

  let mode = <></>;
  if (import.meta.env.DEV) {
    mode = <p>dev bruh</p>;
  }

  let myMaps = <></>;
  let favoriteMaps = <></>;
  let uploadMaps = <>Log in to upload maps!</>;

  if (user?.type == 'UserResponse') {
    favoriteMaps = <Suspense fallback={"Loading favorite maps"}>
      My favorite maps:<br/>
      <FavoriteMaps favoriteMapsPromise={api.favoriteMaps()} />
    </Suspense>;

    myMaps = <Suspense fallback={"Loading my maps"}>
      Maps by me:<br/>
      <MyMaps favoriteMapsPromise={api.allMapsBy({ type: 'AllMapsByRequest', user_id: user.user_id })}/>
    </Suspense>;

    uploadMaps = <Link to="~/map/upload">Upload more</Link>;
  }

  return <>
    {mode}
    <div>{favoriteMaps}</div><br/>
    <div>{myMaps}</div><br/>
    <div>{uploadMaps}</div>
  </>;
}

export default Home
