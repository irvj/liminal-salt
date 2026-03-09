-- Liminal Salt — terminal colors
local p = require("liminal-salt.palette").p

local M = {}

local colors = {
  dark = {
    p.stone300, p.red400, p.sage500, p.amber400,
    p.blue400, p.orange400, p.teal400, p.beige500,
    p.sage600, p.red300, p.sage300, p.amber400,
    p.blue400, p.orange400, p.teal400, p.beige300,
  },
  light = {
    p.beige950, p.red600, p.sage900, p.amber700,
    p.blue700, p.orange700, p.teal700, p.beige200,
    p.beige800, p.red700, p.sage800, p.amber700,
    p.blue700, p.orange700, p.teal700, p.beige100,
  },
}

function M.apply(mode)
  local c = colors[mode] or colors.dark
  for i = 0, 15 do
    vim.g["terminal_color_" .. i] = c[i + 1]
  end
end

return M
