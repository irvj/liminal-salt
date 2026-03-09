-- Liminal Salt — Neovim colorscheme
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
