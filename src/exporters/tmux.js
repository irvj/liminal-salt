import { writeFileSync, mkdirSync } from "fs";
import theme from "../theme.js";

function buildTmux(mode) {
	const isDark = mode === "dark";
	const u = theme.ui[mode];
	const e = theme.editor[mode];
	const s = theme.syntax[mode];

	return `# Liminal Salt ${isDark ? "Dark" : "Light"} — tmux
# https://github.com/irvj/liminal-salt
#
# Usage: add to your tmux.conf:
#   source-file ~/.config/tmux/liminal-salt-${mode}.conf

# Default terminal colors
set -g default-terminal "tmux-256color"

# Pane borders
set -g pane-border-style "fg=${u.border}"
set -g pane-active-border-style "fg=${u.accent}"

# Status bar
set -g status-style "bg=${u.card},fg=${u.foreground}"
set -g status-left-style "bg=${u.accent},fg=${u.accentForeground},bold"
set -g status-right-style "bg=${u.card},fg=${u.mutedForeground}"
set -g status-left " #S "
set -g status-right " %Y-%m-%d %H:%M "
set -g status-left-length 20

# Window status
set -g window-status-style "bg=${u.card},fg=${u.mutedForeground}"
set -g window-status-current-style "bg=${u.background},fg=${u.foreground},bold"
set -g window-status-activity-style "bg=${u.card},fg=${u.warning}"
set -g window-status-bell-style "bg=${u.card},fg=${u.destructive}"
set -g window-status-format " #I:#W "
set -g window-status-current-format " #I:#W "
set -g window-status-separator ""

# Message / command prompt
set -g message-style "bg=${u.card},fg=${u.foreground}"
set -g message-command-style "bg=${u.card},fg=${u.foreground}"

# Copy mode
set -g mode-style "bg=${e.selection},fg=${u.foreground}"

# Clock
set -g clock-mode-colour "${u.accent}"

# Display pane numbers
set -g display-panes-active-colour "${u.accent}"
set -g display-panes-colour "${u.mutedForeground}"
`;
}

export default function exportTmux(outDir) {
	const dir = `${outDir}/tmux`;
	mkdirSync(dir, { recursive: true });

	for (const mode of ["dark", "light"]) {
		const data = buildTmux(mode);
		const filename = `liminal-salt-${mode}.conf`;
		writeFileSync(`${dir}/${filename}`, data);
		console.log(`  ✓ ${dir}/${filename}`);
	}
}
