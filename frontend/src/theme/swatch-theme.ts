export function getThemeSwatches(themeId: string | null): string[] {
  switch (themeId) {
    case "light":
      return ["#ffffff", "#2b6cb0", "#15733b", "#b3261e"];
    case "dark":
      return ["#1a1d21", "#4299e1", "#38a169", "#e53e3e"];
    case "catppuccin-mocha":
      return ["#1e1e2e", "#89b4fa", "#a6e3a1", "#f38ba8"];
    case "nord":
      return ["#2e3440", "#88c0d0", "#a3be8c", "#bf616a"];
    case "tokyo-night":
      return ["#1a1b26", "#7aa2f7", "#9ece6a", "#f7768e"];
    case "gruvbox-dark":
      return ["#282828", "#83a598", "#b8bb26", "#fb4934"];
    case "dracula":
      return ["#282a36", "#bd93f9", "#50fa7b", "#ff5555"];
    case "solarized-dark":
      return ["#002b36", "#268bd2", "#859900", "#dc322f"];
    default:
      return ["#1a1d21", "#3182ce", "#38a169", "#e53e3e"];
  }
}
