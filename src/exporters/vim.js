import { writeFileSync, mkdirSync } from "fs";
import theme from "../theme.js";

function buildVim(mode) {
	const isDark = mode === "dark";
	const u = theme.ui[mode];
	const s = theme.syntax[mode];
	const e = theme.editor[mode];
	const a = theme.ansi[mode];

	// Helper: hi guifg=X guibg=Y gui=Z
	function hi(group, fg, bg, style) {
		const parts = [`hi ${group}`];
		parts.push(`guifg=${fg || "NONE"}`);
		parts.push(`guibg=${bg || "NONE"}`);
		if (style) parts.push(`gui=${style}`);
		return parts.join(" ");
	}

	const lines = [
		`" Liminal Salt ${isDark ? "Dark" : "Light"}`,
		`" https://github.com/irvj/liminal-salt`,
		``,
		`set background=${mode}`,
		`hi clear`,
		`if exists("syntax_on")`,
		`  syntax reset`,
		`endif`,
		`let g:colors_name = "liminal-salt-${mode}"`,
		``,
		`" Editor`,
		hi("Normal", u.foreground, u.background),
		hi("CursorLine", null, e.lineHighlight, "NONE"),
		hi("CursorLineNr", e.gutterActiveForeground, e.lineHighlight),
		hi("LineNr", e.gutterForeground),
		hi("Visual", null, e.selection),
		hi("Search", null, e.findMatch),
		hi("IncSearch", null, e.findMatch, "bold"),
		hi("MatchParen", null, e.bracketMatch, "bold"),
		hi("NonText", e.whitespace),
		hi("SpecialKey", e.whitespace),
		hi("Cursor", e.cursorForeground, e.cursor),
		hi("SignColumn", e.gutterForeground, u.background),
		hi("FoldColumn", e.gutterForeground, u.background),
		hi("Folded", u.mutedForeground, u.muted),
		``,
		`" UI`,
		hi("StatusLine", u.foreground, u.card, "NONE"),
		hi("StatusLineNC", u.mutedForeground, u.card, "NONE"),
		hi("VertSplit", u.border, u.background, "NONE"),
		hi("WinSeparator", u.border, u.background, "NONE"),
		hi("TabLine", u.mutedForeground, u.card, "NONE"),
		hi("TabLineFill", null, u.card, "NONE"),
		hi("TabLineSel", u.foreground, u.background, "bold"),
		hi("Pmenu", u.foreground, u.card),
		hi("PmenuSel", u.foreground, e.selection),
		hi("PmenuSbar", null, u.muted),
		hi("PmenuThumb", null, u.border),
		hi("FloatBorder", u.border, u.card),
		hi("NormalFloat", u.foreground, u.card),
		``,
		`" Diagnostics`,
		hi("DiagnosticError", u.destructive),
		hi("DiagnosticWarn", u.warning),
		hi("DiagnosticInfo", u.accent),
		hi("DiagnosticHint", u.accent),
		hi("ErrorMsg", u.destructive, null, "bold"),
		hi("WarningMsg", u.warning),
		``,
		`" Diff`,
		hi("DiffAdd", null, e.diffInsertedBackground),
		hi("DiffDelete", null, e.diffDeletedBackground),
		hi("DiffChange", null, e.lineHighlight),
		hi("DiffText", null, e.findMatch, "bold"),
		``,
		`" Syntax`,
		hi("Comment", s.comment, null, "italic"),
		hi("Constant", s.constant),
		hi("String", s.string),
		hi("Number", s.number),
		hi("Float", s.number),
		hi("Boolean", s.constant),
		hi("Identifier", s.variable),
		hi("Function", s.function),
		hi("Statement", s.keyword),
		hi("Keyword", s.keyword),
		hi("Conditional", s.keyword),
		hi("Repeat", s.keyword),
		hi("Operator", s.operator),
		hi("PreProc", s.keyword),
		hi("Type", s.type),
		hi("StorageClass", s.keyword),
		hi("Structure", s.type),
		hi("Typedef", s.type),
		hi("Special", s.escape),
		hi("SpecialChar", s.escape),
		hi("Tag", s.tag),
		hi("Delimiter", s.punctuation),
		hi("Todo", u.warning, null, "bold"),
		hi("Underlined", u.link, null, "underline"),
		``,
		`" Treesitter`,
		`hi link @comment Comment`,
		`hi link @keyword Keyword`,
		`hi link @function Function`,
		`hi link @function.call Function`,
		`hi link @method Function`,
		`hi link @string String`,
		`hi link @number Number`,
		`hi link @float Float`,
		`hi link @boolean Boolean`,
		`hi link @type Type`,
		`hi link @type.builtin Type`,
		`hi link @variable Identifier`,
		`hi link @constant Constant`,
		`hi link @operator Operator`,
		`hi link @punctuation Delimiter`,
		`hi link @tag Tag`,
		hi("@tag.attribute", s.attribute),
		hi("@string.regex", s.regex),
		hi("@string.escape", s.escape),
		``,
		`" Git signs`,
		hi("GitSignsAdd", s.inserted),
		hi("GitSignsChange", s.string),
		hi("GitSignsDelete", s.deleted),
		``,
		`" Terminal colors`,
		`let g:terminal_ansi_colors = [`,
		`  \\ '${a.black}', '${a.red}', '${a.green}', '${a.yellow}',`,
		`  \\ '${a.blue}', '${a.magenta}', '${a.cyan}', '${a.white}',`,
		`  \\ '${a.brightBlack}', '${a.brightRed}', '${a.brightGreen}', '${a.brightYellow}',`,
		`  \\ '${a.brightBlue}', '${a.brightMagenta}', '${a.brightCyan}', '${a.brightWhite}'`,
		`  \\ ]`,
		``,
		`" Neovim terminal colors`,
		...[
			["0", a.black], ["1", a.red], ["2", a.green], ["3", a.yellow],
			["4", a.blue], ["5", a.magenta], ["6", a.cyan], ["7", a.white],
			["8", a.brightBlack], ["9", a.brightRed], ["10", a.brightGreen], ["11", a.brightYellow],
			["12", a.brightBlue], ["13", a.brightMagenta], ["14", a.brightCyan], ["15", a.brightWhite],
		].map(([n, c]) => `let g:terminal_color_${n} = '${c}'`),
	];

	return lines.join("\n") + "\n";
}

export default function exportVim(outDir) {
	const dir = `${outDir}/vim/colors`;
	mkdirSync(dir, { recursive: true });

	for (const mode of ["dark", "light"]) {
		const data = buildVim(mode);
		const filename = `liminal-salt-${mode}.vim`;
		writeFileSync(`${dir}/${filename}`, data);
		console.log(`  ✓ ${dir}/${filename}`);
	}
}
