-- Liminal Salt Dark — lualine theme
-- Colors from canonical theme: github.com/irvj/liminal-salt

local p = {
  accent  = "#8fac98",
  insert  = "#95bebe",
  visual  = "#c9a86c",
  replace = "#cc8585",
  command = "#8fb8ad",
  fg      = "#e8e4dc",
  fgMuted = "#9e9b93",
  bgMain  = "#1a1c1b",
  bgChrome = "#242726",
  bgSep   = "#2e312f",
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
