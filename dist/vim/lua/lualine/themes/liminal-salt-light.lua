-- Liminal Salt Light — lualine theme
-- Colors from canonical theme: github.com/irvj/liminal-salt

local p = {
  accent  = "#506e58",
  insert  = "#3e5d5d",
  visual  = "#7d6325",
  replace = "#a54d4d",
  command = "#3e6b5d",
  fg      = "#2d2b28",
  fgMuted = "#6b6762",
  bgMain  = "#f5f2ed",
  bgChrome = "#fdfcfa",
  bgSep   = "#ddd8d0",
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
