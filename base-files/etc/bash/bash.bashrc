PS1='\[\033[01;32m\]root@\h\[\033[00m\]:\[\033[01;36m\]\w\[\033[00m\]\$ '

HISTCONTROL=ignoredups
HISTSIZE=-1
HISTFILESIZE=-1

export TERM=xterm-256color

alias ls="ls --color=auto"
alias clear='printf "\e[2J\e[H"'

# todo: https://github.com/sharkdp/bat/blob/master/src/bin/bat/directories.rs panics if not set?
export BAT_CONFIG_DIR="/cfg/bat"
export BAT_CACHE_PATH="/cache/bat"
export DISPLAY=:0
