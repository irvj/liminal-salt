import { writeFileSync, mkdirSync } from "fs";
import theme from "../theme.js";

function buildStyle(mode) {
	const isDark = mode === "dark";
	const u = theme.ui[mode];
	const s = theme.syntax[mode];
	const e = theme.editor[mode];
	const a = theme.ansi[mode];

	return {
		// General UI
		background: u.muted,
		"background.appearance": isDark ? "opaque" : "opaque",
		border: u.border,
		"border.variant": u.border,
		"border.focused": u.ring,
		"border.selected": u.accent,
		"border.transparent": u.border + "00",
		"border.disabled": u.border + "88",
		"elevated_surface.background": u.card,
		"surface.background": u.muted,
		foreground: u.foreground,
		"text.muted": u.mutedForeground,
		"text.disabled": u.mutedForeground + "88",
		"text.placeholder": u.mutedForeground,
		"text.accent": u.accent,
		accent: u.accent,
		"icon.accent": u.accent,

		// Title bar
		"title_bar.background": u.card,
		"title_bar.inactive_background": u.card,

		// Tab bar
		"tab_bar.background": u.card,
		"tab.active_background": u.background,
		"tab.inactive_background": u.card,

		// Toolbar
		"toolbar.background": u.background,

		// Status bar
		"status_bar.background": u.card,

		// Panel / sidebar
		"panel.background": u.muted,
		"panel.focused_border": u.ring,
		"pane.focused_border": u.ring,

		// Scrollbar
		"scrollbar.track.background": u.background + "00",
		"scrollbar.thumb.background": u.border + "88",
		"scrollbar.thumb.hover_background": u.border + "cc",
		"scrollbar.thumb.border": u.border + "00",

		// Editor
		"editor.background": u.background,
		"editor.foreground": u.foreground,
		"editor.gutter.background": u.background,
		"editor.line_number": e.gutterForeground,
		"editor.active_line_number": e.gutterActiveForeground,
		"editor.active_line.background": e.lineHighlight,
		"editor.highlighted_line.background": e.lineHighlight,
		"editor.invisible": e.whitespace,
		"editor.wrap_guide": e.indentGuide,
		"editor.active_wrap_guide": e.gutterForeground,
		"editor.indent_guide": e.indentGuide,
		"editor.indent_guide_active": e.gutterForeground,
		"editor.document_highlight.read_background": e.selection + "88",
		"editor.document_highlight.write_background": e.selection + "cc",
		"editor.subheader.background": u.muted,

		// Search / find match
		"search.match_background": e.findMatch,

		// Diff
		"created": s.inserted,
		"created.background": e.diffInsertedBackground,
		"modified": s.string,
		"modified.background": e.lineHighlight,
		"deleted": s.deleted,
		"deleted.background": e.diffDeletedBackground,
		"conflict": s.escape,
		"conflict.background": e.findMatch,

		// Diagnostics
		"error": u.destructive,
		"error.background": u.destructive + "22",
		"error.border": u.destructive,
		"warning": u.warning,
		"warning.background": u.warning + "22",
		"warning.border": u.warning,
		"info": u.accent,
		"info.background": u.accent + "22",
		"info.border": u.accent,
		"success": u.success,
		"success.background": u.success + "22",
		"success.border": u.success,
		"hint": u.accent,
		"hint.background": u.accent + "11",
		"hint.border": u.accent,
		"predictive": u.mutedForeground,

		// Ghost element (hover states, etc.)
		"ghost_element.background": u.background + "00",
		"ghost_element.hover": u.border + "88",
		"ghost_element.active": u.border + "cc",
		"ghost_element.selected": u.accent + "33",
		"ghost_element.disabled": u.border + "44",

		// Element
		"element.background": u.muted,
		"element.hover": u.border + "88",
		"element.active": u.border,
		"element.selected": u.accent + "33",
		"element.disabled": u.muted + "88",

		// Drop target
		"drop_target.background": u.accent + "22",

		// Players (collaboration cursors — use accent/theme colors)
		"players": [
			{ cursor: e.cursor, background: e.cursor, selection: e.selection },
			{ cursor: a.blue, background: a.blue, selection: a.blue + "33" },
			{ cursor: a.magenta, background: a.magenta, selection: a.magenta + "33" },
			{ cursor: a.cyan, background: a.cyan, selection: a.cyan + "33" },
			{ cursor: a.yellow, background: a.yellow, selection: a.yellow + "33" },
			{ cursor: a.red, background: a.red, selection: a.red + "33" },
			{ cursor: a.green, background: a.green, selection: a.green + "33" },
			{ cursor: a.brightBlue, background: a.brightBlue, selection: a.brightBlue + "33" },
		],

		// Syntax
		syntax: {
			comment: { color: s.comment, font_style: "italic" },
			"comment.doc": { color: s.comment, font_style: "italic" },
			keyword: { color: s.keyword },
			function: { color: s.function },
			string: { color: s.string },
			"string.escape": { color: s.escape },
			"string.regex": { color: s.regex },
			"string.special": { color: s.escape },
			number: { color: s.number },
			type: { color: s.type },
			variable: { color: s.variable },
			"variable.special": { color: s.constant },
			constant: { color: s.constant },
			boolean: { color: s.constant },
			operator: { color: s.operator },
			punctuation: { color: s.punctuation },
			"punctuation.bracket": { color: s.punctuation },
			"punctuation.delimiter": { color: s.punctuation },
			"punctuation.special": { color: s.punctuation },
			"punctuation.list_marker": { color: s.punctuation },
			tag: { color: s.tag },
			attribute: { color: s.attribute },
			property: { color: s.variable },
			label: { color: s.keyword },
			constructor: { color: s.type },
			enum: { color: s.type },
			variant: { color: s.constant },
			preproc: { color: s.keyword },
			link_text: { color: s.type },
			link_uri: { color: s.string },
			emphasis: { font_style: "italic" },
			"emphasis.strong": { font_weight: 700 },
			title: { color: s.keyword, font_weight: 700 },
			primary: { color: u.foreground },
			predictive: { color: u.mutedForeground, font_style: "italic" },
			hint: { color: u.mutedForeground, font_style: "italic" },
		},

		// Terminal
		"terminal.background": u.background,
		"terminal.foreground": u.foreground,
		"terminal.bright_foreground": isDark ? a.brightWhite : a.black,
		"terminal.dim_foreground": u.mutedForeground,
		"terminal.ansi.black": a.black,
		"terminal.ansi.red": a.red,
		"terminal.ansi.green": a.green,
		"terminal.ansi.yellow": a.yellow,
		"terminal.ansi.blue": a.blue,
		"terminal.ansi.magenta": a.magenta,
		"terminal.ansi.cyan": a.cyan,
		"terminal.ansi.white": a.white,
		"terminal.ansi.bright_black": a.brightBlack,
		"terminal.ansi.bright_red": a.brightRed,
		"terminal.ansi.bright_green": a.brightGreen,
		"terminal.ansi.bright_yellow": a.brightYellow,
		"terminal.ansi.bright_blue": a.brightBlue,
		"terminal.ansi.bright_magenta": a.brightMagenta,
		"terminal.ansi.bright_cyan": a.brightCyan,
		"terminal.ansi.bright_white": a.brightWhite,
		"terminal.ansi.dim_black": a.black + "88",
		"terminal.ansi.dim_red": a.red + "88",
		"terminal.ansi.dim_green": a.green + "88",
		"terminal.ansi.dim_yellow": a.yellow + "88",
		"terminal.ansi.dim_blue": a.blue + "88",
		"terminal.ansi.dim_magenta": a.magenta + "88",
		"terminal.ansi.dim_cyan": a.cyan + "88",
		"terminal.ansi.dim_white": a.white + "88",

		// Link
		"link_text.hover": u.link,
	};
}

export default function exportZed(outDir) {
	const dir = `${outDir}/zed`;
	mkdirSync(dir, { recursive: true });

	const themeFile = {
		$schema: "https://zed.dev/schema/themes/v0.2.0.json",
		name: "Liminal Salt",
		author: "irvj",
		themes: [
			{
				name: "Liminal Salt Dark",
				appearance: "dark",
				style: buildStyle("dark"),
			},
			{
				name: "Liminal Salt Light",
				appearance: "light",
				style: buildStyle("light"),
			},
		],
	};

	const filename = "liminal-salt.json";
	writeFileSync(`${dir}/${filename}`, JSON.stringify(themeFile, null, "\t") + "\n");
	console.log(`  ✓ ${dir}/${filename}`);
}
