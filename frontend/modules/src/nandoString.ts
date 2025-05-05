function createNandoString(el: HTMLElement) {
  let result: HTMLElement = document.createElement("span");
  result.classList.add(...el.classList);

  let tag = el.innerText;

  let bold = false;
  let italic = false;
  let wide = false;
  let narrow = false;
  let uppercase = false;
  let shadow = false;
  let color = undefined;

  let ptr = 0;
  while (ptr < tag.length) {
    let codepoint = tag.codePointAt(ptr) ?? 0;

    let part;
    if (
      0xE000 <= codepoint && codepoint <= 0xF8FF
      || 0xF0000 <= codepoint && codepoint <= 0xFFFFD
      || 0x100000 <= codepoint && codepoint <= 0x10FFFD
    ) {
      part = document.createElement("i");
      part.className = "fa-solid";
      part.innerText = tag.charAt(ptr);
    } else {
      part = document.createTextNode(tag.charAt(ptr));
    }

    if (tag.charAt(ptr) == "$") {
      ptr += 1;
      if (ptr >= tag.length) {
        break;
      }

      if (tag.charAt(ptr) == "$") {
        ptr += 1;
      } else if (tag.charAt(ptr) == "o") {
        bold = true;
        ptr += 1;
        continue;
      } else if (tag.charAt(ptr) == "i") {
        italic = true;
        ptr += 1;
        continue;
      } else if (tag.charAt(ptr) == "w") {
        wide = true;
        narrow = false;
        ptr += 1;
        continue;
      } else if (tag.charAt(ptr) == "n") {
        narrow = true;
        wide = false;
        ptr += 1;
        continue;
      } else if (tag.charAt(ptr) == "t") {
        uppercase = true;
        ptr += 1;
        continue;
      } else if (tag.charAt(ptr) == "s") {
        shadow = true;
        ptr += 1;
        continue;
      } else if (tag.charAt(ptr) == "L") {
        while (ptr < tag.length && tag.charAt(ptr) != ']') {
          ptr += 1;
        }
        ptr += 1;
        continue;
      } else if (tag.charAt(ptr) == "g") {
        color = undefined;
        ptr += 1;
        continue;
      } else if (tag.charAt(ptr) == "z") {
        bold = italic = wide = narrow = uppercase = shadow = false;
        ptr += 1;
        continue;
      } else {
        color = "";
        color += tag.charAt(ptr++);
        color += tag.charAt(ptr++);
        color += tag.charAt(ptr);
        ptr += 1;
        continue;
      }
    }

    if (bold) {
      let inner = document.createElement("b");
      inner.appendChild(part);
      part = inner;
    }
    if (italic) {
      let inner = document.createElement("i");
      inner.appendChild(part);
      part = inner;
    }
    if (wide) {
      let inner = document.createElement("span");
      inner.className = "textStretch";
      inner.appendChild(part);
      part = inner;
    }
    if (narrow) {
      let inner = document.createElement("span");
      inner.className = "textShrink";
      inner.appendChild(part);
      part = inner;
    }
    if (uppercase) {
      let inner = document.createElement("span");
      inner.className = "textUppercase";
      inner.appendChild(part);
      part = inner;
    }
    if (shadow) {
      let inner = document.createElement("span");
      inner.className = "textShadow";
      inner.appendChild(part);
      part = inner;
    }
    if (color != undefined) {
      let inner = document.createElement("span");
      inner.style = "color:#" + color;
      inner.appendChild(part);
      part = inner;
    }

    result.appendChild(part)
    ptr += 1;
  }

  return result;
}

window.addEventListener("load", (_) => {
  let nandoStrings = document.querySelectorAll(".nandoString");
  for (let el of nandoStrings) {
    if (el instanceof HTMLElement) {
      el.replaceWith(createNandoString(el));
    } else {
      console.log("Not an HTML element");
    }
  }
});
