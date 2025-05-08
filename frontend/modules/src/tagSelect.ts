export let selectedTagList = document.getElementById("selectedTagList")!;
export let tagCheckboxes: NodeListOf<HTMLInputElement> = document.querySelectorAll(".tagCheckbox");

for (let checkbox of tagCheckboxes) {
  checkbox.checked = false;
  let label = checkbox.parentElement!;
  let container = label.parentElement!;
  checkbox.onchange = _ => {
    if (checkbox.checked) {
      selectedTagList.appendChild(label);
    } else {
      container.appendChild(label);
    }
  };
}

document.getElementById("resetTags")!.onclick = _ => {
  for (let checkbox of tagCheckboxes) {
    checkbox.checked = false;
    let event = new Event("change");
    checkbox.dispatchEvent(event);
  }
};

