// Click a demo to play it. Nothing is fetched until then: a Bevy wasm
// bundle is megabytes, and most readers are here to read.
document.addEventListener("click", function (e) {
  const button = e.target.closest(".demo button");
  if (!button) return;
  const box = button.closest(".demo");
  const frame = document.createElement("iframe");
  // Each demo is a complete page trunk built, so the iframe gets the
  // canvas, the keyboard and the game loop without this page's help.
  frame.src = "demos/" + box.dataset.demo + "/index.html";
  frame.title = box.dataset.demo;
  box.replaceChildren(frame);
  frame.focus();
});
