-- Liminal Salt — LSP and diagnostic highlights
local p = require("liminal-salt.palette").p

local M = {}

local dark = {
  -- Diagnostics
  DiagnosticError          = { fg = p.red400 },
  DiagnosticWarn           = { fg = p.amber400 },
  DiagnosticInfo           = { fg = p.sage400 },
  DiagnosticHint           = { fg = p.sage400 },
  DiagnosticOk             = { fg = p.sage500 },

  -- Virtual text
  DiagnosticVirtualTextError = { fg = p.red400 },
  DiagnosticVirtualTextWarn  = { fg = p.amber400 },
  DiagnosticVirtualTextInfo  = { fg = p.sage400 },
  DiagnosticVirtualTextHint  = { fg = p.sage400 },
  DiagnosticVirtualTextOk   = { fg = p.sage500 },

  -- Underline
  DiagnosticUnderlineError = { undercurl = true, sp = p.red400 },
  DiagnosticUnderlineWarn  = { undercurl = true, sp = p.amber400 },
  DiagnosticUnderlineInfo  = { undercurl = true, sp = p.sage400 },
  DiagnosticUnderlineHint  = { undercurl = true, sp = p.sage400 },
  DiagnosticUnderlineOk    = { undercurl = true, sp = p.sage500 },

  -- Sign
  DiagnosticSignError = { fg = p.red400 },
  DiagnosticSignWarn  = { fg = p.amber400 },
  DiagnosticSignInfo  = { fg = p.sage400 },
  DiagnosticSignHint  = { fg = p.sage400 },
  DiagnosticSignOk    = { fg = p.sage500 },

  -- LSP references
  LspReferenceText  = { bg = p.sageTint30 },
  LspReferenceRead  = { bg = p.sageTint30 },
  LspReferenceWrite = { bg = p.sageTint30 },

  -- LSP inlay hints
  LspInlayHint = { fg = p.beige600, italic = true },

  -- LSP code lens
  LspCodeLens          = { fg = p.beige600 },
  LspCodeLensSeparator = { fg = p.stone50 },

  -- LSP signature
  LspSignatureActiveParameter = { bg = p.sageTint30, bold = true },
}

local light = {
  -- Diagnostics
  DiagnosticError          = { fg = p.red600 },
  DiagnosticWarn           = { fg = p.amber700 },
  DiagnosticInfo           = { fg = p.sage700 },
  DiagnosticHint           = { fg = p.sage700 },
  DiagnosticOk             = { fg = p.sage900 },

  -- Virtual text
  DiagnosticVirtualTextError = { fg = p.red600 },
  DiagnosticVirtualTextWarn  = { fg = p.amber700 },
  DiagnosticVirtualTextInfo  = { fg = p.sage700 },
  DiagnosticVirtualTextHint  = { fg = p.sage700 },
  DiagnosticVirtualTextOk   = { fg = p.sage900 },

  -- Underline
  DiagnosticUnderlineError = { undercurl = true, sp = p.red600 },
  DiagnosticUnderlineWarn  = { undercurl = true, sp = p.amber700 },
  DiagnosticUnderlineInfo  = { undercurl = true, sp = p.sage700 },
  DiagnosticUnderlineHint  = { undercurl = true, sp = p.sage700 },
  DiagnosticUnderlineOk    = { undercurl = true, sp = p.sage900 },

  -- Sign
  DiagnosticSignError = { fg = p.red600 },
  DiagnosticSignWarn  = { fg = p.amber700 },
  DiagnosticSignInfo  = { fg = p.sage700 },
  DiagnosticSignHint  = { fg = p.sage700 },
  DiagnosticSignOk    = { fg = p.sage900 },

  -- LSP references
  LspReferenceText  = { bg = p.sageTint30L },
  LspReferenceRead  = { bg = p.sageTint30L },
  LspReferenceWrite = { bg = p.sageTint30L },

  -- LSP inlay hints
  LspInlayHint = { fg = p.beige800, italic = true },

  -- LSP code lens
  LspCodeLens          = { fg = p.beige800 },
  LspCodeLensSeparator = { fg = p.beige400 },

  -- LSP signature
  LspSignatureActiveParameter = { bg = p.sageTint30L, bold = true },
}

function M.highlights(mode)
  return mode == "light" and light or dark
end

return M
