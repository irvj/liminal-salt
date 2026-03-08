import exportVSCode from "./exporters/vscode.js";
import exportITerm2 from "./exporters/iterm2.js";
import exportAlacritty from "./exporters/alacritty.js";
import exportGhostty from "./exporters/ghostty.js";
import exportVim from "./exporters/vim.js";
import exportZed from "./exporters/zed.js";
import exportTmux from "./exporters/tmux.js";
import exportWezTerm from "./exporters/wezterm.js";
import exportJetBrains from "./exporters/jetbrains.js";

const OUT_DIR = "dist";

const exporters = {
	vscode: exportVSCode,
	iterm2: exportITerm2,
	alacritty: exportAlacritty,
	ghostty: exportGhostty,
	vim: exportVim,
	zed: exportZed,
	tmux: exportTmux,
	wezterm: exportWezTerm,
	jetbrains: exportJetBrains,
};

const requested = process.argv[2];

if (requested) {
	if (!exporters[requested]) {
		console.error(`Unknown exporter: ${requested}`);
		console.error(`Available: ${Object.keys(exporters).join(", ")}`);
		process.exit(1);
	}
	console.log(`Building ${requested}...`);
	exporters[requested](OUT_DIR);
} else {
	console.log("Building all themes...\n");
	for (const [name, fn] of Object.entries(exporters)) {
		console.log(`${name}:`);
		fn(OUT_DIR);
	}
	console.log("\nDone!");
}
