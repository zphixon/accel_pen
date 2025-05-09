import { createRoot } from "react-dom/client";
import * as types from "./bindings/index";
import * as api from "./api.js";
import TagSelect from "./components/tagSelect";
import { useState } from "react";
import { createPortal } from "react-dom";

let maxTags = 7;

interface ManageMapProps {
  tagInfo: types.TagInfo[],
  mapData: types.MapContext,
}
function ManageMap({ tagInfo, mapData }: ManageMapProps) {
  let [showDelete, setShowDelete] = useState(false);
  let [selectedTags, setSelectedTags] = useState<types.TagInfo[]>(mapData.tags);
  let maySetTags = selectedTags.length > 0 && selectedTags.length <= maxTags;

  return <>
    <TagSelect
      tagInfo={tagInfo}
      selectedTags={selectedTags}
      setSelectedTags={setSelectedTags}
      originalSelectedTags={mapData.tags}
      maxTags={maxTags}
    />
    <p>
      <button disabled={!maySetTags}>Update tags</button>
    </p>
    <p>
      <button onClick={_ => setShowDelete(true)}>Delete map</button>
    </p>
    {showDelete && createPortal(<div className="deleteMapConfirmation">
      <div className="confirmMessage">Are you sure you want to delete this map?</div>
      <div className="confirmButtons">
        <button>Delete</button><button onClick={_ => setShowDelete(false)}>Cancel</button>
      </div>
    </div>, document.body)}
  </>;
}

let tagInfoNode = document.getElementById("tagData")!;
let tagInfo: types.TagInfo[] = JSON.parse(tagInfoNode.innerText);

let mapDataNode = document.getElementById("mapData")!;
let mapData: types.MapContext = JSON.parse(mapDataNode.innerText);

let mapManageNode = document.getElementById("mapManage")!;
let mapManageRoot = createRoot(mapManageNode);
mapManageRoot.render(<ManageMap tagInfo={tagInfo} mapData={mapData} />);
