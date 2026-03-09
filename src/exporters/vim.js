import { writeFileSync, mkdirSync } from "fs";
import theme from "../theme.js";

// Resolve a semantic token (e.g. ui.dark.background → "stone200") to the
// primitive name string. We use theme.refs which has the un-resolved names.
const refs = theme.refs;

// Resolve a primitive name to its hex value for inline palette subsets (lualine)
const prims = theme.primitives;

function write(path, content) {
	writeFileSync(path, content);
	console.log(`  ✓ ${path}`);
}

// ---------------------------------------------------------------------------
// palette.lua — all raw primitives, mode-independent
// ---------------------------------------------------------------------------
function buildPalette() {
	const groups = [
		["Beige / warm neutrals", /^beige/],
		["Stone / cool neutrals", /^stone/],
		["Sage / green", /^sage/],
		["Teal", /^teal/],
		["Red", /^red/],
		["Amber", /^amber/],
		["Blue", /^blue/],
		["Olive", /^olive/],
		["Orange", /^orange/],
		["Tinted backgrounds (dark)", /Tint\d+$/],
		["Tinted backgrounds (light)", /Tint\d+L$/],
	];

	let entries = "";
	const seen = new Set();

	for (const [label, re] of groups) {
		entries += `  -- ${label}\n`;
		for (const [name, hex] of Object.entries(prims)) {
			if (re.test(name) && !seen.has(name)) {
				const pad = " ".repeat(Math.max(0, 14 - name.length));
				entries += `  ${name}${pad} = "${hex}",\n`;
				seen.add(name);
			}
		}
		entries += "\n";
	}

	return `-- Liminal Salt — canonical palette
-- Source: github.com/irvj/liminal-salt/src/theme.js

local M = {}

M.primitives = {
${entries.trimEnd()}
}

-- Convenience alias
M.p = M.primitives

return M
`;
}

// ---------------------------------------------------------------------------
// init.lua — orchestrator
// ---------------------------------------------------------------------------
function buildInit() {
	return `-- Liminal Salt — Neovim colorscheme
-- https://github.com/irvj/liminal-salt

local M = {}

function M.load(mode)
  mode = mode or "dark"

  if vim.g.colors_name then
    vim.cmd("hi clear")
  end
  if vim.fn.exists("syntax_on") then
    vim.cmd("syntax reset")
  end

  vim.o.termguicolors = true
  vim.o.background = mode
  vim.g.colors_name = "liminal-salt-" .. mode

  -- Collect all highlight groups
  local groups = {}
  local modules = {
    require("liminal-salt.editor"),
    require("liminal-salt.syntax"),
    require("liminal-salt.treesitter"),
    require("liminal-salt.lsp"),
    require("liminal-salt.plugins.gitsigns"),
    require("liminal-salt.plugins.snacks"),
  }

  for _, mod in ipairs(modules) do
    for name, hl in pairs(mod.highlights(mode)) do
      groups[name] = hl
    end
  end

  -- Apply all highlights
  for name, hl in pairs(groups) do
    vim.api.nvim_set_hl(0, name, hl)
  end

  -- Apply terminal colors
  require("liminal-salt.terminal").apply(mode)
end

return M
`;
}

// ---------------------------------------------------------------------------
// editor.lua
// ---------------------------------------------------------------------------
function buildEditor() {
	const d = refs.ui.dark;
	const dl = refs.ui.light;
	const ed = refs.editor.dark;
	const el = refs.editor.light;

	return `-- Liminal Salt — editor UI highlights
local p = require("liminal-salt.palette").p

local M = {}

local dark = {
  -- Editor
  Normal       = { fg = p.${d.foreground}, bg = p.${d.background} },
  NormalNC     = { fg = p.${d.foreground}, bg = p.${d.background} },
  CursorLine   = { bg = p.${ed.lineHighlight} },
  CursorLineNr = { fg = p.${ed.gutterActiveForeground}, bg = p.${ed.lineHighlight} },
  LineNr       = { fg = p.${ed.gutterForeground} },
  Visual       = { bg = p.${ed.selection} },
  Search       = { bg = p.${ed.findMatch} },
  IncSearch    = { bg = p.${ed.findMatch}, bold = true },
  CurSearch    = { bg = p.${ed.findMatch}, bold = true },
  MatchParen   = { bg = p.${ed.bracketMatch}, bold = true },
  NonText      = { fg = p.${ed.whitespace} },
  SpecialKey   = { fg = p.${ed.whitespace} },
  Cursor       = { fg = p.${ed.cursorForeground}, bg = p.${ed.cursor} },
  lCursor      = { fg = p.${ed.cursorForeground}, bg = p.${ed.cursor} },
  CursorIM     = { fg = p.${ed.cursorForeground}, bg = p.${ed.cursor} },
  SignColumn   = { fg = p.${ed.gutterForeground}, bg = p.${d.background} },
  FoldColumn   = { fg = p.${ed.gutterForeground}, bg = p.${d.background} },
  Folded       = { fg = p.${d.mutedForeground}, bg = p.${d.muted} },
  ColorColumn  = { bg = p.${d.muted} },
  Conceal      = { fg = p.${d.mutedForeground} },
  Directory    = { fg = p.${d.accent} },
  EndOfBuffer  = { fg = p.${d.background} },
  WildMenu     = { fg = p.${d.foreground}, bg = p.${ed.selection} },
  QuickFixLine = { bg = p.${ed.selection} },
  Substitute   = { bg = p.${ed.findMatch} },

  -- UI chrome
  StatusLine   = { fg = p.${d.foreground}, bg = p.${d.card} },
  StatusLineNC = { fg = p.${d.mutedForeground}, bg = p.${d.card} },
  VertSplit    = { fg = p.${d.border}, bg = p.${d.background} },
  WinSeparator = { fg = p.${d.border}, bg = p.${d.background} },
  TabLine      = { fg = p.${d.mutedForeground}, bg = p.${d.card} },
  TabLineFill  = { bg = p.${d.card} },
  TabLineSel   = { fg = p.${d.foreground}, bg = p.${d.background}, bold = true },
  Title        = { fg = p.${d.accent}, bold = true },

  -- Popup menu
  Pmenu        = { fg = p.${d.foreground}, bg = p.${d.card} },
  PmenuSel     = { fg = p.${d.foreground}, bg = p.${ed.selection} },
  PmenuSbar    = { bg = p.${d.muted} },
  PmenuThumb   = { bg = p.${d.border} },

  -- Floating windows
  NormalFloat  = { fg = p.${d.foreground}, bg = p.${d.card} },
  FloatBorder  = { fg = p.${d.border}, bg = p.${d.card} },
  FloatTitle   = { fg = p.${d.accent}, bg = p.${d.card}, bold = true },

  -- Messages
  ErrorMsg   = { fg = p.${d.destructive}, bold = true },
  WarningMsg = { fg = p.${d.warning} },
  MoreMsg    = { fg = p.${d.accent} },
  ModeMsg    = { fg = p.${d.foreground}, bold = true },
  Question   = { fg = p.${d.accent} },

  -- Diff
  DiffAdd    = { bg = p.${ed.diffInsertedBackground} },
  DiffDelete = { bg = p.${ed.diffDeletedBackground} },
  DiffChange = { bg = p.${ed.lineHighlight} },
  DiffText   = { bg = p.${ed.findMatch}, bold = true },

  -- Spell
  SpellBad   = { undercurl = true, sp = p.${d.destructive} },
  SpellCap   = { undercurl = true, sp = p.${d.warning} },
  SpellLocal = { undercurl = true, sp = p.teal400 },
  SpellRare  = { undercurl = true, sp = p.blue400 },
}

local light = {
  -- Editor
  Normal       = { fg = p.${dl.foreground}, bg = p.${dl.background} },
  NormalNC     = { fg = p.${dl.foreground}, bg = p.${dl.background} },
  CursorLine   = { bg = p.${el.lineHighlight} },
  CursorLineNr = { fg = p.${el.gutterActiveForeground}, bg = p.${el.lineHighlight} },
  LineNr       = { fg = p.${el.gutterForeground} },
  Visual       = { bg = p.${el.selection} },
  Search       = { bg = p.${el.findMatch} },
  IncSearch    = { bg = p.${el.findMatch}, bold = true },
  CurSearch    = { bg = p.${el.findMatch}, bold = true },
  MatchParen   = { bg = p.${el.bracketMatch}, bold = true },
  NonText      = { fg = p.${el.whitespace} },
  SpecialKey   = { fg = p.${el.whitespace} },
  Cursor       = { fg = p.${el.cursorForeground}, bg = p.${el.cursor} },
  lCursor      = { fg = p.${el.cursorForeground}, bg = p.${el.cursor} },
  CursorIM     = { fg = p.${el.cursorForeground}, bg = p.${el.cursor} },
  SignColumn   = { fg = p.${el.gutterForeground}, bg = p.${dl.background} },
  FoldColumn   = { fg = p.${el.gutterForeground}, bg = p.${dl.background} },
  Folded       = { fg = p.${dl.mutedForeground}, bg = p.${dl.muted} },
  ColorColumn  = { bg = p.${dl.muted} },
  Conceal      = { fg = p.${dl.mutedForeground} },
  Directory    = { fg = p.${dl.accent} },
  EndOfBuffer  = { fg = p.${dl.background} },
  WildMenu     = { fg = p.${dl.foreground}, bg = p.${el.selection} },
  QuickFixLine = { bg = p.${el.selection} },
  Substitute   = { bg = p.${el.findMatch} },

  -- UI chrome
  StatusLine   = { fg = p.${dl.foreground}, bg = p.${dl.card} },
  StatusLineNC = { fg = p.${dl.mutedForeground}, bg = p.${dl.card} },
  VertSplit    = { fg = p.${dl.border}, bg = p.${dl.background} },
  WinSeparator = { fg = p.${dl.border}, bg = p.${dl.background} },
  TabLine      = { fg = p.${dl.mutedForeground}, bg = p.${dl.card} },
  TabLineFill  = { bg = p.${dl.card} },
  TabLineSel   = { fg = p.${dl.foreground}, bg = p.${dl.background}, bold = true },
  Title        = { fg = p.${dl.accent}, bold = true },

  -- Popup menu
  Pmenu        = { fg = p.${dl.foreground}, bg = p.${dl.card} },
  PmenuSel     = { fg = p.${dl.foreground}, bg = p.${el.selection} },
  PmenuSbar    = { bg = p.${dl.muted} },
  PmenuThumb   = { bg = p.${dl.border} },

  -- Floating windows
  NormalFloat  = { fg = p.${dl.foreground}, bg = p.${dl.card} },
  FloatBorder  = { fg = p.${dl.border}, bg = p.${dl.card} },
  FloatTitle   = { fg = p.${dl.accent}, bg = p.${dl.card}, bold = true },

  -- Messages
  ErrorMsg   = { fg = p.${dl.destructive}, bold = true },
  WarningMsg = { fg = p.${dl.warning} },
  MoreMsg    = { fg = p.${dl.accent} },
  ModeMsg    = { fg = p.${dl.foreground}, bold = true },
  Question   = { fg = p.${dl.accent} },

  -- Diff
  DiffAdd    = { bg = p.${el.diffInsertedBackground} },
  DiffDelete = { bg = p.${el.diffDeletedBackground} },
  DiffChange = { bg = p.${el.lineHighlight} },
  DiffText   = { bg = p.${el.findMatch}, bold = true },

  -- Spell
  SpellBad   = { undercurl = true, sp = p.${dl.destructive} },
  SpellCap   = { undercurl = true, sp = p.${dl.warning} },
  SpellLocal = { undercurl = true, sp = p.teal700 },
  SpellRare  = { undercurl = true, sp = p.blue700 },
}

function M.highlights(mode)
  return mode == "light" and light or dark
end

return M
`;
}

// ---------------------------------------------------------------------------
// syntax.lua
// ---------------------------------------------------------------------------
function buildSyntax() {
	const d = refs.syntax.dark;
	const dl = refs.syntax.light;
	const ud = refs.ui.dark;
	const ul = refs.ui.light;

	return `-- Liminal Salt — syntax highlights
local p = require("liminal-salt.palette").p

local M = {}

local dark = {
  Comment    = { fg = p.${d.comment}, italic = true },
  Constant   = { fg = p.${d.constant} },
  String     = { fg = p.${d.string} },
  Character  = { fg = p.${d.string} },
  Number     = { fg = p.${d.number} },
  Float      = { fg = p.${d.number} },
  Boolean    = { fg = p.${d.constant} },
  Identifier = { fg = p.${d.variable} },
  Function   = { fg = p.${d.function} },
  Statement  = { fg = p.${d.keyword} },
  Keyword    = { fg = p.${d.keyword} },
  Conditional = { fg = p.${d.keyword} },
  Repeat     = { fg = p.${d.keyword} },
  Label      = { fg = p.${d.keyword} },
  Exception  = { fg = p.${d.keyword} },
  Operator   = { fg = p.${d.operator} },
  PreProc    = { fg = p.${d.keyword} },
  Include    = { fg = p.${d.keyword} },
  Define     = { fg = p.${d.keyword} },
  Macro      = { fg = p.${d.keyword} },
  Type       = { fg = p.${d.type} },
  StorageClass = { fg = p.${d.keyword} },
  Structure  = { fg = p.${d.type} },
  Typedef    = { fg = p.${d.type} },
  Special    = { fg = p.${d.escape} },
  SpecialChar = { fg = p.${d.escape} },
  Tag        = { fg = p.${d.tag} },
  Delimiter  = { fg = p.${d.punctuation} },
  Debug      = { fg = p.${d.tag} },
  Todo       = { fg = p.${ud.warning}, bold = true },
  Underlined = { fg = p.${ud.link}, underline = true },
  Error      = { fg = p.${d.deleted} },
}

local light = {
  Comment    = { fg = p.${dl.comment}, italic = true },
  Constant   = { fg = p.${dl.constant} },
  String     = { fg = p.${dl.string} },
  Character  = { fg = p.${dl.string} },
  Number     = { fg = p.${dl.number} },
  Float      = { fg = p.${dl.number} },
  Boolean    = { fg = p.${dl.constant} },
  Identifier = { fg = p.${dl.variable} },
  Function   = { fg = p.${dl.function} },
  Statement  = { fg = p.${dl.keyword} },
  Keyword    = { fg = p.${dl.keyword} },
  Conditional = { fg = p.${dl.keyword} },
  Repeat     = { fg = p.${dl.keyword} },
  Label      = { fg = p.${dl.keyword} },
  Exception  = { fg = p.${dl.keyword} },
  Operator   = { fg = p.${dl.operator} },
  PreProc    = { fg = p.${dl.keyword} },
  Include    = { fg = p.${dl.keyword} },
  Define     = { fg = p.${dl.keyword} },
  Macro      = { fg = p.${dl.keyword} },
  Type       = { fg = p.${dl.type} },
  StorageClass = { fg = p.${dl.keyword} },
  Structure  = { fg = p.${dl.type} },
  Typedef    = { fg = p.${dl.type} },
  Special    = { fg = p.${dl.escape} },
  SpecialChar = { fg = p.${dl.escape} },
  Tag        = { fg = p.${dl.tag} },
  Delimiter  = { fg = p.${dl.punctuation} },
  Debug      = { fg = p.${dl.tag} },
  Todo       = { fg = p.${ul.warning}, bold = true },
  Underlined = { fg = p.${ul.link}, underline = true },
  Error      = { fg = p.${dl.deleted} },
}

function M.highlights(mode)
  return mode == "light" and light or dark
end

return M
`;
}

// ---------------------------------------------------------------------------
// treesitter.lua
// ---------------------------------------------------------------------------
function buildTreesitter() {
	const d = refs.syntax.dark;
	const dl = refs.syntax.light;

	return `-- Liminal Salt — treesitter highlights
local p = require("liminal-salt.palette").p

local M = {}

local dark = {
  -- Misc
  ["@comment"]               = { link = "Comment" },
  ["@error"]                 = { fg = p.${d.deleted} },
  ["@preproc"]               = { fg = p.${d.keyword} },

  -- Punctuation
  ["@punctuation.delimiter"] = { fg = p.${d.punctuation} },
  ["@punctuation.bracket"]   = { fg = p.${d.punctuation} },
  ["@punctuation.special"]   = { fg = p.${d.punctuation} },

  -- Literals
  ["@string"]                = { fg = p.${d.string} },
  ["@string.regex"]          = { fg = p.${d.regex} },
  ["@string.escape"]         = { fg = p.${d.escape} },
  ["@string.special"]        = { fg = p.${d.escape} },
  ["@character"]             = { fg = p.${d.string} },
  ["@boolean"]               = { fg = p.${d.constant} },
  ["@number"]                = { fg = p.${d.number} },
  ["@float"]                 = { fg = p.${d.number} },

  -- Functions
  ["@function"]              = { fg = p.${d.function} },
  ["@function.call"]         = { fg = p.${d.function} },
  ["@function.builtin"]      = { fg = p.${d.function} },
  ["@function.macro"]        = { fg = p.${d.keyword} },
  ["@method"]                = { fg = p.${d.function} },
  ["@method.call"]           = { fg = p.${d.function} },
  ["@constructor"]           = { fg = p.${d.type} },

  -- Keywords
  ["@keyword"]               = { fg = p.${d.keyword} },
  ["@keyword.function"]      = { fg = p.${d.keyword} },
  ["@keyword.operator"]      = { fg = p.${d.keyword} },
  ["@keyword.return"]        = { fg = p.${d.keyword} },
  ["@conditional"]           = { fg = p.${d.keyword} },
  ["@repeat"]                = { fg = p.${d.keyword} },
  ["@label"]                 = { fg = p.${d.keyword} },
  ["@include"]               = { fg = p.${d.keyword} },
  ["@exception"]             = { fg = p.${d.keyword} },

  -- Types
  ["@type"]                  = { fg = p.${d.type} },
  ["@type.builtin"]          = { fg = p.${d.type} },
  ["@type.qualifier"]        = { fg = p.${d.keyword} },
  ["@type.definition"]       = { fg = p.${d.type} },
  ["@storageclass"]          = { fg = p.${d.keyword} },
  ["@attribute"]             = { fg = p.${d.attribute} },
  ["@field"]                 = { fg = p.${d.variable} },
  ["@property"]              = { fg = p.${d.variable} },

  -- Identifiers
  ["@variable"]              = { fg = p.${d.variable} },
  ["@variable.builtin"]      = { fg = p.${d.constant} },
  ["@constant"]              = { fg = p.${d.constant} },
  ["@constant.builtin"]      = { fg = p.${d.constant} },
  ["@constant.macro"]        = { fg = p.${d.constant} },
  ["@namespace"]             = { fg = p.${d.type} },
  ["@module"]                = { fg = p.${d.type} },
  ["@symbol"]                = { fg = p.${d.keyword} },

  -- Text / markup
  ["@text"]                  = { fg = p.${d.variable} },
  ["@text.strong"]           = { bold = true },
  ["@text.emphasis"]         = { italic = true },
  ["@text.underline"]        = { underline = true },
  ["@text.strike"]           = { strikethrough = true },
  ["@text.title"]            = { fg = p.${d.keyword}, bold = true },
  ["@text.literal"]          = { fg = p.${d.string} },
  ["@text.uri"]              = { fg = p.${d.keyword}, underline = true },
  ["@text.todo"]             = { fg = p.${d.string}, bold = true },
  ["@text.note"]             = { fg = p.${d.keyword}, bold = true },
  ["@text.warning"]          = { fg = p.${d.string}, bold = true },
  ["@text.danger"]           = { fg = p.${d.deleted}, bold = true },
  ["@text.diff.add"]         = { fg = p.${d.inserted} },
  ["@text.diff.delete"]      = { fg = p.${d.deleted} },

  -- Tags
  ["@tag"]                   = { fg = p.${d.tag} },
  ["@tag.attribute"]         = { fg = p.${d.attribute} },
  ["@tag.delimiter"]         = { fg = p.${d.punctuation} },

  -- Markup (new treesitter captures)
  ["@markup.heading"]        = { fg = p.${d.keyword}, bold = true },
  ["@markup.italic"]         = { italic = true },
  ["@markup.strong"]         = { bold = true },
  ["@markup.raw"]            = { fg = p.${d.string} },
  ["@markup.link"]           = { fg = p.${d.keyword}, underline = true },
  ["@markup.link.url"]       = { fg = p.${d.string}, underline = true },
  ["@markup.list"]           = { fg = p.${d.punctuation} },
}

local light = {
  -- Misc
  ["@comment"]               = { link = "Comment" },
  ["@error"]                 = { fg = p.${dl.deleted} },
  ["@preproc"]               = { fg = p.${dl.keyword} },

  -- Punctuation
  ["@punctuation.delimiter"] = { fg = p.${dl.punctuation} },
  ["@punctuation.bracket"]   = { fg = p.${dl.punctuation} },
  ["@punctuation.special"]   = { fg = p.${dl.punctuation} },

  -- Literals
  ["@string"]                = { fg = p.${dl.string} },
  ["@string.regex"]          = { fg = p.${dl.regex} },
  ["@string.escape"]         = { fg = p.${dl.escape} },
  ["@string.special"]        = { fg = p.${dl.escape} },
  ["@character"]             = { fg = p.${dl.string} },
  ["@boolean"]               = { fg = p.${dl.constant} },
  ["@number"]                = { fg = p.${dl.number} },
  ["@float"]                 = { fg = p.${dl.number} },

  -- Functions
  ["@function"]              = { fg = p.${dl.function} },
  ["@function.call"]         = { fg = p.${dl.function} },
  ["@function.builtin"]      = { fg = p.${dl.function} },
  ["@function.macro"]        = { fg = p.${dl.keyword} },
  ["@method"]                = { fg = p.${dl.function} },
  ["@method.call"]           = { fg = p.${dl.function} },
  ["@constructor"]           = { fg = p.${dl.type} },

  -- Keywords
  ["@keyword"]               = { fg = p.${dl.keyword} },
  ["@keyword.function"]      = { fg = p.${dl.keyword} },
  ["@keyword.operator"]      = { fg = p.${dl.keyword} },
  ["@keyword.return"]        = { fg = p.${dl.keyword} },
  ["@conditional"]           = { fg = p.${dl.keyword} },
  ["@repeat"]                = { fg = p.${dl.keyword} },
  ["@label"]                 = { fg = p.${dl.keyword} },
  ["@include"]               = { fg = p.${dl.keyword} },
  ["@exception"]             = { fg = p.${dl.keyword} },

  -- Types
  ["@type"]                  = { fg = p.${dl.type} },
  ["@type.builtin"]          = { fg = p.${dl.type} },
  ["@type.qualifier"]        = { fg = p.${dl.keyword} },
  ["@type.definition"]       = { fg = p.${dl.type} },
  ["@storageclass"]          = { fg = p.${dl.keyword} },
  ["@attribute"]             = { fg = p.${dl.attribute} },
  ["@field"]                 = { fg = p.${dl.variable} },
  ["@property"]              = { fg = p.${dl.variable} },

  -- Identifiers
  ["@variable"]              = { fg = p.${dl.variable} },
  ["@variable.builtin"]      = { fg = p.${dl.constant} },
  ["@constant"]              = { fg = p.${dl.constant} },
  ["@constant.builtin"]      = { fg = p.${dl.constant} },
  ["@constant.macro"]        = { fg = p.${dl.constant} },
  ["@namespace"]             = { fg = p.${dl.type} },
  ["@module"]                = { fg = p.${dl.type} },
  ["@symbol"]                = { fg = p.${dl.keyword} },

  -- Text / markup
  ["@text"]                  = { fg = p.${dl.variable} },
  ["@text.strong"]           = { bold = true },
  ["@text.emphasis"]         = { italic = true },
  ["@text.underline"]        = { underline = true },
  ["@text.strike"]           = { strikethrough = true },
  ["@text.title"]            = { fg = p.${dl.keyword}, bold = true },
  ["@text.literal"]          = { fg = p.${dl.string} },
  ["@text.uri"]              = { fg = p.${dl.keyword}, underline = true },
  ["@text.todo"]             = { fg = p.${dl.string}, bold = true },
  ["@text.note"]             = { fg = p.${dl.keyword}, bold = true },
  ["@text.warning"]          = { fg = p.${dl.string}, bold = true },
  ["@text.danger"]           = { fg = p.${dl.deleted}, bold = true },
  ["@text.diff.add"]         = { fg = p.${dl.inserted} },
  ["@text.diff.delete"]      = { fg = p.${dl.deleted} },

  -- Tags
  ["@tag"]                   = { fg = p.${dl.tag} },
  ["@tag.attribute"]         = { fg = p.${dl.attribute} },
  ["@tag.delimiter"]         = { fg = p.${dl.punctuation} },

  -- Markup (new treesitter captures)
  ["@markup.heading"]        = { fg = p.${dl.keyword}, bold = true },
  ["@markup.italic"]         = { italic = true },
  ["@markup.strong"]         = { bold = true },
  ["@markup.raw"]            = { fg = p.${dl.string} },
  ["@markup.link"]           = { fg = p.${dl.keyword}, underline = true },
  ["@markup.link.url"]       = { fg = p.${dl.string}, underline = true },
  ["@markup.list"]           = { fg = p.${dl.punctuation} },
}

function M.highlights(mode)
  return mode == "light" and light or dark
end

return M
`;
}

// ---------------------------------------------------------------------------
// lsp.lua
// ---------------------------------------------------------------------------
function buildLsp() {
	const ud = refs.ui.dark;
	const ul = refs.ui.light;
	const ed = refs.editor.dark;
	const el = refs.editor.light;
	const sd = refs.syntax.dark;
	const sl = refs.syntax.light;

	return `-- Liminal Salt — LSP and diagnostic highlights
local p = require("liminal-salt.palette").p

local M = {}

local dark = {
  -- Diagnostics
  DiagnosticError          = { fg = p.${ud.destructive} },
  DiagnosticWarn           = { fg = p.${ud.warning} },
  DiagnosticInfo           = { fg = p.${ud.accent} },
  DiagnosticHint           = { fg = p.${ud.accent} },
  DiagnosticOk             = { fg = p.${ud.success} },

  -- Virtual text
  DiagnosticVirtualTextError = { fg = p.${ud.destructive} },
  DiagnosticVirtualTextWarn  = { fg = p.${ud.warning} },
  DiagnosticVirtualTextInfo  = { fg = p.${ud.accent} },
  DiagnosticVirtualTextHint  = { fg = p.${ud.accent} },
  DiagnosticVirtualTextOk   = { fg = p.${ud.success} },

  -- Underline
  DiagnosticUnderlineError = { undercurl = true, sp = p.${ud.destructive} },
  DiagnosticUnderlineWarn  = { undercurl = true, sp = p.${ud.warning} },
  DiagnosticUnderlineInfo  = { undercurl = true, sp = p.${ud.accent} },
  DiagnosticUnderlineHint  = { undercurl = true, sp = p.${ud.accent} },
  DiagnosticUnderlineOk    = { undercurl = true, sp = p.${ud.success} },

  -- Sign
  DiagnosticSignError = { fg = p.${ud.destructive} },
  DiagnosticSignWarn  = { fg = p.${ud.warning} },
  DiagnosticSignInfo  = { fg = p.${ud.accent} },
  DiagnosticSignHint  = { fg = p.${ud.accent} },
  DiagnosticSignOk    = { fg = p.${ud.success} },

  -- LSP references
  LspReferenceText  = { bg = p.${ed.selection} },
  LspReferenceRead  = { bg = p.${ed.selection} },
  LspReferenceWrite = { bg = p.${ed.selection} },

  -- LSP inlay hints
  LspInlayHint = { fg = p.${ud.mutedForeground}, italic = true },

  -- LSP code lens
  LspCodeLens          = { fg = p.${ud.mutedForeground} },
  LspCodeLensSeparator = { fg = p.${ud.border} },

  -- LSP signature
  LspSignatureActiveParameter = { bg = p.${ed.selection}, bold = true },
}

local light = {
  -- Diagnostics
  DiagnosticError          = { fg = p.${ul.destructive} },
  DiagnosticWarn           = { fg = p.${ul.warning} },
  DiagnosticInfo           = { fg = p.${ul.accent} },
  DiagnosticHint           = { fg = p.${ul.accent} },
  DiagnosticOk             = { fg = p.${ul.success} },

  -- Virtual text
  DiagnosticVirtualTextError = { fg = p.${ul.destructive} },
  DiagnosticVirtualTextWarn  = { fg = p.${ul.warning} },
  DiagnosticVirtualTextInfo  = { fg = p.${ul.accent} },
  DiagnosticVirtualTextHint  = { fg = p.${ul.accent} },
  DiagnosticVirtualTextOk   = { fg = p.${ul.success} },

  -- Underline
  DiagnosticUnderlineError = { undercurl = true, sp = p.${ul.destructive} },
  DiagnosticUnderlineWarn  = { undercurl = true, sp = p.${ul.warning} },
  DiagnosticUnderlineInfo  = { undercurl = true, sp = p.${ul.accent} },
  DiagnosticUnderlineHint  = { undercurl = true, sp = p.${ul.accent} },
  DiagnosticUnderlineOk    = { undercurl = true, sp = p.${ul.success} },

  -- Sign
  DiagnosticSignError = { fg = p.${ul.destructive} },
  DiagnosticSignWarn  = { fg = p.${ul.warning} },
  DiagnosticSignInfo  = { fg = p.${ul.accent} },
  DiagnosticSignHint  = { fg = p.${ul.accent} },
  DiagnosticSignOk    = { fg = p.${ul.success} },

  -- LSP references
  LspReferenceText  = { bg = p.${el.selection} },
  LspReferenceRead  = { bg = p.${el.selection} },
  LspReferenceWrite = { bg = p.${el.selection} },

  -- LSP inlay hints
  LspInlayHint = { fg = p.${ul.mutedForeground}, italic = true },

  -- LSP code lens
  LspCodeLens          = { fg = p.${ul.mutedForeground} },
  LspCodeLensSeparator = { fg = p.${ul.border} },

  -- LSP signature
  LspSignatureActiveParameter = { bg = p.${el.selection}, bold = true },
}

function M.highlights(mode)
  return mode == "light" and light or dark
end

return M
`;
}

// ---------------------------------------------------------------------------
// terminal.lua
// ---------------------------------------------------------------------------
function buildTerminal() {
	const ad = refs.ansi.dark;
	const al = refs.ansi.light;

	return `-- Liminal Salt — terminal colors
local p = require("liminal-salt.palette").p

local M = {}

local colors = {
  dark = {
    p.${ad.black}, p.${ad.red}, p.${ad.green}, p.${ad.yellow},
    p.${ad.blue}, p.${ad.magenta}, p.${ad.cyan}, p.${ad.white},
    p.${ad.brightBlack}, p.${ad.brightRed}, p.${ad.brightGreen}, p.${ad.brightYellow},
    p.${ad.brightBlue}, p.${ad.brightMagenta}, p.${ad.brightCyan}, p.${ad.brightWhite},
  },
  light = {
    p.${al.black}, p.${al.red}, p.${al.green}, p.${al.yellow},
    p.${al.blue}, p.${al.magenta}, p.${al.cyan}, p.${al.white},
    p.${al.brightBlack}, p.${al.brightRed}, p.${al.brightGreen}, p.${al.brightYellow},
    p.${al.brightBlue}, p.${al.brightMagenta}, p.${al.brightCyan}, p.${al.brightWhite},
  },
}

function M.apply(mode)
  local c = colors[mode] or colors.dark
  for i = 0, 15 do
    vim.g["terminal_color_" .. i] = c[i + 1]
  end
end

return M
`;
}

// ---------------------------------------------------------------------------
// plugins/gitsigns.lua
// ---------------------------------------------------------------------------
function buildGitsigns() {
	const sd = refs.syntax.dark;
	const sl = refs.syntax.light;
	const ed = refs.editor.dark;
	const el = refs.editor.light;
	const ud = refs.ui.dark;
	const ul = refs.ui.light;

	return `-- Liminal Salt — gitsigns highlights
local p = require("liminal-salt.palette").p

local M = {}

local dark = {
  GitSignsAdd          = { fg = p.${sd.inserted} },
  GitSignsChange       = { fg = p.${sd.string} },
  GitSignsDelete       = { fg = p.${sd.deleted} },
  GitSignsAddNr        = { fg = p.${sd.inserted} },
  GitSignsChangeNr     = { fg = p.${sd.string} },
  GitSignsDeleteNr     = { fg = p.${sd.deleted} },
  GitSignsAddLn        = { bg = p.${ed.diffInsertedBackground} },
  GitSignsChangeLn     = { bg = p.${ed.lineHighlight} },
  GitSignsDeleteLn     = { bg = p.${ed.diffDeletedBackground} },
  GitSignsCurrentLineBlame = { fg = p.${ud.mutedForeground}, italic = true },
}

local light = {
  GitSignsAdd          = { fg = p.${sl.inserted} },
  GitSignsChange       = { fg = p.${sl.string} },
  GitSignsDelete       = { fg = p.${sl.deleted} },
  GitSignsAddNr        = { fg = p.${sl.inserted} },
  GitSignsChangeNr     = { fg = p.${sl.string} },
  GitSignsDeleteNr     = { fg = p.${sl.deleted} },
  GitSignsAddLn        = { bg = p.${el.diffInsertedBackground} },
  GitSignsChangeLn     = { bg = p.${el.lineHighlight} },
  GitSignsDeleteLn     = { bg = p.${el.diffDeletedBackground} },
  GitSignsCurrentLineBlame = { fg = p.${ul.mutedForeground}, italic = true },
}

function M.highlights(mode)
  return mode == "light" and light or dark
end

return M
`;
}

// ---------------------------------------------------------------------------
// plugins/snacks.lua
// ---------------------------------------------------------------------------
function buildSnacks() {
	const ud = refs.ui.dark;
	const ul = refs.ui.light;
	const sd = refs.syntax.dark;
	const sl = refs.syntax.light;

	return `-- Liminal Salt — snacks.nvim highlights
local p = require("liminal-salt.palette").p

local M = {}

local dark = {
  -- Picker sidebar (surface-secondary for darker sidebar)
  SnacksPickerList        = { fg = p.${ud.foreground}, bg = p.${ud.muted} },
  SnacksPickerInput       = { fg = p.${ud.foreground}, bg = p.${ud.muted} },
  SnacksPickerInputBorder = { fg = p.${ud.border}, bg = p.${ud.muted} },
  SnacksPickerBox         = { fg = p.${ud.foreground}, bg = p.${ud.muted} },
  SnacksPickerBoxBorder   = { fg = p.${ud.border}, bg = p.${ud.muted} },

  -- Picker content
  SnacksPickerDir             = { fg = p.${ud.mutedForeground} },
  SnacksPickerTotals          = { fg = p.${ud.mutedForeground} },
  SnacksPickerMatch           = { fg = p.${ud.accent}, bold = true },
  SnacksPickerBufFlags        = { fg = p.${ud.mutedForeground} },
  SnacksPickerPathHidden      = { fg = p.${ud.mutedForeground} },
  SnacksPickerPathIgnored     = { fg = p.${ud.mutedForeground} },
  SnacksPickerGitStatusIgnored   = { fg = p.${ud.mutedForeground} },
  SnacksPickerGitStatusUntracked = { fg = p.${sd.inserted} },

  -- Dashboard
  SnacksDashboardDir    = { fg = p.${ud.mutedForeground} },
  SnacksDashboardHeader = { fg = p.${ud.accent} },
  SnacksDashboardIcon   = { fg = p.${ud.accent} },
  SnacksDashboardKey    = { fg = p.teal400 },
  SnacksDashboardTitle  = { fg = p.${ud.accent}, bold = true },
  SnacksDashboardDesc   = { fg = p.${ud.foregroundSecondary} },

  -- Indent
  SnacksIndent      = { fg = p.${ud.border} },
  SnacksIndentScope = { fg = p.sage600 },

  -- Notifier
  SnacksNotifierInfo  = { fg = p.${ud.accent} },
  SnacksNotifierWarn  = { fg = p.${ud.warning} },
  SnacksNotifierError = { fg = p.${ud.destructive} },
}

local light = {
  -- Picker sidebar (surface-secondary for darker sidebar)
  SnacksPickerList        = { fg = p.${ul.foreground}, bg = p.${ul.muted} },
  SnacksPickerInput       = { fg = p.${ul.foreground}, bg = p.${ul.muted} },
  SnacksPickerInputBorder = { fg = p.${ul.border}, bg = p.${ul.muted} },
  SnacksPickerBox         = { fg = p.${ul.foreground}, bg = p.${ul.muted} },
  SnacksPickerBoxBorder   = { fg = p.${ul.border}, bg = p.${ul.muted} },

  -- Picker content
  SnacksPickerDir             = { fg = p.${ul.mutedForeground} },
  SnacksPickerTotals          = { fg = p.${ul.mutedForeground} },
  SnacksPickerMatch           = { fg = p.${ul.accent}, bold = true },
  SnacksPickerBufFlags        = { fg = p.${ul.mutedForeground} },
  SnacksPickerPathHidden      = { fg = p.${ul.mutedForeground} },
  SnacksPickerPathIgnored     = { fg = p.${ul.mutedForeground} },
  SnacksPickerGitStatusIgnored   = { fg = p.${ul.mutedForeground} },
  SnacksPickerGitStatusUntracked = { fg = p.${sl.inserted} },

  -- Dashboard
  SnacksDashboardDir    = { fg = p.${ul.mutedForeground} },
  SnacksDashboardHeader = { fg = p.${ul.accent} },
  SnacksDashboardIcon   = { fg = p.${ul.accent} },
  SnacksDashboardKey    = { fg = p.teal700 },
  SnacksDashboardTitle  = { fg = p.${ul.accent}, bold = true },
  SnacksDashboardDesc   = { fg = p.${ul.foregroundSecondary} },

  -- Indent
  SnacksIndent      = { fg = p.${ul.border} },
  SnacksIndentScope = { fg = p.sage700 },

  -- Notifier
  SnacksNotifierInfo  = { fg = p.${ul.accent} },
  SnacksNotifierWarn  = { fg = p.${ul.warning} },
  SnacksNotifierError = { fg = p.${ul.destructive} },
}

function M.highlights(mode)
  return mode == "light" and light or dark
end

return M
`;
}

// ---------------------------------------------------------------------------
// colors/liminal-salt-{mode}.lua
// ---------------------------------------------------------------------------
function buildColorscheme(mode) {
	return `require("liminal-salt").load("${mode}")
`;
}

// ---------------------------------------------------------------------------
// lualine theme
// ---------------------------------------------------------------------------
function buildLualine(mode) {
	const u = theme.ui[mode];
	const isDark = mode === "dark";

	// Lualine inlines a palette subset for zero-dependency loading
	const accent = isDark ? prims.sage400 : prims.sage700;
	const insert = isDark ? prims.teal400 : prims.teal700;
	const visual = isDark ? prims.amber400 : prims.amber700;
	const replace = isDark ? prims.red400 : prims.red600;
	const command = isDark ? prims.teal500 : prims.teal800;
	const fg = u.foreground;
	const fgMuted = u.mutedForeground;
	const bgMain = u.background;
	const bgChrome = u.card;
	const bgSep = isDark ? prims.stone50 : prims.beige400;

	return `-- Liminal Salt ${isDark ? "Dark" : "Light"} — lualine theme
-- Colors from canonical theme: github.com/irvj/liminal-salt

local p = {
  accent  = "${accent}",
  insert  = "${insert}",
  visual  = "${visual}",
  replace = "${replace}",
  command = "${command}",
  fg      = "${fg}",
  fgMuted = "${fgMuted}",
  bgMain  = "${bgMain}",
  bgChrome = "${bgChrome}",
  bgSep   = "${bgSep}",
}

return {
  normal = {
    a = { bg = p.accent, fg = p.bgMain, gui = "bold" },
    b = { bg = p.bgSep, fg = p.fg },
    c = { bg = p.bgChrome, fg = p.fgMuted },
  },
  insert = {
    a = { bg = p.insert, fg = p.bgMain, gui = "bold" },
    b = { bg = p.bgSep, fg = p.fg },
    c = { bg = p.bgChrome, fg = p.fgMuted },
  },
  visual = {
    a = { bg = p.visual, fg = p.bgMain, gui = "bold" },
    b = { bg = p.bgSep, fg = p.fg },
    c = { bg = p.bgChrome, fg = p.fgMuted },
  },
  replace = {
    a = { bg = p.replace, fg = p.bgMain, gui = "bold" },
    b = { bg = p.bgSep, fg = p.fg },
    c = { bg = p.bgChrome, fg = p.fgMuted },
  },
  command = {
    a = { bg = p.command, fg = p.bgMain, gui = "bold" },
    b = { bg = p.bgSep, fg = p.fg },
    c = { bg = p.bgChrome, fg = p.fgMuted },
  },
  terminal = {
    a = { bg = p.command, fg = p.bgMain, gui = "bold" },
    b = { bg = p.bgSep, fg = p.fg },
    c = { bg = p.bgChrome, fg = p.fgMuted },
  },
  inactive = {
    a = { bg = p.bgChrome, fg = p.fgMuted },
    b = { bg = p.bgChrome, fg = p.fgMuted },
    c = { bg = p.bgChrome, fg = p.fgMuted },
  },
}
`;
}

// ---------------------------------------------------------------------------
// Main export
// ---------------------------------------------------------------------------
export default function exportVim(outDir) {
	const base = `${outDir}/vim`;
	const dirs = [
		`${base}/colors`,
		`${base}/lua/liminal-salt/plugins`,
		`${base}/lua/lualine/themes`,
	];
	for (const d of dirs) mkdirSync(d, { recursive: true });

	// Plugin core
	write(`${base}/lua/liminal-salt/palette.lua`, buildPalette());
	write(`${base}/lua/liminal-salt/init.lua`, buildInit());
	write(`${base}/lua/liminal-salt/editor.lua`, buildEditor());
	write(`${base}/lua/liminal-salt/syntax.lua`, buildSyntax());
	write(`${base}/lua/liminal-salt/treesitter.lua`, buildTreesitter());
	write(`${base}/lua/liminal-salt/lsp.lua`, buildLsp());
	write(`${base}/lua/liminal-salt/terminal.lua`, buildTerminal());

	// Plugin integrations
	write(`${base}/lua/liminal-salt/plugins/gitsigns.lua`, buildGitsigns());
	write(`${base}/lua/liminal-salt/plugins/snacks.lua`, buildSnacks());

	// Colorscheme entry points
	write(`${base}/colors/liminal-salt-dark.lua`, buildColorscheme("dark"));
	write(`${base}/colors/liminal-salt-light.lua`, buildColorscheme("light"));

	// Lualine themes
	write(`${base}/lua/lualine/themes/liminal-salt-dark.lua`, buildLualine("dark"));
	write(`${base}/lua/lualine/themes/liminal-salt-light.lua`, buildLualine("light"));
}
