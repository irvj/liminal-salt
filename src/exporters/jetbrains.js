import { writeFileSync, mkdirSync } from "fs";
import theme from "../theme.js";

// Strip leading # from hex for JetBrains format
function h(hex) {
	return hex.slice(1).toUpperCase();
}

function attr(name, fg, bg, fontType, effectColor, effectType) {
	let inner = "";
	if (fg) inner += `\n\t\t\t\t<option name="FOREGROUND" value="${h(fg)}" />`;
	if (bg) inner += `\n\t\t\t\t<option name="BACKGROUND" value="${h(bg)}" />`;
	if (fontType) inner += `\n\t\t\t\t<option name="FONT_TYPE" value="${fontType}" />`;
	if (effectColor) inner += `\n\t\t\t\t<option name="EFFECT_COLOR" value="${h(effectColor)}" />`;
	if (effectType) inner += `\n\t\t\t\t<option name="EFFECT_TYPE" value="${effectType}" />`;
	return `\t\t<option name="${name}">
\t\t\t<value>${inner}
\t\t\t</value>
\t\t</option>`;
}

function buildJetBrains(mode) {
	const isDark = mode === "dark";
	const u = theme.ui[mode];
	const s = theme.syntax[mode];
	const e = theme.editor[mode];
	const a = theme.ansi[mode];
	const parent = isDark ? "Darcula" : "Default";
	const label = isDark ? "Dark" : "Light";

	return `<?xml version="1.0" encoding="UTF-8"?>
<scheme name="Liminal Salt ${label}" version="142" parent_scheme="${parent}">
\t<metaInfo>
\t\t<property name="created">Liminal Salt theme generator</property>
\t\t<property name="ide">Idea</property>
\t\t<property name="ideVersion">2024.1</property>
\t\t<property name="originalScheme">Liminal Salt ${label}</property>
\t</metaInfo>
\t<colors>
\t\t<option name="CARET_COLOR" value="${h(e.cursor)}" />
\t\t<option name="CARET_ROW_COLOR" value="${h(e.lineHighlight)}" />
\t\t<option name="CONSOLE_BACKGROUND_KEY" value="${h(u.background)}" />
\t\t<option name="GUTTER_BACKGROUND" value="${h(u.background)}" />
\t\t<option name="INDENT_GUIDE" value="${h(e.indentGuide)}" />
\t\t<option name="LINE_NUMBERS_COLOR" value="${h(e.gutterForeground)}" />
\t\t<option name="LINE_NUMBER_ON_CARET_ROW_COLOR" value="${h(e.gutterActiveForeground)}" />
\t\t<option name="MATCHED_BRACES_INDENT_GUIDE_COLOR" value="${h(e.bracketMatch)}" />
\t\t<option name="METHOD_SEPARATORS_COLOR" value="${h(u.border)}" />
\t\t<option name="RIGHT_MARGIN_COLOR" value="${h(e.indentGuide)}" />
\t\t<option name="SELECTED_INDENT_GUIDE" value="${h(e.gutterForeground)}" />
\t\t<option name="SELECTED_TEARLINE_COLOR" value="${h(u.accent)}" />
\t\t<option name="SELECTION_BACKGROUND" value="${h(e.selection)}" />
\t\t<option name="SELECTION_FOREGROUND" value="${h(u.foreground)}" />
\t\t<option name="SOFT_WRAP_SIGN_COLOR" value="${h(e.whitespace)}" />
\t\t<option name="TEARLINE_COLOR" value="${h(u.border)}" />
\t\t<option name="WHITESPACES" value="${h(e.whitespace)}" />
\t\t<option name="ADDED_LINES_COLOR" value="${h(s.inserted)}" />
\t\t<option name="MODIFIED_LINES_COLOR" value="${h(s.string)}" />
\t\t<option name="DELETED_LINES_COLOR" value="${h(s.deleted)}" />
\t\t<option name="WHITESPACES_MODIFIED_LINES_COLOR" value="${h(u.border)}" />
\t\t<option name="FILESTATUS_ADDED" value="${h(s.inserted)}" />
\t\t<option name="FILESTATUS_MODIFIED" value="${h(s.string)}" />
\t\t<option name="FILESTATUS_DELETED" value="${h(s.deleted)}" />
\t\t<option name="FILESTATUS_NOT_CHANGED_IMMEDIATE" value="${h(u.mutedForeground)}" />
\t\t<option name="NOTIFICATION_BACKGROUND" value="${h(u.card)}" />
\t\t<option name="INFORMATION_HINT" value="${h(u.card)}" />
\t\t<option name="DOCUMENTATION_COLOR" value="${h(u.card)}" />
\t\t<option name="LOOKUP_COLOR" value="${h(u.card)}" />
\t\t<option name="ScrollBar.Mac.thumbColor" value="${h(u.border)}88" />
\t\t<option name="ScrollBar.Mac.hoverThumbColor" value="${h(u.border)}cc" />
\t\t<option name="TAB_UNDERLINE" value="${h(u.accent)}" />
\t\t<option name="TAB_UNDERLINE_INACTIVE" value="${h(u.border)}" />
\t\t<!--  Terminal ANSI colors  -->
\t\t<option name="TERMINAL_COMMAND_TO_RUN_USING_IDE" value="${h(u.accent)}" />
\t</colors>
\t<attributes>
\t\t<!-- Text -->
${attr("TEXT", u.foreground, u.background)}
\t\t<!-- Syntax -->
${attr("DEFAULT_KEYWORD", s.keyword, null, "1")}
${attr("DEFAULT_FUNCTION_DECLARATION", s.function)}
${attr("DEFAULT_FUNCTION_CALL", s.function)}
${attr("DEFAULT_STRING", s.string)}
${attr("DEFAULT_VALID_STRING_ESCAPE", s.escape)}
${attr("DEFAULT_INVALID_STRING_ESCAPE", s.escape, null, null, s.deleted, "WAVE_UNDERSCORE")}
${attr("DEFAULT_NUMBER", s.number)}
${attr("DEFAULT_CONSTANT", s.constant)}
${attr("DEFAULT_CLASS_NAME", s.type)}
${attr("DEFAULT_INTERFACE_NAME", s.type)}
${attr("DEFAULT_INSTANCE_FIELD", s.variable)}
${attr("DEFAULT_STATIC_FIELD", s.constant)}
${attr("DEFAULT_GLOBAL_VARIABLE", s.variable)}
${attr("DEFAULT_LOCAL_VARIABLE", s.variable)}
${attr("DEFAULT_PARAMETER", s.variable)}
${attr("DEFAULT_IDENTIFIER", s.variable)}
${attr("DEFAULT_OPERATION_SIGN", s.operator)}
${attr("DEFAULT_DOT", s.punctuation)}
${attr("DEFAULT_SEMICOLON", s.punctuation)}
${attr("DEFAULT_COMMA", s.punctuation)}
${attr("DEFAULT_PARENTHS", s.punctuation)}
${attr("DEFAULT_BRACKETS", s.punctuation)}
${attr("DEFAULT_BRACES", s.punctuation)}
${attr("DEFAULT_LINE_COMMENT", s.comment, null, "2")}
${attr("DEFAULT_BLOCK_COMMENT", s.comment, null, "2")}
${attr("DEFAULT_DOC_COMMENT", s.comment, null, "2")}
${attr("DEFAULT_DOC_COMMENT_TAG", s.keyword, null, "3")}
${attr("DEFAULT_DOC_COMMENT_TAG_VALUE", s.variable, null, "2")}
${attr("DEFAULT_DOC_MARKUP", s.comment)}
${attr("DEFAULT_TAG", s.tag)}
${attr("DEFAULT_ATTRIBUTE", s.attribute)}
${attr("DEFAULT_METADATA", s.attribute)}
${attr("DEFAULT_LABEL", s.keyword)}
${attr("DEFAULT_TEMPLATE_LANGUAGE_COLOR", s.escape)}
${attr("DEFAULT_ENTITY", s.type)}
\t\t<!-- Markup -->
${attr("MARKDOWN_HEADER", s.keyword, null, "1")}
${attr("MARKDOWN_BOLD", null, null, "1")}
${attr("MARKDOWN_ITALIC", null, null, "2")}
${attr("MARKDOWN_CODE_SPAN", s.string)}
${attr("MARKDOWN_LINK_TEXT", s.type)}
${attr("MARKDOWN_LINK_DESTINATION", s.string)}
\t\t<!-- Search -->
${attr("SEARCH_RESULT_ATTRIBUTES", null, e.findMatch)}
${attr("WRITE_SEARCH_RESULT_ATTRIBUTES", null, e.findMatch)}
${attr("TEXT_SEARCH_RESULT_ATTRIBUTES", null, e.findMatch)}
\t\t<!-- Diff -->
${attr("DIFF_INSERTED", s.inserted, e.diffInsertedBackground)}
${attr("DIFF_DELETED", s.deleted, e.diffDeletedBackground)}
${attr("DIFF_MODIFIED", s.string, e.lineHighlight)}
\t\t<!-- Matched braces -->
${attr("MATCHED_BRACE_ATTRIBUTES", null, e.bracketMatch, "1")}
${attr("UNMATCHED_BRACE_ATTRIBUTES", null, e.bracketMatch, null, s.deleted, "WAVE_UNDERSCORE")}
\t\t<!-- Errors / warnings -->
${attr("ERRORS_ATTRIBUTES", null, null, null, u.destructive, "WAVE_UNDERSCORE")}
${attr("WARNING_ATTRIBUTES", null, null, null, u.warning, "WAVE_UNDERSCORE")}
${attr("INFO_ATTRIBUTES", null, null, null, u.accent, "WAVE_UNDERSCORE")}
${attr("DEPRECATED_ATTRIBUTES", null, null, null, u.mutedForeground, "STRIKEOUT")}
${attr("TODO_DEFAULT_ATTRIBUTES", u.warning, null, "2")}
\t\t<!-- Hyperlinks -->
${attr("HYPERLINK_ATTRIBUTES", u.link, null, null, u.link, "LINE_UNDERSCORE")}
${attr("FOLLOWED_HYPERLINK_ATTRIBUTES", u.linkVisited, null, null, u.linkVisited, "LINE_UNDERSCORE")}
\t\t<!-- Console / Terminal -->
${attr("CONSOLE_NORMAL_OUTPUT", u.foreground)}
${attr("CONSOLE_ERROR_OUTPUT", u.destructive)}
${attr("CONSOLE_USER_INPUT", s.string)}
${attr("LOG_ERROR_OUTPUT", u.destructive)}
${attr("LOG_WARNING_OUTPUT", u.warning)}
${attr("LOG_INFO_OUTPUT", u.foreground)}
${attr("LOG_DEBUG_OUTPUT", u.mutedForeground)}
${attr("CONSOLE_BLACK_OUTPUT", a.black)}
${attr("CONSOLE_RED_OUTPUT", a.red)}
${attr("CONSOLE_GREEN_OUTPUT", a.green)}
${attr("CONSOLE_YELLOW_OUTPUT", a.yellow)}
${attr("CONSOLE_BLUE_OUTPUT", a.blue)}
${attr("CONSOLE_MAGENTA_OUTPUT", a.magenta)}
${attr("CONSOLE_CYAN_OUTPUT", a.cyan)}
${attr("CONSOLE_WHITE_OUTPUT", a.white)}
${attr("CONSOLE_DARKGRAY_OUTPUT", a.brightBlack)}
${attr("CONSOLE_RED_BRIGHT_OUTPUT", a.brightRed)}
${attr("CONSOLE_GREEN_BRIGHT_OUTPUT", a.brightGreen)}
${attr("CONSOLE_YELLOW_BRIGHT_OUTPUT", a.brightYellow)}
${attr("CONSOLE_BLUE_BRIGHT_OUTPUT", a.brightBlue)}
${attr("CONSOLE_MAGENTA_BRIGHT_OUTPUT", a.brightMagenta)}
${attr("CONSOLE_CYAN_BRIGHT_OUTPUT", a.brightCyan)}
${attr("CONSOLE_GRAY_OUTPUT", a.brightWhite)}
\t</attributes>
</scheme>
`;
}

export default function exportJetBrains(outDir) {
	const dir = `${outDir}/jetbrains`;
	mkdirSync(dir, { recursive: true });

	for (const mode of ["dark", "light"]) {
		const data = buildJetBrains(mode);
		const filename = `Liminal Salt ${mode === "dark" ? "Dark" : "Light"}.icls`;
		writeFileSync(`${dir}/${filename}`, data);
		console.log(`  ✓ ${dir}/${filename}`);
	}
}
