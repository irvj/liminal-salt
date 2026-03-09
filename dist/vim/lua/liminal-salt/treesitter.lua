-- Liminal Salt — treesitter highlights
local p = require("liminal-salt.palette").p

local M = {}

local dark = {
  -- Misc
  ["@comment"]               = { link = "Comment" },
  ["@error"]                 = { fg = p.red400 },
  ["@preproc"]               = { fg = p.sage400 },

  -- Punctuation
  ["@punctuation.delimiter"] = { fg = p.beige500 },
  ["@punctuation.bracket"]   = { fg = p.beige500 },
  ["@punctuation.special"]   = { fg = p.beige500 },

  -- Literals
  ["@string"]                = { fg = p.amber400 },
  ["@string.regex"]          = { fg = p.olive400 },
  ["@string.escape"]         = { fg = p.orange400 },
  ["@string.special"]        = { fg = p.orange400 },
  ["@character"]             = { fg = p.amber400 },
  ["@boolean"]               = { fg = p.blue400 },
  ["@number"]                = { fg = p.blue400 },
  ["@float"]                 = { fg = p.blue400 },

  -- Functions
  ["@function"]              = { fg = p.sage300 },
  ["@function.call"]         = { fg = p.sage300 },
  ["@function.builtin"]      = { fg = p.sage300 },
  ["@function.macro"]        = { fg = p.sage400 },
  ["@method"]                = { fg = p.sage300 },
  ["@method.call"]           = { fg = p.sage300 },
  ["@constructor"]           = { fg = p.teal500 },

  -- Keywords
  ["@keyword"]               = { fg = p.sage400 },
  ["@keyword.function"]      = { fg = p.sage400 },
  ["@keyword.operator"]      = { fg = p.sage400 },
  ["@keyword.return"]        = { fg = p.sage400 },
  ["@conditional"]           = { fg = p.sage400 },
  ["@repeat"]                = { fg = p.sage400 },
  ["@label"]                 = { fg = p.sage400 },
  ["@include"]               = { fg = p.sage400 },
  ["@exception"]             = { fg = p.sage400 },

  -- Types
  ["@type"]                  = { fg = p.teal500 },
  ["@type.builtin"]          = { fg = p.teal500 },
  ["@type.qualifier"]        = { fg = p.sage400 },
  ["@type.definition"]       = { fg = p.teal500 },
  ["@storageclass"]          = { fg = p.sage400 },
  ["@attribute"]             = { fg = p.amber400 },
  ["@field"]                 = { fg = p.beige300 },
  ["@property"]              = { fg = p.beige300 },

  -- Identifiers
  ["@variable"]              = { fg = p.beige300 },
  ["@variable.builtin"]      = { fg = p.blue400 },
  ["@constant"]              = { fg = p.blue400 },
  ["@constant.builtin"]      = { fg = p.blue400 },
  ["@constant.macro"]        = { fg = p.blue400 },
  ["@namespace"]             = { fg = p.teal500 },
  ["@module"]                = { fg = p.teal500 },
  ["@symbol"]                = { fg = p.sage400 },

  -- Text / markup
  ["@text"]                  = { fg = p.beige300 },
  ["@text.strong"]           = { bold = true },
  ["@text.emphasis"]         = { italic = true },
  ["@text.underline"]        = { underline = true },
  ["@text.strike"]           = { strikethrough = true },
  ["@text.title"]            = { fg = p.sage400, bold = true },
  ["@text.literal"]          = { fg = p.amber400 },
  ["@text.uri"]              = { fg = p.sage400, underline = true },
  ["@text.todo"]             = { fg = p.amber400, bold = true },
  ["@text.note"]             = { fg = p.sage400, bold = true },
  ["@text.warning"]          = { fg = p.amber400, bold = true },
  ["@text.danger"]           = { fg = p.red400, bold = true },
  ["@text.diff.add"]         = { fg = p.sage500 },
  ["@text.diff.delete"]      = { fg = p.red400 },

  -- Tags
  ["@tag"]                   = { fg = p.red400 },
  ["@tag.attribute"]         = { fg = p.amber400 },
  ["@tag.delimiter"]         = { fg = p.beige500 },

  -- Markup (new treesitter captures)
  ["@markup.heading"]        = { fg = p.sage400, bold = true },
  ["@markup.italic"]         = { italic = true },
  ["@markup.strong"]         = { bold = true },
  ["@markup.raw"]            = { fg = p.amber400 },
  ["@markup.link"]           = { fg = p.sage400, underline = true },
  ["@markup.link.url"]       = { fg = p.amber400, underline = true },
  ["@markup.list"]           = { fg = p.beige500 },
}

local light = {
  -- Misc
  ["@comment"]               = { link = "Comment" },
  ["@error"]                 = { fg = p.red600 },
  ["@preproc"]               = { fg = p.sage700 },

  -- Punctuation
  ["@punctuation.delimiter"] = { fg = p.beige900 },
  ["@punctuation.bracket"]   = { fg = p.beige900 },
  ["@punctuation.special"]   = { fg = p.beige900 },

  -- Literals
  ["@string"]                = { fg = p.amber700 },
  ["@string.regex"]          = { fg = p.olive700 },
  ["@string.escape"]         = { fg = p.orange700 },
  ["@string.special"]        = { fg = p.orange700 },
  ["@character"]             = { fg = p.amber700 },
  ["@boolean"]               = { fg = p.blue700 },
  ["@number"]                = { fg = p.blue700 },
  ["@float"]                 = { fg = p.blue700 },

  -- Functions
  ["@function"]              = { fg = p.sage800 },
  ["@function.call"]         = { fg = p.sage800 },
  ["@function.builtin"]      = { fg = p.sage800 },
  ["@function.macro"]        = { fg = p.sage700 },
  ["@method"]                = { fg = p.sage800 },
  ["@method.call"]           = { fg = p.sage800 },
  ["@constructor"]           = { fg = p.teal800 },

  -- Keywords
  ["@keyword"]               = { fg = p.sage700 },
  ["@keyword.function"]      = { fg = p.sage700 },
  ["@keyword.operator"]      = { fg = p.sage700 },
  ["@keyword.return"]        = { fg = p.sage700 },
  ["@conditional"]           = { fg = p.sage700 },
  ["@repeat"]                = { fg = p.sage700 },
  ["@label"]                 = { fg = p.sage700 },
  ["@include"]               = { fg = p.sage700 },
  ["@exception"]             = { fg = p.sage700 },

  -- Types
  ["@type"]                  = { fg = p.teal800 },
  ["@type.builtin"]          = { fg = p.teal800 },
  ["@type.qualifier"]        = { fg = p.sage700 },
  ["@type.definition"]       = { fg = p.teal800 },
  ["@storageclass"]          = { fg = p.sage700 },
  ["@attribute"]             = { fg = p.amber700 },
  ["@field"]                 = { fg = p.beige950 },
  ["@property"]              = { fg = p.beige950 },

  -- Identifiers
  ["@variable"]              = { fg = p.beige950 },
  ["@variable.builtin"]      = { fg = p.blue700 },
  ["@constant"]              = { fg = p.blue700 },
  ["@constant.builtin"]      = { fg = p.blue700 },
  ["@constant.macro"]        = { fg = p.blue700 },
  ["@namespace"]             = { fg = p.teal800 },
  ["@module"]                = { fg = p.teal800 },
  ["@symbol"]                = { fg = p.sage700 },

  -- Text / markup
  ["@text"]                  = { fg = p.beige950 },
  ["@text.strong"]           = { bold = true },
  ["@text.emphasis"]         = { italic = true },
  ["@text.underline"]        = { underline = true },
  ["@text.strike"]           = { strikethrough = true },
  ["@text.title"]            = { fg = p.sage700, bold = true },
  ["@text.literal"]          = { fg = p.amber700 },
  ["@text.uri"]              = { fg = p.sage700, underline = true },
  ["@text.todo"]             = { fg = p.amber700, bold = true },
  ["@text.note"]             = { fg = p.sage700, bold = true },
  ["@text.warning"]          = { fg = p.amber700, bold = true },
  ["@text.danger"]           = { fg = p.red600, bold = true },
  ["@text.diff.add"]         = { fg = p.sage900 },
  ["@text.diff.delete"]      = { fg = p.red600 },

  -- Tags
  ["@tag"]                   = { fg = p.red600 },
  ["@tag.attribute"]         = { fg = p.amber700 },
  ["@tag.delimiter"]         = { fg = p.beige900 },

  -- Markup (new treesitter captures)
  ["@markup.heading"]        = { fg = p.sage700, bold = true },
  ["@markup.italic"]         = { italic = true },
  ["@markup.strong"]         = { bold = true },
  ["@markup.raw"]            = { fg = p.amber700 },
  ["@markup.link"]           = { fg = p.sage700, underline = true },
  ["@markup.link.url"]       = { fg = p.amber700, underline = true },
  ["@markup.list"]           = { fg = p.beige900 },
}

function M.highlights(mode)
  return mode == "light" and light or dark
end

return M
