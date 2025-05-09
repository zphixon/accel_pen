import React, { useState } from "react";
import * as types from "../bindings/index";

interface TagBadgeProps {
  tag: types.TagInfo,
  checked: boolean,
  onChange?: React.ChangeEventHandler<HTMLInputElement>,
}
function TagBadge({ tag, checked, onChange }: TagBadgeProps) {
  return <>
    <div className={[tag.kind, tag.name].join(" ")}>
      <input
        hidden
        className="tagCheckbox"
        type="checkbox"
        id={tag.name}
        onChange={onChange}
        checked={checked}
      />
      <label
        htmlFor={tag.name}
        className={["tagName", tag.kind].join(" ")}
      >
        {tag.name}
      </label>
    </div>
  </>;
}

interface TagSelectProps {
  tagInfo: types.TagInfo[],
  selectedTags: types.TagInfo[],
  setSelectedTags: (newSelectedTags: types.TagInfo[]) => void,
  maxTags: number,
}
function TagSelect({ tagInfo, selectedTags, setSelectedTags, maxTags }: TagSelectProps) {
  function toggleTag(event: React.ChangeEvent<HTMLInputElement>) {
    if (event.target.checked) {
      if (selectedTags.length >= maxTags) {
        event.target.checked = false;
        return;
      }

      let newSelectedTags = structuredClone(selectedTags);
      newSelectedTags.push(tagInfo.find(tag => tag.name == event.target.id)!);
      setSelectedTags(newSelectedTags);
    } else {
      let newSelectedTags = structuredClone(selectedTags);
      let rm = newSelectedTags.findIndex(tag => tag.name == event.target.id)!;
      newSelectedTags.splice(rm, 1);
      setSelectedTags(newSelectedTags);
    }
  }

  let selectedTagList = selectedTags.map(tag => <TagBadge key={tag.id} tag={tag} checked={true} onChange={toggleTag} />);
  let tagGrid = tagInfo.map(tag => <div key={tag.id} className="tagContainer">
    {selectedTags.find(selectedTag => selectedTag.name == tag.name)
      ? '' : <TagBadge key={tag.id} tag={tag} checked={false} onChange={toggleTag} />}
  </div>);

  let gridClasses = ["tagList", "tagSelectGrid"];
  if (selectedTags.length >= maxTags) {
    gridClasses.push("fullSelection");
  }

  return <div id="tagSelect">
    <div id="tagSelectHeader">
      <div id="selectedTagsContainer">
        Selected tags: <span className="tagList" id="selectedTagList">{selectedTagList}</span>
      </div>
      <div id="resetTagsContainer">
        <button id="resetTags" onClick={_ => setSelectedTags([])}>Reset tags</button>
      </div>
    </div>
    <hr />
    <div className={gridClasses.join(" ")}>
      {tagGrid}
    </div>
  </div>;
}

export default TagSelect
