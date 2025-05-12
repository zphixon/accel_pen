import { createRoot } from "react-dom/client";
import * as types from "./bindings/index";
import { useEffect, useState } from "react";

function useSearchParams(): [URLSearchParams, (newParams: URLSearchParams) => void] {
  let [params, innerSetParams] = useState(new URLSearchParams(window.location.search));

  function setParams(newParams: URLSearchParams) {
    let newParamsObject = new URLSearchParams();
    newParams.forEach((value, key, _) => newParamsObject.set(key, value));
    innerSetParams(newParamsObject);

    // oh my god.
    let newUrl = new URL(window.location.href);
    newUrl.searchParams.forEach((_, key, params) => params.delete(key));
    newParamsObject.forEach((value, key, _) => newUrl.searchParams.set(key, value));

    // replace search params
    history.replaceState(null, "", newUrl);
  }

  return [params, setParams];
}

function MapSearch() {
  let [params, setParams] = useSearchParams();

  useEffect(() => {
    console.log("wowee");
  }, [params]);

  return <>
    <button onClick={_ => {
      params.set("holy", "moly");
      setParams(params);
    }}>juan</button>
    <button onClick={_ => {
      params.set("wowie", "zowie");
      setParams(params);
    }}>two</button>
  </>;
}

let tagInfoNode = document.getElementById("tagData")!;
let tagInfo: types.TagInfo[] = JSON.parse(tagInfoNode.innerText);

let searchNode = document.getElementById("search")!;
let searchRoot = createRoot(searchNode);
searchRoot.render(<MapSearch />);
