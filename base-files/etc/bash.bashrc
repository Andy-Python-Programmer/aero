# If not running interactively, don't do anything
[[ $- != *i* ]] && return

alias ls='ls --color=auto'
alias grep='grep --color=auto'

PS1='\[\033[01;32m\]root@\h\[\033[00m\]:\[\033[01;36m\]\w\[\033[00m\]\$ '

HISTCONTROL=ignoredups
HISTSIZE=-1
HISTFILESIZE=-1

export DISPLAY=:0
