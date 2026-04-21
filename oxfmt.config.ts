import { defineConfig } from "oxfmt";

export default defineConfig({
  sortImports: {
    newlinesBetween: false,
    customGroups: [
      {
        groupName: "components",
        elementNamePattern: ["./components/**", "./components"],
      },
    ],
    groups: [
      ["value-builtin", "value-external"],
      { newlinesBetween: true },
      "components",
      { newlinesBetween: true },
      ["value-internal", "value-parent", "value-sibling", "value-index"],
      "type-import",
      "unknown",
    ],
  },
});
