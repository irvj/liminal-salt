-- Liminal Salt — canonical palette
-- Source: github.com/irvj/liminal-salt/src/theme.js

local M = {}

M.primitives = {
  -- Beige / warm neutrals
  beige50        = "#fdfcfa",
  beige100       = "#f5f2ed",
  beige200       = "#ebe7e0",
  beige300       = "#e8e4dc",
  beige400       = "#ddd8d0",
  beige500       = "#c5c1b8",
  beige600       = "#9e9b93",
  beige700       = "#888379",
  beige800       = "#6b6762",
  beige900       = "#5a5753",
  beige950       = "#2d2b28",
  beigeTint15    = "#393a38",
  beigeTint15L   = "#d7d4cf",

  -- Stone / cool neutrals
  stone50        = "#2e312f",
  stone100       = "#242726",
  stone200       = "#1a1c1b",
  stone300       = "#141615",

  -- Sage / green
  sage300        = "#a3bfac",
  sage400        = "#8fac98",
  sage500        = "#7dba8a",
  sage600        = "#6b7369",
  sage700        = "#506e58",
  sage800        = "#425f4a",
  sage900        = "#3a7346",
  sageTint30     = "#3d4741",
  sageTint30L    = "#c4cac0",

  -- Teal
  teal400        = "#95bebe",
  teal500        = "#8fb8ad",
  teal700        = "#3e5d5d",
  teal800        = "#3e6b5d",

  -- Red
  red300         = "#d99292",
  red400         = "#cc8585",
  red600         = "#a54d4d",
  red700         = "#984242",
  redTint30      = "#4f3c3b",
  redTint30L     = "#ddc1bd",

  -- Amber
  amber400       = "#c9a86c",
  amber700       = "#7d6325",
  amberTint30    = "#4f4633",
  amberTint30L   = "#d1c7b1",

  -- Blue
  blue400        = "#7eb8c9",
  blue700        = "#3a6f80",

  -- Olive
  olive400       = "#a9ad78",
  olive700       = "#5c6135",

  -- Orange
  orange400      = "#c9956c",
  orange700      = "#8a5c30",

  -- Tinted backgrounds (dark)
  greenTint30    = "#384b3c",

  -- Tinted backgrounds (light)
  greenTint30L   = "#bdccbb",
}

-- Convenience alias
M.p = M.primitives

return M
