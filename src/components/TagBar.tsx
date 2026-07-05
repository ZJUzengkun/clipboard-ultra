import { Component, For } from "solid-js";
import { FilterTag } from "../hooks/useClipboard";

interface TagBarProps {
  tags: FilterTag[];
  activeTag: string;
  onSelectTag: (value: string) => void;
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
      <For each={props.tags}>
        {(tag) => (
          <button
            class={`tag-chip ${props.activeTag === tag.value ? "active" : ""}`}
            onClick={() => props.onSelectTag(tag.value)}
            style={{ "--chip-color": tag.color }}
          >
            <span class="tag-dot" style={{ background: tag.color }}></span>
            {tag.name}
          </button>
        )}
      </For>
    </div>
  );
};

export default TagBar;
