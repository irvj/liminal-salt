-- Liminal Salt — editor UI highlights
local p = require("liminal-salt.palette").p

local M = {}

local dark = {
  -- Editor
  Normal       = { fg = p.beige300, bg = p.stone200 },
  NormalNC     = { fg = p.beige300, bg = p.stone200 },
  CursorLine   = { bg = p.beigeTint15 },
  CursorLineNr = { fg = p.beige500, bg = p.beigeTint15 },
  LineNr       = { fg = p.beige600 },
  Visual       = { bg = p.sageTint30 },
  Search       = { bg = p.amberTint30 },
  IncSearch    = { bg = p.amberTint30, bold = true },
  CurSearch    = { bg = p.amberTint30, bold = true },
  MatchParen   = { bg = p.stone50, bold = true },
  NonText      = { fg = p.stone50 },
  SpecialKey   = { fg = p.stone50 },
  Cursor       = { fg = p.stone200, bg = p.sage400 },
  lCursor      = { fg = p.stone200, bg = p.sage400 },
  CursorIM     = { fg = p.stone200, bg = p.sage400 },
  SignColumn   = { fg = p.beige600, bg = p.stone200 },
  FoldColumn   = { fg = p.beige600, bg = p.stone200 },
  Folded       = { fg = p.beige600, bg = p.stone300 },
  ColorColumn  = { bg = p.stone300 },
  Conceal      = { fg = p.beige600 },
  Directory    = { fg = p.sage400 },
  EndOfBuffer  = { fg = p.stone200 },
  WildMenu     = { fg = p.beige300, bg = p.sageTint30 },
  QuickFixLine = { bg = p.sageTint30 },
  Substitute   = { bg = p.amberTint30 },

  -- UI chrome
  StatusLine   = { fg = p.beige300, bg = p.stone100 },
  StatusLineNC = { fg = p.beige600, bg = p.stone100 },
  VertSplit    = { fg = p.stone50, bg = p.stone200 },
  WinSeparator = { fg = p.stone50, bg = p.stone200 },
  TabLine      = { fg = p.beige600, bg = p.stone100 },
  TabLineFill  = { bg = p.stone100 },
  TabLineSel   = { fg = p.beige300, bg = p.stone200, bold = true },
  Title        = { fg = p.sage400, bold = true },

  -- Popup menu
  Pmenu        = { fg = p.beige300, bg = p.stone100 },
  PmenuSel     = { fg = p.beige300, bg = p.sageTint30 },
  PmenuSbar    = { bg = p.stone300 },
  PmenuThumb   = { bg = p.stone50 },

  -- Floating windows
  NormalFloat  = { fg = p.beige300, bg = p.stone100 },
  FloatBorder  = { fg = p.stone50, bg = p.stone100 },
  FloatTitle   = { fg = p.sage400, bg = p.stone100, bold = true },

  -- Messages
  ErrorMsg   = { fg = p.red400, bold = true },
  WarningMsg = { fg = p.amber400 },
  MoreMsg    = { fg = p.sage400 },
  ModeMsg    = { fg = p.beige300, bold = true },
  Question   = { fg = p.sage400 },

  -- Diff
  DiffAdd    = { bg = p.greenTint30 },
  DiffDelete = { bg = p.redTint30 },
  DiffChange = { bg = p.beigeTint15 },
  DiffText   = { bg = p.amberTint30, bold = true },

  -- Spell
  SpellBad   = { undercurl = true, sp = p.red400 },
  SpellCap   = { undercurl = true, sp = p.amber400 },
  SpellLocal = { undercurl = true, sp = p.teal400 },
  SpellRare  = { undercurl = true, sp = p.blue400 },
}

local light = {
  -- Editor
  Normal       = { fg = p.beige950, bg = p.beige100 },
  NormalNC     = { fg = p.beige950, bg = p.beige100 },
  CursorLine   = { bg = p.beigeTint15L },
  CursorLineNr = { fg = p.beige900, bg = p.beigeTint15L },
  LineNr       = { fg = p.beige800 },
  Visual       = { bg = p.sageTint30L },
  Search       = { bg = p.amberTint30L },
  IncSearch    = { bg = p.amberTint30L, bold = true },
  CurSearch    = { bg = p.amberTint30L, bold = true },
  MatchParen   = { bg = p.beige400, bold = true },
  NonText      = { fg = p.beige400 },
  SpecialKey   = { fg = p.beige400 },
  Cursor       = { fg = p.beige100, bg = p.sage700 },
  lCursor      = { fg = p.beige100, bg = p.sage700 },
  CursorIM     = { fg = p.beige100, bg = p.sage700 },
  SignColumn   = { fg = p.beige800, bg = p.beige100 },
  FoldColumn   = { fg = p.beige800, bg = p.beige100 },
  Folded       = { fg = p.beige800, bg = p.beige200 },
  ColorColumn  = { bg = p.beige200 },
  Conceal      = { fg = p.beige800 },
  Directory    = { fg = p.sage700 },
  EndOfBuffer  = { fg = p.beige100 },
  WildMenu     = { fg = p.beige950, bg = p.sageTint30L },
  QuickFixLine = { bg = p.sageTint30L },
  Substitute   = { bg = p.amberTint30L },

  -- UI chrome
  StatusLine   = { fg = p.beige950, bg = p.beige50 },
  StatusLineNC = { fg = p.beige800, bg = p.beige50 },
  VertSplit    = { fg = p.beige400, bg = p.beige100 },
  WinSeparator = { fg = p.beige400, bg = p.beige100 },
  TabLine      = { fg = p.beige800, bg = p.beige50 },
  TabLineFill  = { bg = p.beige50 },
  TabLineSel   = { fg = p.beige950, bg = p.beige100, bold = true },
  Title        = { fg = p.sage700, bold = true },

  -- Popup menu
  Pmenu        = { fg = p.beige950, bg = p.beige50 },
  PmenuSel     = { fg = p.beige950, bg = p.sageTint30L },
  PmenuSbar    = { bg = p.beige200 },
  PmenuThumb   = { bg = p.beige400 },

  -- Floating windows
  NormalFloat  = { fg = p.beige950, bg = p.beige50 },
  FloatBorder  = { fg = p.beige400, bg = p.beige50 },
  FloatTitle   = { fg = p.sage700, bg = p.beige50, bold = true },

  -- Messages
  ErrorMsg   = { fg = p.red600, bold = true },
  WarningMsg = { fg = p.amber700 },
  MoreMsg    = { fg = p.sage700 },
  ModeMsg    = { fg = p.beige950, bold = true },
  Question   = { fg = p.sage700 },

  -- Diff
  DiffAdd    = { bg = p.greenTint30L },
  DiffDelete = { bg = p.redTint30L },
  DiffChange = { bg = p.beigeTint15L },
  DiffText   = { bg = p.amberTint30L, bold = true },

  -- Spell
  SpellBad   = { undercurl = true, sp = p.red600 },
  SpellCap   = { undercurl = true, sp = p.amber700 },
  SpellLocal = { undercurl = true, sp = p.teal700 },
  SpellRare  = { undercurl = true, sp = p.blue700 },
}

function M.highlights(mode)
  return mode == "light" and light or dark
end

return M
