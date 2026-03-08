import { writeFileSync, mkdirSync } from "fs";
import theme from "../theme.js";

function q(hex) {
	return `"${hex}"`;
}

function buildAlacritty(mode) {
	const u = theme.ui[mode];
	const e = theme.editor[mode];
	const a = theme.ansi[mode];

	return `# Liminal Salt ${mode === "dark" ? "Dark" : "Light"} — Alacritty
# https://github.com/irvj/liminal-salt

[colors.primary]
background = ${q(u.background)}
foreground = ${q(u.foreground)}

[colors.cursor]
cursor = ${q(e.cursor)}
text = ${q(e.cursorForeground)}

[colors.selection]
background = ${q(e.selection)}
text = ${q(u.foreground)}

[colors.normal]
black   = ${q(a.black)}
red     = ${q(a.red)}
green   = ${q(a.green)}
yellow  = ${q(a.yellow)}
blue    = ${q(a.blue)}
magenta = ${q(a.magenta)}
cyan    = ${q(a.cyan)}
white   = ${q(a.white)}

[colors.bright]
black   = ${q(a.brightBlack)}
red     = ${q(a.brightRed)}
green   = ${q(a.brightGreen)}
yellow  = ${q(a.brightYellow)}
blue    = ${q(a.brightBlue)}
magenta = ${q(a.brightMagenta)}
cyan    = ${q(a.brightCyan)}
white   = ${q(a.brightWhite)}
`;
}

export default function exportAlacritty(outDir) {
	const dir = `${outDir}/alacritty`;
	mkdirSync(dir, { recursive: true });

	for (const mode of ["dark", "light"]) {
		const data = buildAlacritty(mode);
		const filename = `liminal-salt-${mode}.toml`;
		writeFileSync(`${dir}/${filename}`, data);
		console.log(`  ✓ ${dir}/${filename}`);
	}
}
