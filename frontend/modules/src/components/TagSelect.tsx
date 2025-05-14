import Select, { MultiValueProps, OptionProps, components } from "react-select";
import * as types from "../bindings/index";

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
  let selected = options.filter(tag => selectedTags.find(selectedTag => tag.value.id == selectedTag.id));

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
          //option: (state) => state.isSelected ? "" : ["tagName", state.data.value.name.split("/")[0]].join(" "),
        }}
        components={{
          MultiValue: (props: MultiValueProps<TagOption>) => <components.MultiValue {...props}>
            <span className={["tagName", props.data.value.name.split("/")[0]].join(" ")}>
              <span className="rootName">{props.data.value.name.split("/")[0]}</span>/{props.data.value.name.split("/").slice(1)}
            </span>
          </components.MultiValue>,
          Option: (props: OptionProps<TagOption>) => <>
            <components.Option {...props}>
              <span className={["tagName", props.data.value.name.split("/")[0]].join(" ")}>
                <span className="rootName">{props.data.value.name.split("/")[0]}</span>/{props.data.value.name.split("/").slice(1)}
              </span>
            </components.Option>
          </>
        }}
      />
      <button onClick={_ => setSelectedTags(originalSelectedTags)}>
        {originalSelectedTags.length == 0 ? "Clear" : "Reset"}
      </button>
    </div>
  </>;
}

export default TagSelect
