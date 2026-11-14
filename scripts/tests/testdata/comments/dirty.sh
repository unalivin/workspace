#!/usr/bin/env bash
# explain
echo hi # trailing
cat <<TEXT
body # not flagged
TEXT
# after here-document
