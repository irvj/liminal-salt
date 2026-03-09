-- Liminal Salt — gitsigns highlights
local p = require("liminal-salt.palette").p

local M = {}

local dark = {
  GitSignsAdd          = { fg = p.sage500 },
  GitSignsChange       = { fg = p.amber400 },
  GitSignsDelete       = { fg = p.red400 },
  GitSignsAddNr        = { fg = p.sage500 },
  GitSignsChangeNr     = { fg = p.amber400 },
  GitSignsDeleteNr     = { fg = p.red400 },
  GitSignsAddLn        = { bg = p.greenTint30 },
  GitSignsChangeLn     = { bg = p.beigeTint15 },
  GitSignsDeleteLn     = { bg = p.redTint30 },
  GitSignsCurrentLineBlame = { fg = p.beige600, italic = true },
}

local light = {
  GitSignsAdd          = { fg = p.sage900 },
  GitSignsChange       = { fg = p.amber700 },
  GitSignsDelete       = { fg = p.red600 },
  GitSignsAddNr        = { fg = p.sage900 },
  GitSignsChangeNr     = { fg = p.amber700 },
  GitSignsDeleteNr     = { fg = p.red600 },
  GitSignsAddLn        = { bg = p.greenTint30L },
  GitSignsChangeLn     = { bg = p.beigeTint15L },
  GitSignsDeleteLn     = { bg = p.redTint30L },
  GitSignsCurrentLineBlame = { fg = p.beige800, italic = true },
}

function M.highlights(mode)
  return mode == "light" and light or dark
end

return M
