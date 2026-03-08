import { writeFileSync, mkdirSync } from "fs";
import theme from "../theme.js";

// Convert hex color to iTerm2 plist color component (0.0–1.0)
function hexToComponents(hex) {
	const r = parseInt(hex.slice(1, 3), 16) / 255;
	const g = parseInt(hex.slice(3, 5), 16) / 255;
	const b = parseInt(hex.slice(5, 7), 16) / 255;
	return { r, g, b };
}

function colorEntry(name, hex) {
	const { r, g, b } = hexToComponents(hex);
	return `\t<key>${name}</key>
\t<dict>
\t\t<key>Alpha Component</key>
\t\t<real>1</real>
\t\t<key>Blue Component</key>
\t\t<real>${b.toFixed(8)}</real>
\t\t<key>Color Space</key>
\t\t<string>sRGB</string>
\t\t<key>Green Component</key>
\t\t<real>${g.toFixed(8)}</real>
\t\t<key>Red Component</key>
\t\t<real>${r.toFixed(8)}</real>
\t</dict>`;
}

function buildITerm2(mode) {
	const u = theme.ui[mode];
	const e = theme.editor[mode];
	const a = theme.ansi[mode];

	const ansiMap = [
		["Ansi 0 Color", a.black],
		["Ansi 1 Color", a.red],
		["Ansi 2 Color", a.green],
		["Ansi 3 Color", a.yellow],
		["Ansi 4 Color", a.blue],
		["Ansi 5 Color", a.magenta],
		["Ansi 6 Color", a.cyan],
		["Ansi 7 Color", a.white],
		["Ansi 8 Color", a.brightBlack],
		["Ansi 9 Color", a.brightRed],
		["Ansi 10 Color", a.brightGreen],
		["Ansi 11 Color", a.brightYellow],
		["Ansi 12 Color", a.brightBlue],
		["Ansi 13 Color", a.brightMagenta],
		["Ansi 14 Color", a.brightCyan],
		["Ansi 15 Color", a.brightWhite],
		["Background Color", u.background],
		["Foreground Color", u.foreground],
		["Bold Color", u.foreground],
		["Cursor Color", e.cursor],
		["Cursor Text Color", e.cursorForeground],
		["Selected Text Color", u.foreground],
		["Selection Color", e.selection],
		["Badge Color", u.accent],
		["Link Color", u.link],
	];

	const entries = ansiMap.map(([name, hex]) => colorEntry(name, hex)).join("\n");

	return `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
${entries}
</dict>
</plist>
`;
}

export default function exportITerm2(outDir) {
	const dir = `${outDir}/iterm2`;
	mkdirSync(dir, { recursive: true });

	for (const mode of ["dark", "light"]) {
		const data = buildITerm2(mode);
		const filename = `Liminal Salt ${mode === "dark" ? "Dark" : "Light"}.itermcolors`;
		writeFileSync(`${dir}/${filename}`, data);
		console.log(`  ✓ ${dir}/${filename}`);
	}
}
