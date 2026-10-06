import { type Component, Switch, Match } from "solid-js";

export type IconName =
  | "folder"
  | "folder-open"
  | "settings"
  | "search"
  | "close"
  | "warning"
  | "check"
  | "error"
  | "dirty"
  | "branch"
  | "commit"
  | "refresh"
  | "copy"
  | "palette"
  | "terminal"
  | "chevron-down"
  | "chevron-right"
  | "plus"
  | "trash"
  | "arrow-up"
  | "arrow-down"
  | "arrow-up-down"
  | "star"
  | "list"
  | "tag"
  | "package"
  | "archive"
  | "history"
  | "edit";

export interface IconProps {
  name: IconName;
  size?: number | string;
  class?: string;
  title?: string;
  "aria-hidden"?: boolean | "true" | "false";
}

export const Icon: Component<IconProps> = (props) => {
  const size = () => props.size ?? 16;
  const isAriaHidden = () => props["aria-hidden"] ?? (props.title ? undefined : true);

  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      width={size()}
      height={size()}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
      class={props.class}
      aria-hidden={isAriaHidden()}
      role={props.title ? "img" : undefined}
    >
      {props.title && <title>{props.title}</title>}
      <Switch>
        <Match when={props.name === "folder"}>
          <path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.93a2 2 0 0 1-1.66-.9l-.82-1.2A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13c0 1.1.9 2 2 2Z" />
        </Match>
        <Match when={props.name === "folder-open"}>
          <path d="m6 14 1.45-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.55 6a2 2 0 0 1-1.94 1.5H4a2 2 0 0 1-2-2V5c0-1.1.9-2 2-2h3.93a2 2 0 0 1 1.66.9l.82 1.2a2 2 0 0 0 1.66.9H18a2 2 0 0 1 2 2v2" />
        </Match>
        <Match when={props.name === "settings"}>
          <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" />
          <circle cx="12" cy="12" r="3" />
        </Match>
        <Match when={props.name === "search"}>
          <circle cx="11" cy="11" r="8" />
          <path d="m21 21-4.3-4.3" />
        </Match>
        <Match when={props.name === "close"}>
          <path d="M18 6 6 18" />
          <path d="m6 6 12 12" />
        </Match>
        <Match when={props.name === "warning"}>
          <path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z" />
          <line x1="12" y1="9" x2="12" y2="13" />
          <line x1="12" y1="17" x2="12.01" y2="17" />
        </Match>
        <Match when={props.name === "check"}>
          <polyline points="20 6 9 17 4 12" />
        </Match>
        <Match when={props.name === "error"}>
          <circle cx="12" cy="12" r="10" />
          <line x1="15" y1="9" x2="9" y2="15" />
          <line x1="9" y1="9" x2="15" y2="15" />
        </Match>
        <Match when={props.name === "dirty"}>
          <circle cx="12" cy="12" r="6" fill="currentColor" stroke="none" />
        </Match>
        <Match when={props.name === "branch"}>
          <line x1="6" y1="3" x2="6" y2="15" />
          <circle cx="18" cy="6" r="3" />
          <circle cx="6" cy="18" r="3" />
          <path d="M18 9a9 9 0 0 1-9 9" />
        </Match>
        <Match when={props.name === "commit"}>
          <circle cx="12" cy="12" r="4" />
          <line x1="1.05" y1="12" x2="7" y2="12" />
          <line x1="17.01" y1="12" x2="22.96" y2="12" />
        </Match>
        <Match when={props.name === "refresh"}>
          <path d="M3 12a9 9 0 0 1 15-6.7L21 8" />
          <path d="M21 3v5h-5" />
          <path d="M21 12a9 9 0 0 1-15 6.7L3 16" />
          <path d="M3 21v-5h5" />
        </Match>
        <Match when={props.name === "copy"}>
          <rect width="14" height="14" x="8" y="8" rx="2" ry="2" />
          <path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2" />
        </Match>
        <Match when={props.name === "palette"}>
          <circle cx="13.5" cy="6.5" r=".5" fill="currentColor" />
          <circle cx="17.5" cy="10.5" r=".5" fill="currentColor" />
          <circle cx="8.5" cy="7.5" r=".5" fill="currentColor" />
          <circle cx="6.5" cy="12.5" r=".5" fill="currentColor" />
          <path d="M12 2C6.5 2 2 6.5 2 12s4.5 10 10 10c.926 0 1.648-.746 1.648-1.688 0-.437-.18-.835-.437-1.125-.29-.289-.438-.652-.438-1.125a1.64 1.64 0 0 1 1.668-1.668h1.996c3.051 0 5.555-2.503 5.555-5.554C21.965 6.012 17.461 2 12 2z" />
        </Match>
        <Match when={props.name === "terminal"}>
          <polyline points="4 17 10 11 4 5" />
          <line x1="12" y1="19" x2="20" y2="19" />
        </Match>
        <Match when={props.name === "chevron-down"}>
          <polyline points="6 9 12 15 18 9" />
        </Match>
        <Match when={props.name === "chevron-right"}>
          <polyline points="9 18 15 12 9 6" />
        </Match>
        <Match when={props.name === "plus"}>
          <line x1="12" y1="5" x2="12" y2="19" />
          <line x1="5" y1="12" x2="19" y2="12" />
        </Match>
        <Match when={props.name === "trash"}>
          <path d="M3 6h18" />
          <path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6" />
          <path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" />
        </Match>
        <Match when={props.name === "arrow-up"}>
          <line x1="12" y1="19" x2="12" y2="5" />
          <polyline points="5 12 12 5 19 12" />
        </Match>
        <Match when={props.name === "arrow-down"}>
          <line x1="12" y1="5" x2="12" y2="19" />
          <polyline points="19 12 12 19 5 12" />
        </Match>
        <Match when={props.name === "arrow-up-down"}>
          <line x1="8" y1="3" x2="8" y2="17" />
          <polyline points="4 7 8 3 12 7" />
          <line x1="16" y1="21" x2="16" y2="7" />
          <polyline points="12 17 16 21 20 17" />
        </Match>
        <Match when={props.name === "star"}>
          <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
        </Match>
        <Match when={props.name === "list"}>
          <line x1="8" y1="6" x2="21" y2="6" />
          <line x1="8" y1="12" x2="21" y2="12" />
          <line x1="8" y1="18" x2="21" y2="18" />
          <line x1="3" y1="6" x2="3.01" y2="6" />
          <line x1="3" y1="12" x2="3.01" y2="12" />
          <line x1="3" y1="18" x2="3.01" y2="18" />
        </Match>
        <Match when={props.name === "tag"}>
          <path d="M12 2H2v10l9.29 9.29c.94.94 2.48.94 3.42 0l6.58-6.58c.94-.94.94-2.48 0-3.42L12 2Z" />
          <line x1="7" y1="7" x2="7.01" y2="7" />
        </Match>
        <Match when={props.name === "package"}>
          <path d="m16.5 9.4-9-5.19M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z" />
          <polyline points="3.27 6.96 12 12.01 20.73 6.96" />
          <line x1="12" y1="22.08" x2="12" y2="12" />
        </Match>
        <Match when={props.name === "archive"}>
          <polyline points="21 8 21 21 3 21 3 8" />
          <rect width="22" height="5" x="1" y="3" rx="1" />
          <line x1="10" y1="12" x2="14" y2="12" />
        </Match>
        <Match when={props.name === "history"}>
          <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
          <path d="M3 3v5h5" />
          <polyline points="12 7 12 12 15 15" />
        </Match>
        <Match when={props.name === "edit"}>
          <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
          <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
        </Match>
      </Switch>
    </svg>
  );
};
