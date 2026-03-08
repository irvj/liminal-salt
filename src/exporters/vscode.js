import { writeFileSync, mkdirSync } from "fs";
import theme from "../theme.js";

function buildVSCodeTheme(mode) {
	const isDark = mode === "dark";
	const u = theme.ui[mode];
	const s = theme.syntax[mode];
	const e = theme.editor[mode];
	const a = theme.ansi[mode];

	return {
		name: `Liminal Salt ${isDark ? "Dark" : "Light"}`,
		type: isDark ? "dark" : "light",
		colors: {
			// Editor
			"editor.background": u.background,
			"editor.foreground": u.foreground,
			"editorCursor.foreground": e.cursor,
			"editor.selectionBackground": e.selection,
			"editor.lineHighlightBackground": e.lineHighlight,
			"editor.findMatchBackground": e.findMatch,
			"editor.findMatchHighlightBackground": e.findMatch + "88",
			"editorLineNumber.foreground": e.gutterForeground,
			"editorLineNumber.activeForeground": e.gutterActiveForeground,
			"editorBracketMatch.background": e.bracketMatch + "44",
			"editorBracketMatch.border": e.bracketMatch,
			"editorIndentGuide.background": e.indentGuide,
			"editorIndentGuide.activeBackground": e.gutterForeground,
			"editorWhitespace.foreground": e.whitespace,
			"diffEditor.insertedTextBackground": e.diffInsertedBackground + "88",
			"diffEditor.removedTextBackground": e.diffDeletedBackground + "88",

			// Sidebar / activity bar
			"sideBar.background": u.card,
			"sideBar.foreground": u.foreground,
			"sideBar.border": u.border,
			"sideBarTitle.foreground": u.foreground,
			"sideBarSectionHeader.background": u.muted,
			"sideBarSectionHeader.foreground": u.foreground,
			"activityBar.background": u.card,
			"activityBar.foreground": u.foreground,
			"activityBar.border": u.border,
			"activityBarBadge.background": u.accent,
			"activityBarBadge.foreground": u.accentForeground,

			// Title bar
			"titleBar.activeBackground": u.card,
			"titleBar.activeForeground": u.foreground,
			"titleBar.inactiveBackground": u.card,
			"titleBar.inactiveForeground": u.mutedForeground,
			"titleBar.border": u.border,

			// Status bar
			"statusBar.background": u.card,
			"statusBar.foreground": u.foreground,
			"statusBar.border": u.border,
			"statusBar.debuggingBackground": u.warning,
			"statusBar.debuggingForeground": u.warningForeground,
			"statusBar.noFolderBackground": u.muted,

			// Tabs
			"tab.activeBackground": u.background,
			"tab.activeForeground": u.foreground,
			"tab.inactiveBackground": u.card,
			"tab.inactiveForeground": u.mutedForeground,
			"tab.border": u.border,
			"editorGroupHeader.tabsBackground": u.card,
			"editorGroupHeader.tabsBorder": u.border,

			// Lists (file explorer, quick open, etc.)
			"list.activeSelectionBackground": u.accent + "33",
			"list.activeSelectionForeground": u.foreground,
			"list.hoverBackground": u.border + "88",
			"list.focusBackground": u.accent + "22",
			"list.inactiveSelectionBackground": u.muted,

			// Input / dropdown
			"input.background": u.muted,
			"input.foreground": u.foreground,
			"input.border": u.border,
			"input.placeholderForeground": u.mutedForeground,
			"dropdown.background": u.card,
			"dropdown.border": u.border,
			"dropdown.foreground": u.foreground,

			// Buttons
			"button.background": u.accent,
			"button.foreground": u.accentForeground,
			"button.hoverBackground": u.accentHover,

			// Scrollbar
			"scrollbarSlider.background": u.border + "88",
			"scrollbarSlider.hoverBackground": u.border + "cc",
			"scrollbarSlider.activeBackground": u.border,

			// Badges
			"badge.background": u.accent,
			"badge.foreground": u.accentForeground,

			// Notifications
			"notifications.background": u.card,
			"notifications.foreground": u.foreground,
			"notifications.border": u.border,

			// Panel (terminal, output, etc.)
			"panel.background": u.card,
			"panel.foreground": u.foreground,
			"panel.border": u.border,
			"panelTitle.activeBorder": u.accent,
			"panelTitle.activeForeground": u.foreground,
			"panelTitle.inactiveForeground": u.mutedForeground,

			// Terminal colors
			"terminal.background": u.background,
			"terminal.foreground": u.foreground,
			"terminal.ansiBlack": a.black,
			"terminal.ansiRed": a.red,
			"terminal.ansiGreen": a.green,
			"terminal.ansiYellow": a.yellow,
			"terminal.ansiBlue": a.blue,
			"terminal.ansiMagenta": a.magenta,
			"terminal.ansiCyan": a.cyan,
			"terminal.ansiWhite": a.white,
			"terminal.ansiBrightBlack": a.brightBlack,
			"terminal.ansiBrightRed": a.brightRed,
			"terminal.ansiBrightGreen": a.brightGreen,
			"terminal.ansiBrightYellow": a.brightYellow,
			"terminal.ansiBrightBlue": a.brightBlue,
			"terminal.ansiBrightMagenta": a.brightMagenta,
			"terminal.ansiBrightCyan": a.brightCyan,
			"terminal.ansiBrightWhite": a.brightWhite,

			// Peek view
			"peekView.border": u.accent,
			"peekViewEditor.background": u.muted,
			"peekViewResult.background": u.card,
			"peekViewTitle.background": u.card,

			// Git decoration
			"gitDecoration.modifiedResourceForeground": s.string,
			"gitDecoration.deletedResourceForeground": s.deleted,
			"gitDecoration.untrackedResourceForeground": s.inserted,
			"gitDecoration.conflictingResourceForeground": s.escape,

			// Breadcrumb
			"breadcrumb.foreground": u.mutedForeground,
			"breadcrumb.focusForeground": u.foreground,
			"breadcrumb.activeSelectionForeground": u.foreground,

			// Widget
			"editorWidget.background": u.card,
			"editorWidget.foreground": u.foreground,
			"editorWidget.border": u.border,

			// Focus border
			focusBorder: u.ring,

			// Selection highlight
			"editor.selectionHighlightBackground": e.selection + "88",
			"editor.wordHighlightBackground": e.selection + "66",
			"editor.wordHighlightStrongBackground": e.selection + "99",

			// Minimap
			"minimap.selectionHighlight": e.selection,
			"minimap.findMatchHighlight": e.findMatch,

			// Error / warning
			"editorError.foreground": u.destructive,
			"editorWarning.foreground": u.warning,
			"editorInfo.foreground": u.accent,

			// Overview ruler
			"editorOverviewRuler.errorForeground": u.destructive,
			"editorOverviewRuler.warningForeground": u.warning,
			"editorOverviewRuler.infoForeground": u.accent,
		},
		tokenColors: [
			{
				name: "Comment",
				scope: ["comment", "punctuation.definition.comment"],
				settings: { foreground: s.comment, fontStyle: "italic" },
			},
			{
				name: "Keyword",
				scope: [
					"keyword",
					"storage.type",
					"storage.modifier",
					"keyword.control",
				],
				settings: { foreground: s.keyword },
			},
			{
				name: "Function",
				scope: [
					"entity.name.function",
					"support.function",
					"meta.function-call",
				],
				settings: { foreground: s.function },
			},
			{
				name: "String",
				scope: ["string", "string.quoted"],
				settings: { foreground: s.string },
			},
			{
				name: "Number",
				scope: ["constant.numeric"],
				settings: { foreground: s.number },
			},
			{
				name: "Type",
				scope: [
					"entity.name.type",
					"support.type",
					"entity.name.class",
					"support.class",
				],
				settings: { foreground: s.type },
			},
			{
				name: "Variable",
				scope: ["variable", "variable.other"],
				settings: { foreground: s.variable },
			},
			{
				name: "Constant",
				scope: [
					"constant",
					"constant.language",
					"variable.other.constant",
				],
				settings: { foreground: s.constant },
			},
			{
				name: "Operator",
				scope: ["keyword.operator"],
				settings: { foreground: s.operator },
			},
			{
				name: "Punctuation",
				scope: [
					"punctuation",
					"meta.brace",
					"punctuation.definition.tag",
				],
				settings: { foreground: s.punctuation },
			},
			{
				name: "Tag",
				scope: ["entity.name.tag", "support.tag"],
				settings: { foreground: s.tag },
			},
			{
				name: "Attribute",
				scope: ["entity.other.attribute-name"],
				settings: { foreground: s.attribute },
			},
			{
				name: "Regex",
				scope: ["string.regexp"],
				settings: { foreground: s.regex },
			},
			{
				name: "Escape",
				scope: ["constant.character.escape"],
				settings: { foreground: s.escape },
			},
			{
				name: "Inserted",
				scope: ["markup.inserted"],
				settings: { foreground: s.inserted },
			},
			{
				name: "Deleted",
				scope: ["markup.deleted"],
				settings: { foreground: s.deleted },
			},
			{
				name: "Markup Heading",
				scope: ["markup.heading", "entity.name.section"],
				settings: { foreground: s.keyword, fontStyle: "bold" },
			},
			{
				name: "Markup Bold",
				scope: ["markup.bold"],
				settings: { fontStyle: "bold" },
			},
			{
				name: "Markup Italic",
				scope: ["markup.italic"],
				settings: { fontStyle: "italic" },
			},
			{
				name: "Markup Link",
				scope: ["markup.underline.link", "string.other.link"],
				settings: { foreground: s.type },
			},
		],
	};
}

export default function exportVSCode(outDir) {
	const dir = `${outDir}/vscode`;
	mkdirSync(dir, { recursive: true });

	for (const mode of ["dark", "light"]) {
		const data = buildVSCodeTheme(mode);
		const filename = `liminal-salt-${mode}.json`;
		writeFileSync(`${dir}/${filename}`, JSON.stringify(data, null, "\t") + "\n");
		console.log(`  ✓ ${dir}/${filename}`);
	}
}
