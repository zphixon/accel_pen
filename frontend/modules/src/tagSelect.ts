export let selectedTagList = document.getElementById("selectedTagList")!;
export let tagCheckboxes: NodeListOf<HTMLInputElement> = document.querySelectorAll(".tagCheckbox");
export let maxSelected = 7;

let selected = 0;
for (let checkbox of tagCheckboxes) {
  checkbox.checked = false;
  let label = checkbox.parentElement!;
  let container = label.parentElement!;
  checkbox.onchange = ev => {
    ev.preventDefault();

    if (checkbox.checked) {
      selected += 1;
    } else {
      selected = Math.max(0, selected - 1);
    }

    if (selected > maxSelected) {
      selected -= 1;
      container.appendChild(label);
      checkbox.checked = false;
    } else if (checkbox.checked) {
      selectedTagList.appendChild(label);
    } else {
      container.appendChild(label);
    }

    let tagSelectGrid: HTMLElement = document.querySelector(".tagSelectGrid")!;
    if (selected == maxSelected) {
      tagSelectGrid.classList.add("fullSelection");
    } else {
      tagSelectGrid.classList.remove("fullSelection");
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
