" Liminal Salt Dark
" https://github.com/irvj/liminal-salt

set background=dark
hi clear
if exists("syntax_on")
  syntax reset
endif
let g:colors_name = "liminal-salt-dark"

" Editor
hi Normal guifg=#e8e4dc guibg=#1a1c1b
hi CursorLine guifg=NONE guibg=#393a38 gui=NONE
hi CursorLineNr guifg=#c5c1b8 guibg=#393a38
hi LineNr guifg=#9e9b93 guibg=NONE
hi Visual guifg=NONE guibg=#3d4741
hi Search guifg=NONE guibg=#4f4633
hi IncSearch guifg=NONE guibg=#4f4633 gui=bold
hi MatchParen guifg=NONE guibg=#2e312f gui=bold
hi NonText guifg=#2e312f guibg=NONE
hi SpecialKey guifg=#2e312f guibg=NONE
hi Cursor guifg=#1a1c1b guibg=#8fac98
hi SignColumn guifg=#9e9b93 guibg=#1a1c1b
hi FoldColumn guifg=#9e9b93 guibg=#1a1c1b
hi Folded guifg=#9e9b93 guibg=#141615

" UI
hi StatusLine guifg=#e8e4dc guibg=#242726 gui=NONE
hi StatusLineNC guifg=#9e9b93 guibg=#242726 gui=NONE
hi VertSplit guifg=#2e312f guibg=#1a1c1b gui=NONE
hi WinSeparator guifg=#2e312f guibg=#1a1c1b gui=NONE
hi TabLine guifg=#9e9b93 guibg=#242726 gui=NONE
hi TabLineFill guifg=NONE guibg=#242726 gui=NONE
hi TabLineSel guifg=#e8e4dc guibg=#1a1c1b gui=bold
hi Pmenu guifg=#e8e4dc guibg=#242726
hi PmenuSel guifg=#e8e4dc guibg=#3d4741
hi PmenuSbar guifg=NONE guibg=#141615
hi PmenuThumb guifg=NONE guibg=#2e312f
hi FloatBorder guifg=#2e312f guibg=#242726
hi NormalFloat guifg=#e8e4dc guibg=#242726

" Diagnostics
hi DiagnosticError guifg=#cc8585 guibg=NONE
hi DiagnosticWarn guifg=#c9a86c guibg=NONE
hi DiagnosticInfo guifg=#8fac98 guibg=NONE
hi DiagnosticHint guifg=#8fac98 guibg=NONE
hi ErrorMsg guifg=#cc8585 guibg=NONE gui=bold
hi WarningMsg guifg=#c9a86c guibg=NONE

" Diff
hi DiffAdd guifg=NONE guibg=#384b3c
hi DiffDelete guifg=NONE guibg=#4f3c3b
hi DiffChange guifg=NONE guibg=#393a38
hi DiffText guifg=NONE guibg=#4f4633 gui=bold

" Syntax
hi Comment guifg=#9e9b93 guibg=NONE gui=italic
hi Constant guifg=#7eb8c9 guibg=NONE
hi String guifg=#c9a86c guibg=NONE
hi Number guifg=#7eb8c9 guibg=NONE
hi Float guifg=#7eb8c9 guibg=NONE
hi Boolean guifg=#7eb8c9 guibg=NONE
hi Identifier guifg=#e8e4dc guibg=NONE
hi Function guifg=#a3bfac guibg=NONE
hi Statement guifg=#8fac98 guibg=NONE
hi Keyword guifg=#8fac98 guibg=NONE
hi Conditional guifg=#8fac98 guibg=NONE
hi Repeat guifg=#8fac98 guibg=NONE
hi Operator guifg=#c5c1b8 guibg=NONE
hi PreProc guifg=#8fac98 guibg=NONE
hi Type guifg=#8fb8ad guibg=NONE
hi StorageClass guifg=#8fac98 guibg=NONE
hi Structure guifg=#8fb8ad guibg=NONE
hi Typedef guifg=#8fb8ad guibg=NONE
hi Special guifg=#c9956c guibg=NONE
hi SpecialChar guifg=#c9956c guibg=NONE
hi Tag guifg=#cc8585 guibg=NONE
hi Delimiter guifg=#c5c1b8 guibg=NONE
hi Todo guifg=#c9a86c guibg=NONE gui=bold
hi Underlined guifg=#8fac98 guibg=NONE gui=underline

" Treesitter
hi link @comment Comment
hi link @keyword Keyword
hi link @function Function
hi link @function.call Function
hi link @method Function
hi link @string String
hi link @number Number
hi link @float Float
hi link @boolean Boolean
hi link @type Type
hi link @type.builtin Type
hi link @variable Identifier
hi link @constant Constant
hi link @operator Operator
hi link @punctuation Delimiter
hi link @tag Tag
hi @tag.attribute guifg=#c9a86c guibg=NONE
hi @string.regex guifg=#a9ad78 guibg=NONE
hi @string.escape guifg=#c9956c guibg=NONE

" Git signs
hi GitSignsAdd guifg=#7dba8a guibg=NONE
hi GitSignsChange guifg=#c9a86c guibg=NONE
hi GitSignsDelete guifg=#cc8585 guibg=NONE

" Terminal colors
let g:terminal_ansi_colors = [
  \ '#141615', '#cc8585', '#7dba8a', '#c9a86c',
  \ '#7eb8c9', '#c9956c', '#95bebe', '#c5c1b8',
  \ '#6b7369', '#d99292', '#a3bfac', '#c9a86c',
  \ '#7eb8c9', '#c9956c', '#95bebe', '#e8e4dc'
  \ ]

" Neovim terminal colors
let g:terminal_color_0 = '#141615'
let g:terminal_color_1 = '#cc8585'
let g:terminal_color_2 = '#7dba8a'
let g:terminal_color_3 = '#c9a86c'
let g:terminal_color_4 = '#7eb8c9'
let g:terminal_color_5 = '#c9956c'
let g:terminal_color_6 = '#95bebe'
let g:terminal_color_7 = '#c5c1b8'
let g:terminal_color_8 = '#6b7369'
let g:terminal_color_9 = '#d99292'
let g:terminal_color_10 = '#a3bfac'
let g:terminal_color_11 = '#c9a86c'
let g:terminal_color_12 = '#7eb8c9'
let g:terminal_color_13 = '#c9956c'
let g:terminal_color_14 = '#95bebe'
let g:terminal_color_15 = '#e8e4dc'
