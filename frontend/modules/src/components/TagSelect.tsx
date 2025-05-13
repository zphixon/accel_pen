import * as types from "../bindings/index";

interface TagBadgeProps {
  tag: types.TagInfo,
  checked: boolean,
  selected: boolean,
  onChange?: React.ChangeEventHandler<HTMLInputElement>,
}
function TagBadge({ tag, checked, selected, onChange }: TagBadgeProps) {
  let root = tag.name.split("/")[0];
  let rest = tag.name.split("/").slice(1).join("/");
  return <>
    <div className={["tagBadge", root].join(" ")}>
      <input
        hidden
        className={["tagCheckbox", selected ? "tagCheckboxSelected" : ""].join(" ")}
        type="checkbox"
        id={tag.name}
        onChange={onChange}
        checked={checked}
      />
      <label
        htmlFor={tag.name}
        className={["tagName", root].join(" ")}
      >
        <span className="rootName">{root}/</span>{rest}
      </label>
    </div>
  </>;
}

interface TagSelectProps {
  originalSelectedTags?: types.TagInfo[],
  tagInfo: types.TagInfo[],
  selectedTags: types.TagInfo[],
  setSelectedTags: (newSelectedTags: types.TagInfo[]) => void,
  maxTags: number,
  maySelectTags?: boolean,
}
function TagSelect({ tagInfo, selectedTags, setSelectedTags, originalSelectedTags = [], maySelectTags = true, maxTags }: TagSelectProps) {
  function toggleTag(event: React.ChangeEvent<HTMLInputElement>) {
    if (event.target.checked) {
      if (selectedTags.length >= maxTags || !maySelectTags) {
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

  let selectedTagList = selectedTags.map(tag => <TagBadge key={tag.id} tag={tag} selected={false} checked={true} onChange={toggleTag} />);
  let tagGrid = tagInfo.map(tag => <div key={tag.id} className="tagContainer">
    {selectedTags.find(selectedTag => selectedTag.name == tag.name)
      ? <TagBadge key={tag.id} tag={tag} selected={true} checked={false} onChange={toggleTag} />
      : <TagBadge key={tag.id} tag={tag} selected={false} checked={false} onChange={toggleTag} />}
  </div>);

  let gridClasses = ["tagList", "tagSelectGrid"];
  if (selectedTags.length >= maxTags || !maySelectTags) {
    gridClasses.push("fullSelection");
  }

  let buttonText = "Reset tags";
  if (originalSelectedTags.length == 0) {
    buttonText = "Clear tags";
  }

  return <div id="tagSelect">
    <div id="tagSelectHeader">
      <div id="selectedTagsContainer">
        Selected tags: <span className="tagList" id="selectedTagList">{selectedTagList}</span>
      </div>
      <div id="resetTagsContainer">
        <button id="resetTags" onClick={_ => setSelectedTags(originalSelectedTags)}>{buttonText}</button>
      </div>
    </div>
    <br/>
    <div className={gridClasses.join(" ")}>
      {tagGrid}
    </div>
  </div>;
}

export default TagSelect
