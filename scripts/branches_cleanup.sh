#!/bin/zsh

#
# removes all untracked branches
# NOTE: even unmerged ones - so be careful
#
git branch -vv | grep ': gone]'|  grep -v "\*" | awk '{ print $1; }' | xargs -r git branch -D
