import { Component } from "solid-js";

interface SearchBarProps {
  value: string;
  onInput: (value: string) => void;
}

const SearchBar: Component<SearchBarProps> = (props) => {
  return (
    <div class="search-bar">
      <svg class="search-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <circle cx="11" cy="11" r="8" />
        <path d="m21 21-4.35-4.35" />
      </svg>
      <input
        type="text"
        placeholder="搜索剪贴板历史..."
        value={props.value}
        onInput={(e) => props.onInput(e.currentTarget.value)}
        autofocus
      />
      {props.value && (
        <button class="clear-btn" onClick={() => props.onInput("")}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M18 6 6 18M6 6l12 12" />
          </svg>
        </button>
      )}
    </div>
  );
};

export default SearchBar;
