import { Component, For } from "solid-js";
import { TagRule } from "../hooks/useClipboard";

interface TagBarProps {
  rules: TagRule[];
  activeTag: string;
  onSelectTag: (tag: string) => void;
}

const TagBar: Component<TagBarProps> = (props) => {
  return (
    <div class="tag-bar">
      <button
        class={`tag-chip ${props.activeTag === "" ? "active" : ""}`}
        onClick={() => props.onSelectTag("")}
      >
        全部
      </button>
      <For each={props.rules}>
        {(rule) => (
          <button
            class={`tag-chip ${props.activeTag === rule.name ? "active" : ""}`}
            onClick={() => props.onSelectTag(rule.name)}
            style={{ "--chip-color": rule.color }}
          >
            <span class="tag-dot" style={{ background: rule.color }}></span>
            {rule.name}
          </button>
        )}
      </For>
    </div>
  );
};

export default TagBar;
