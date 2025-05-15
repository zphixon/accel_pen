import { createRoot } from "react-dom/client";
import { useEffect, useState } from "react";
import * as api from "./api";
import * as types from "./bindings/index";
import TagSelect from "./components/TagSelect";
import { TagBadge } from "./components/TagBadge";

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
    dateStyle: "medium",
    timeStyle: "short",
  })

  let [matchAll, setMatchAll] = useState(false);
  let searchResults = [];
  if (searchResponse?.type == "MapSearchResponse") {
    searchResponse.maps.sort((a, b) => a.uploaded.localeCompare(b.uploaded));
    searchResponse.maps.reverse();
    for (let map of searchResponse.maps) {
      if (
        matchAll
        && selectedTags.filter(selected =>
          // retain items from selectedTags that do not appear in map.tags
          map.tags.find(tag => tag.id == selected.id) == undefined
        ).length != 0
      ) {
        continue;
      }

      let tags = [];
      for (let tag of map.tags) {
        tags.push(<TagBadge key={tag.name + map.gbx_uid} tag={tag} searchLink />);
      }

      let uploadedDate = new Date(map.uploaded);
      let uploadedText = dateFormat.format(uploadedDate);

      searchResults.push(
        <div className="searchResult" key={map.gbx_uid}>
          <div><a href={"/map/" + map.id}>{map.plain_name}</a></div>
          <div className="tagList">{tags}</div>
          <div>{map.author.display_name}</div>
          <div>{uploadedText}</div>
        </div>
      );
    }
  }

  return <>
    <TagSelect
      tagInfo={tagInfo}
      selectedTags={selectedTags}
      setSelectedTags={updateParamsForTags}
      maxTags={7}
    />
    <input id="matchAll" type="checkbox" checked={matchAll} onChange={_ => setMatchAll(!matchAll)} />
    <label htmlFor="matchAll">Maps must have all tags</label>
    <hr/>
    <div id="searchResults">
      <div className="searchResult">
        <div>Name</div>
        <div>Tags</div>
        <div>Author</div>
        <div>Uploaded</div>
      </div>
      {searchResults}
    </div>
  </>;
}

let tagInfoNode = document.getElementById("tagData")!;
let tagInfo: types.TagInfo[] = JSON.parse(tagInfoNode.innerText);

let searchNode = document.getElementById("search")!;
let searchRoot = createRoot(searchNode);
searchRoot.render(<MapSearch />);
