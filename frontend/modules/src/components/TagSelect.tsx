import { TagBadge } from "./TagBadge";
import * as types from "../bindings/index";
import Select, { MultiValueProps, OptionProps, components } from "react-select";

interface TagOption {
  value: types.TagInfo,
  label: string,
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
  let options = tagInfo.map(tag => ({value: tag, label: tag.name} as TagOption));
  if (selectedTags.length >= maxTags) {
    options = selectedTags.map(tag => ({value: tag, label: tag.name} as TagOption));
  }

  // selected needs to contain the same TagOption objects as in options
  let selected = [];
  for (let select of selectedTags) {
    selected.push(options.find(option => option.value.id == select.id)!);
  }

  return <>
    <div className="tagSelectContainer">
      <Select
        className="tagSelect"
        isMulti={true}
        isClearable={false}
        closeMenuOnSelect={false}
        hideSelectedOptions={false}
        options={options}
        value={selected}
        onChange={(newValue, _meta) => {
          setSelectedTags(newValue.map(value => value.value));
        }}
        classNames={{
          multiValue: (state) => ["selectMultiValue", state.data.value.name.split("/")[0]].join(" "),
        }}
        components={{
          MultiValue: (props: MultiValueProps<TagOption>) => <components.MultiValue {...props}>
            <TagBadge tag={props.data.value} />
          </components.MultiValue>,
          Option: (props: OptionProps<TagOption>) => <components.Option {...props}>
            <TagBadge tag={props.data.value} />
          </components.Option>,
        }}
        isDisabled={!maySelectTags}
      />
      <button onClick={_ => setSelectedTags(originalSelectedTags)}>
        {originalSelectedTags.length == 0 ? "Clear" : "Reset"}
      </button>
    </div>
  </>;
}

export default TagSelect
