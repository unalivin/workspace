#!/usr/bin/env bash
set -euo pipefail
count="${#items[@]}"
args=$#
echo "# not a comment"
echo '# also not'
value="${name#prefix}"
case "$x" in *#*) ;; esac
cat <<TEXT
# inside here-document
TEXT
cat <<-'INDENTED'
	# inside indented
	INDENTED
echo "it's" \# escaped
echo $'tab\t\'# quoted'
