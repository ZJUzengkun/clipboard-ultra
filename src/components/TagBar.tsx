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
      <For each={props.tags}>
        {(tag) => (
          <button
            class={`tag-chip ${props.activeTag === tag.value ? "active" : ""}`}
            onClick={() =>
              // 再点一次已选标签 = 取消筛选，回到当前板视图
              props.onSelectTag(props.activeTag === tag.value ? "" : tag.value)
            }
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
