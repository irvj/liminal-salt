import { writeFileSync, mkdirSync } from "fs";
import theme from "../theme.js";

function buildGhostty(mode) {
	const u = theme.ui[mode];
	const e = theme.editor[mode];
	const a = theme.ansi[mode];

	// Ghostty uses bare hex without the # prefix
	const h = (hex) => hex.slice(1);

	const lines = [
		`# Liminal Salt ${mode === "dark" ? "Dark" : "Light"} — Ghostty`,
		`# https://github.com/irvj/liminal-salt`,
		``,
		`background = ${h(u.background)}`,
		`foreground = ${h(u.foreground)}`,
		`cursor-color = ${h(e.cursor)}`,
		`cursor-text = ${h(e.cursorForeground)}`,
		`selection-background = ${h(e.selection)}`,
		`selection-foreground = ${h(u.foreground)}`,
		``,
		`# Normal colors`,
		`palette = 0=${h(a.black)}`,
		`palette = 1=${h(a.red)}`,
		`palette = 2=${h(a.green)}`,
		`palette = 3=${h(a.yellow)}`,
		`palette = 4=${h(a.blue)}`,
		`palette = 5=${h(a.magenta)}`,
		`palette = 6=${h(a.cyan)}`,
		`palette = 7=${h(a.white)}`,
		``,
		`# Bright colors`,
		`palette = 8=${h(a.brightBlack)}`,
		`palette = 9=${h(a.brightRed)}`,
		`palette = 10=${h(a.brightGreen)}`,
		`palette = 11=${h(a.brightYellow)}`,
		`palette = 12=${h(a.brightBlue)}`,
		`palette = 13=${h(a.brightMagenta)}`,
		`palette = 14=${h(a.brightCyan)}`,
		`palette = 15=${h(a.brightWhite)}`,
	];

	return lines.join("\n") + "\n";
}

export default function exportGhostty(outDir) {
	const dir = `${outDir}/ghostty`;
	mkdirSync(dir, { recursive: true });

	for (const mode of ["dark", "light"]) {
		const data = buildGhostty(mode);
		const filename = `liminal-salt-${mode}`;
		writeFileSync(`${dir}/${filename}`, data);
		console.log(`  ✓ ${dir}/${filename}`);
	}
}
