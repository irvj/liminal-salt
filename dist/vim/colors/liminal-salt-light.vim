" Liminal Salt Light
" https://github.com/irvj/liminal-salt

set background=light
hi clear
if exists("syntax_on")
  syntax reset
endif
let g:colors_name = "liminal-salt-light"

" Editor
hi Normal guifg=#2d2b28 guibg=#f5f2ed
hi CursorLine guifg=NONE guibg=#d7d4cf gui=NONE
hi CursorLineNr guifg=#5a5753 guibg=#d7d4cf
hi LineNr guifg=#6b6762 guibg=NONE
hi Visual guifg=NONE guibg=#c4cac0
hi Search guifg=NONE guibg=#d1c7b1
hi IncSearch guifg=NONE guibg=#d1c7b1 gui=bold
hi MatchParen guifg=NONE guibg=#ddd8d0 gui=bold
hi NonText guifg=#ddd8d0 guibg=NONE
hi SpecialKey guifg=#ddd8d0 guibg=NONE
hi Cursor guifg=#f5f2ed guibg=#506e58
hi SignColumn guifg=#6b6762 guibg=#f5f2ed
hi FoldColumn guifg=#6b6762 guibg=#f5f2ed
hi Folded guifg=#6b6762 guibg=#ebe7e0

" UI
hi StatusLine guifg=#2d2b28 guibg=#fdfcfa gui=NONE
hi StatusLineNC guifg=#6b6762 guibg=#fdfcfa gui=NONE
hi VertSplit guifg=#ddd8d0 guibg=#f5f2ed gui=NONE
hi WinSeparator guifg=#ddd8d0 guibg=#f5f2ed gui=NONE
hi TabLine guifg=#6b6762 guibg=#fdfcfa gui=NONE
hi TabLineFill guifg=NONE guibg=#fdfcfa gui=NONE
hi TabLineSel guifg=#2d2b28 guibg=#f5f2ed gui=bold
hi Pmenu guifg=#2d2b28 guibg=#fdfcfa
hi PmenuSel guifg=#2d2b28 guibg=#c4cac0
hi PmenuSbar guifg=NONE guibg=#ebe7e0
hi PmenuThumb guifg=NONE guibg=#ddd8d0
hi FloatBorder guifg=#ddd8d0 guibg=#fdfcfa
hi NormalFloat guifg=#2d2b28 guibg=#fdfcfa

" Diagnostics
hi DiagnosticError guifg=#a54d4d guibg=NONE
hi DiagnosticWarn guifg=#7d6325 guibg=NONE
hi DiagnosticInfo guifg=#506e58 guibg=NONE
hi DiagnosticHint guifg=#506e58 guibg=NONE
hi ErrorMsg guifg=#a54d4d guibg=NONE gui=bold
hi WarningMsg guifg=#7d6325 guibg=NONE

" Diff
hi DiffAdd guifg=NONE guibg=#bdccbb
hi DiffDelete guifg=NONE guibg=#ddc1bd
hi DiffChange guifg=NONE guibg=#d7d4cf
hi DiffText guifg=NONE guibg=#d1c7b1 gui=bold

" Syntax
hi Comment guifg=#6b6762 guibg=NONE gui=italic
hi Constant guifg=#3a6f80 guibg=NONE
hi String guifg=#7d6325 guibg=NONE
hi Number guifg=#3a6f80 guibg=NONE
hi Float guifg=#3a6f80 guibg=NONE
hi Boolean guifg=#3a6f80 guibg=NONE
hi Identifier guifg=#2d2b28 guibg=NONE
hi Function guifg=#425f4a guibg=NONE
hi Statement guifg=#506e58 guibg=NONE
hi Keyword guifg=#506e58 guibg=NONE
hi Conditional guifg=#506e58 guibg=NONE
hi Repeat guifg=#506e58 guibg=NONE
hi Operator guifg=#5a5753 guibg=NONE
hi PreProc guifg=#506e58 guibg=NONE
hi Type guifg=#3e6b5d guibg=NONE
hi StorageClass guifg=#506e58 guibg=NONE
hi Structure guifg=#3e6b5d guibg=NONE
hi Typedef guifg=#3e6b5d guibg=NONE
hi Special guifg=#8a5c30 guibg=NONE
hi SpecialChar guifg=#8a5c30 guibg=NONE
hi Tag guifg=#a54d4d guibg=NONE
hi Delimiter guifg=#5a5753 guibg=NONE
hi Todo guifg=#7d6325 guibg=NONE gui=bold
hi Underlined guifg=#506e58 guibg=NONE gui=underline

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
hi @tag.attribute guifg=#7d6325 guibg=NONE
hi @string.regex guifg=#5c6135 guibg=NONE
hi @string.escape guifg=#8a5c30 guibg=NONE

" Git signs
hi GitSignsAdd guifg=#3a7346 guibg=NONE
hi GitSignsChange guifg=#7d6325 guibg=NONE
hi GitSignsDelete guifg=#a54d4d guibg=NONE

" Terminal colors
let g:terminal_ansi_colors = [
  \ '#2d2b28', '#a54d4d', '#3a7346', '#7d6325',
  \ '#3a6f80', '#8a5c30', '#3e5d5d', '#ebe7e0',
  \ '#6b6762', '#984242', '#425f4a', '#7d6325',
  \ '#3a6f80', '#8a5c30', '#3e5d5d', '#f5f2ed'
  \ ]

" Neovim terminal colors
let g:terminal_color_0 = '#2d2b28'
let g:terminal_color_1 = '#a54d4d'
let g:terminal_color_2 = '#3a7346'
let g:terminal_color_3 = '#7d6325'
let g:terminal_color_4 = '#3a6f80'
let g:terminal_color_5 = '#8a5c30'
let g:terminal_color_6 = '#3e5d5d'
let g:terminal_color_7 = '#ebe7e0'
let g:terminal_color_8 = '#6b6762'
let g:terminal_color_9 = '#984242'
let g:terminal_color_10 = '#425f4a'
let g:terminal_color_11 = '#7d6325'
let g:terminal_color_12 = '#3a6f80'
let g:terminal_color_13 = '#8a5c30'
let g:terminal_color_14 = '#3e5d5d'
let g:terminal_color_15 = '#f5f2ed'
