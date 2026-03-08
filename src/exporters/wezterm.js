import { writeFileSync, mkdirSync } from "fs";
import theme from "../theme.js";

function buildWezTerm(mode) {
	const isDark = mode === "dark";
	const u = theme.ui[mode];
	const e = theme.editor[mode];
	const a = theme.ansi[mode];

	// WezTerm color scheme uses a TOML format
	return `# Liminal Salt ${isDark ? "Dark" : "Light"} — WezTerm
# https://github.com/irvj/liminal-salt
#
# Install:
#   Copy to ~/.config/wezterm/colors/
#   Then in wezterm.lua:
#     config.color_scheme = "Liminal Salt ${isDark ? "Dark" : "Light"}"

[metadata]
name = "Liminal Salt ${isDark ? "Dark" : "Light"}"
author = "irvj"
origin_url = "https://github.com/irvj/liminal-salt"

[colors]
background = "${u.background}"
foreground = "${u.foreground}"
cursor_bg = "${e.cursor}"
cursor_fg = "${e.cursorForeground}"
cursor_border = "${e.cursor}"
selection_bg = "${e.selection}"
selection_fg = "${u.foreground}"
scrollbar_thumb = "${u.border}"
split = "${u.border}"
compose_cursor = "${u.warning}"

ansi = [
    "${a.black}",
    "${a.red}",
    "${a.green}",
    "${a.yellow}",
    "${a.blue}",
    "${a.magenta}",
    "${a.cyan}",
    "${a.white}",
]

brights = [
    "${a.brightBlack}",
    "${a.brightRed}",
    "${a.brightGreen}",
    "${a.brightYellow}",
    "${a.brightBlue}",
    "${a.brightMagenta}",
    "${a.brightCyan}",
    "${a.brightWhite}",
]

[colors.tab_bar]
background = "${u.card}"

[colors.tab_bar.active_tab]
bg_color = "${u.background}"
fg_color = "${u.foreground}"
intensity = "Bold"

[colors.tab_bar.inactive_tab]
bg_color = "${u.card}"
fg_color = "${u.mutedForeground}"

[colors.tab_bar.inactive_tab_hover]
bg_color = "${u.muted}"
fg_color = "${u.foreground}"
italic = true

[colors.tab_bar.new_tab]
bg_color = "${u.card}"
fg_color = "${u.mutedForeground}"

[colors.tab_bar.new_tab_hover]
bg_color = "${u.accent}"
fg_color = "${u.accentForeground}"
`;
}

export default function exportWezTerm(outDir) {
	const dir = `${outDir}/wezterm`;
	mkdirSync(dir, { recursive: true });

	for (const mode of ["dark", "light"]) {
		const data = buildWezTerm(mode);
		const filename = `Liminal Salt ${isDark(mode) ? "Dark" : "Light"}.toml`;
		writeFileSync(`${dir}/${filename}`, data);
		console.log(`  ✓ ${dir}/${filename}`);
	}
}

function isDark(mode) {
	return mode === "dark";
}
