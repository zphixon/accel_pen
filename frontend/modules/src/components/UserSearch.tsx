import * as types from "../bindings/index";
import * as api from "../api";
import AsyncSelect from "react-select/async";

interface UserSearchProps {
  selection?: types.UserResponse,
  setSelection?: (newSelection: types.UserResponse) => void,
}
export function UserSearch({ selection, setSelection }: UserSearchProps) {
  interface UserOption {
    value: types.UserResponse,
    label: string,
  }

  async function loadOptions(input: string): Promise<UserOption[]> {
    let result = await api.userSearchByName(input);
    if (Array.isArray(result)) {
      return result.map(user => ({ value: user, label: user.display_name }));
    } else {
      return [];
    }
  }

  return <>
    <AsyncSelect
      className="userSearchSelect"
      placeholder="Search user"
      cacheOptions
      loadOptions={loadOptions}
      isClearable
      value={selection ? { value: selection, label: selection.display_name } : undefined}
      onChange={(newValue, _) => { if (setSelection && newValue) setSelection(newValue.value) }}
    />
  </>;
}
