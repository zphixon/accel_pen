import { createRoot } from "react-dom/client";
import * as types from "./bindings/index";
import { useEffect, useState } from "react";
import TagSelect from "./components/TagSelect";
import * as api from "./api.js";

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
  let paramSelectedTags: types.TagInfo[] = [];
  let [params, setParams] = useSearchParams();
  params.forEach((value, key, _) => {
    if (key == "tagged_with") {
      for (let tagName of value.split(",")) {
        let tag = tagInfo.find(tag => tag.name == tagName);
        if (tag) {
          paramSelectedTags.push(tag);
        }
      }
    }
  });

  let [selectedTags, setSelectedTags] = useState<types.TagInfo[]>(paramSelectedTags);

  function updateParamsForTags(newSelectedTags: types.TagInfo[]) {
    params.set("tagged_with", newSelectedTags.map(tag => tag.name).join(","));
    setParams(params);
    console.log("hmm", newSelectedTags);
    setSelectedTags(newSelectedTags);
  }

  let [searchResponse, setSearchResponse] = useState<types.MapSearchResponse | types.TsApiError | undefined>(undefined);
  useEffect(() => {
    api.mapSearch({ tagged_with: selectedTags }).then(setSearchResponse);
  }, [params]);

  let dateFormat = new Intl.DateTimeFormat(undefined, {
    dateStyle: "full",
    timeStyle: "short",
  })
  let searchResults = [];
  if (searchResponse?.type == "MapSearchResponse") {
    searchResponse.maps.sort((a, b) => a.uploaded.localeCompare(b.uploaded));
    for (let map of searchResponse.maps) {
      searchResults.push(<div className="searchResult" key={map.gbx_uid}>
        <div><a href={"/map/" + map.id}>{map.plain_name}</a></div>
        <div>{map.author.display_name}</div>
        <div>{dateFormat.format(new Date(map.uploaded))}</div>
      </div>);
    }
  }

  return <>
    <TagSelect
      tagInfo={tagInfo}
      selectedTags={selectedTags}
      setSelectedTags={updateParamsForTags}
      maxTags={7}
    />
    <div id="searchResults">
      {searchResults}
    </div>
  </>;
}

let tagInfoNode = document.getElementById("tagData")!;
let tagInfo: types.TagInfo[] = JSON.parse(tagInfoNode.innerText);

let searchNode = document.getElementById("search")!;
let searchRoot = createRoot(searchNode);
searchRoot.render(<MapSearch />);
