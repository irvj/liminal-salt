// Liminal Salt — canonical theme definition
// Single source of truth for all color mappings.
// Semantic tokens reference primitive keys by name.

const primitives = {
	// Beige / warm neutrals
	beige50: "#fdfcfa",
	beige100: "#f5f2ed",
	beige200: "#ebe7e0",
	beige300: "#e8e4dc",
	beige400: "#ddd8d0",
	beige500: "#c5c1b8",
	beige600: "#9e9b93",
	beige700: "#888379",
	beige800: "#6b6762",
	beige900: "#5a5753",
	beige950: "#2d2b28",

	// Stone / cool neutrals
	stone50: "#2e312f",
	stone100: "#242726",
	stone200: "#1a1c1b",
	stone300: "#141615",

	// Sage / green
	sage300: "#a3bfac",
	sage400: "#8fac98",
	sage500: "#7dba8a",
	sage600: "#6b7369",
	sage700: "#506e58",
	sage800: "#425f4a",
	sage900: "#3a7346",

	// Teal
	teal400: "#95bebe",
	teal500: "#8fb8ad",
	teal700: "#3e5d5d",
	teal800: "#3e6b5d",

	// Red
	red300: "#d99292",
	red400: "#cc8585",
	red600: "#a54d4d",
	red700: "#984242",

	// Amber
	amber400: "#c9a86c",
	amber700: "#7d6325",

	// Blue
	blue400: "#7eb8c9",
	blue700: "#3a6f80",

	// Olive
	olive400: "#a9ad78",
	olive700: "#5c6135",

	// Orange
	orange400: "#c9956c",
	orange700: "#8a5c30",

	// Tinted backgrounds (dark)
	sageTint30: "#3d4741",
	beigeTint15: "#393a38",
	amberTint30: "#4f4633",
	redTint30: "#4f3c3b",
	greenTint30: "#384b3c",

	// Tinted backgrounds (light)
	sageTint30L: "#c4cac0",
	beigeTint15L: "#d7d4cf",
	amberTint30L: "#d1c7b1",
	redTint30L: "#ddc1bd",
	greenTint30L: "#bdccbb",
};

const ui = {
	dark: {
		background: "stone200",
		foreground: "beige300",
		foregroundSecondary: "beige500",
		muted: "stone300",
		mutedForeground: "beige600",
		card: "stone100",
		cardForeground: "beige300",
		accent: "sage400",
		accentHover: "sage300",
		accentForeground: "stone200",
		destructive: "red400",
		destructiveHover: "red300",
		destructiveForeground: "stone200",
		success: "sage500",
		successForeground: "stone200",
		warning: "amber400",
		warningForeground: "stone200",
		ring: "teal400",
		input: "sage600",
		border: "stone50",
		link: "sage400",
		linkActive: "sage300",
		linkVisited: "sage400",
	},
	light: {
		background: "beige100",
		foreground: "beige950",
		foregroundSecondary: "beige900",
		muted: "beige200",
		mutedForeground: "beige800",
		card: "beige50",
		cardForeground: "beige950",
		accent: "sage700",
		accentHover: "sage800",
		accentForeground: "beige100",
		destructive: "red600",
		destructiveHover: "red700",
		destructiveForeground: "beige100",
		success: "sage900",
		successForeground: "beige100",
		warning: "amber700",
		warningForeground: "beige100",
		ring: "teal700",
		input: "beige700",
		border: "beige400",
		link: "sage700",
		linkActive: "sage800",
		linkVisited: "sage700",
	},
};

const syntax = {
	dark: {
		comment: "beige600",
		punctuation: "beige500",
		keyword: "sage400",
		function: "sage300",
		string: "amber400",
		number: "blue400",
		type: "teal500",
		variable: "beige300",
		constant: "blue400",
		operator: "beige500",
		tag: "red400",
		attribute: "amber400",
		regex: "olive400",
		escape: "orange400",
		deleted: "red400",
		inserted: "sage500",
	},
	light: {
		comment: "beige800",
		punctuation: "beige900",
		keyword: "sage700",
		function: "sage800",
		string: "amber700",
		number: "blue700",
		type: "teal800",
		variable: "beige950",
		constant: "blue700",
		operator: "beige900",
		tag: "red600",
		attribute: "amber700",
		regex: "olive700",
		escape: "orange700",
		deleted: "red600",
		inserted: "sage900",
	},
};

const editor = {
	dark: {
		cursor: "sage400",
		cursorForeground: "stone200",
		selection: "sageTint30",
		lineHighlight: "beigeTint15",
		findMatch: "amberTint30",
		gutterForeground: "beige600",
		gutterActiveForeground: "beige500",
		bracketMatch: "stone50",
		indentGuide: "stone50",
		whitespace: "stone50",
		diffDeletedBackground: "redTint30",
		diffInsertedBackground: "greenTint30",
	},
	light: {
		cursor: "sage700",
		cursorForeground: "beige100",
		selection: "sageTint30L",
		lineHighlight: "beigeTint15L",
		findMatch: "amberTint30L",
		gutterForeground: "beige800",
		gutterActiveForeground: "beige900",
		bracketMatch: "beige400",
		indentGuide: "beige400",
		whitespace: "beige400",
		diffDeletedBackground: "redTint30L",
		diffInsertedBackground: "greenTint30L",
	},
};

const ansi = {
	dark: {
		black: "stone300",
		red: "red400",
		green: "sage500",
		yellow: "amber400",
		blue: "blue400",
		magenta: "orange400",
		cyan: "teal400",
		white: "beige500",
		brightBlack: "sage600",
		brightRed: "red300",
		brightGreen: "sage300",
		brightYellow: "amber400",
		brightBlue: "blue400",
		brightMagenta: "orange400",
		brightCyan: "teal400",
		brightWhite: "beige300",
	},
	light: {
		black: "beige950",
		red: "red600",
		green: "sage900",
		yellow: "amber700",
		blue: "blue700",
		magenta: "orange700",
		cyan: "teal700",
		white: "beige200",
		brightBlack: "beige800",
		brightRed: "red700",
		brightGreen: "sage800",
		brightYellow: "amber700",
		brightBlue: "blue700",
		brightMagenta: "orange700",
		brightCyan: "teal700",
		brightWhite: "beige100",
	},
};

// Resolve a primitive reference to its hex value
function resolve(key) {
	if (!primitives[key]) {
		throw new Error(`Unknown primitive: "${key}"`);
	}
	return primitives[key];
}

// Resolve all values in a token map
function resolveMap(map) {
	const resolved = {};
	for (const [k, v] of Object.entries(map)) {
		resolved[k] = resolve(v);
	}
	return resolved;
}

export default {
	name: "Liminal Salt",
	primitives,
	ui: { dark: resolveMap(ui.dark), light: resolveMap(ui.light) },
	syntax: { dark: resolveMap(syntax.dark), light: resolveMap(syntax.light) },
	editor: { dark: resolveMap(editor.dark), light: resolveMap(editor.light) },
	ansi: { dark: resolveMap(ansi.dark), light: resolveMap(ansi.light) },
	// Also export raw references for tooling that needs primitive names
	refs: { ui, syntax, editor, ansi },
};
