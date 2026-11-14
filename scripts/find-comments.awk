function report(kind) {
  printf "%s:%d: %s\n", FILENAME, FNR, kind
  found = 1
}

function repeat(text, count, out, k) {
  out = ""
  for (k = 0; k < count; k++) {
    out = out text
  }
  return out
}

function is_word_char(ch) {
  return ch ~ /[A-Za-z0-9_]/
}

function scan_c(line, n, i, c, d, prev, rest, p, hashes) {
  i = 1
  while (i <= n) {
    c = substr(line, i, 1)
    d = substr(line, i, 2)
    if (state == "block") {
      if (d == "*/") {
        state = "code"
        i += 2
      } else {
        i++
      }
      continue
    }
    if (state == "str") {
      if (c == "\\") {
        i += 2
        continue
      }
      if (c == "\"") {
        state = "code"
      }
      i++
      continue
    }
    if (state == "raw") {
      if (substr(line, i, length(rawend)) == rawend) {
        i += length(rawend)
        state = "code"
      } else {
        i++
      }
      continue
    }
    if (d == "//") {
      if (!(lang == "go" && i == 1 && substr(line, 1, 5) == "//go:")) {
        report("line comment")
      }
      return
    }
    if (d == "/*") {
      report("block comment")
      state = "block"
      i += 2
      continue
    }
    prev = i > 1 ? substr(line, i - 1, 1) : ""
    if (lang == "go" && c == "`") {
      state = "raw"
      rawend = "`"
      i++
      continue
    }
    if (lang == "rust" && c == "r" && (prev == "b" || !is_word_char(prev)) && match(substr(line, i), /^r#*"/)) {
      hashes = RLENGTH - 2
      rawend = "\"" repeat("#", hashes)
      state = "raw"
      i += RLENGTH
      continue
    }
    if (lang == "cpp" && c == "R" && !is_word_char(prev) && substr(line, i + 1, 1) == "\"") {
      rest = substr(line, i + 2)
      p = index(rest, "(")
      if (p > 0) {
        rawend = ")" substr(rest, 1, p - 1) "\""
        state = "raw"
        i += 2 + p
        continue
      }
    }
    if (c == "\"") {
      state = "str"
      i++
      continue
    }
    if (c == "'") {
      if (match(substr(line, i), /^'(\\.[^']*|[^\\'])'/)) {
        i += RLENGTH
        continue
      }
    }
    i++
  }
  if (state == "str" && lang != "rust" && substr(line, n, 1) != "\\") {
    state = "code"
  }
}

function scan_sql(line, n, i, c, d) {
  i = 1
  while (i <= n) {
    c = substr(line, i, 1)
    d = substr(line, i, 2)
    if (state == "block") {
      if (d == "*/") {
        state = "code"
        i += 2
      } else {
        i++
      }
      continue
    }
    if (state == "sq") {
      if (c == "'") {
        state = "code"
      }
      i++
      continue
    }
    if (state == "dq") {
      if (c == "\"") {
        state = "code"
      }
      i++
      continue
    }
    if (d == "--") {
      report("line comment")
      return
    }
    if (d == "/*") {
      report("block comment")
      state = "block"
      i += 2
      continue
    }
    if (c == "'") {
      state = "sq"
    } else if (c == "\"") {
      state = "dq"
    }
    i++
  }
}

function scan_sh(line, n, i, c, prev, rest, word, text) {
  if (FNR == 1 && substr(line, 1, 2) == "#!") {
    return
  }
  if (heredoc != "") {
    text = line
    if (heredoc_tabs) {
      sub(/^\t+/, "", text)
    }
    if (text == heredoc) {
      heredoc = ""
    }
    return
  }
  i = 1
  while (i <= n) {
    c = substr(line, i, 1)
    if (state == "sq") {
      if (c == "'") {
        state = "code"
      }
      i++
      continue
    }
    if (state == "ansi") {
      if (c == "\\") {
        i += 2
        continue
      }
      if (c == "'") {
        state = "code"
      }
      i++
      continue
    }
    if (state == "dq") {
      if (c == "\\") {
        i += 2
        continue
      }
      if (c == "\"") {
        state = "code"
      }
      i++
      continue
    }
    prev = i > 1 ? substr(line, i - 1, 1) : ""
    if (c == "\\") {
      i += 2
      continue
    }
    if (c == "'") {
      state = prev == "$" ? "ansi" : "sq"
      i++
      continue
    }
    if (c == "\"") {
      state = "dq"
      i++
      continue
    }
    if (c == "#" && (i == 1 || prev ~ /[ \t;(|&]/)) {
      report("comment")
      return
    }
    if (substr(line, i, 2) == "<<" && substr(line, i, 3) != "<<<") {
      rest = substr(line, i + 2)
      pending_tabs = 0
      if (substr(rest, 1, 1) == "-") {
        pending_tabs = 1
        rest = substr(rest, 2)
      }
      sub(/^[ \t]+/, "", rest)
      if (match(rest, /^['"]?[A-Za-z_][A-Za-z_0-9]*['"]?/)) {
        word = substr(rest, 1, RLENGTH)
        gsub(/['"]/, "", word)
        pending = word
      }
      i += 2
      continue
    }
    i++
  }
  if (pending != "") {
    heredoc = pending
    heredoc_tabs = pending_tabs
    pending = ""
  }
}

FNR == 1 {
  state = "code"
  rawend = ""
  heredoc = ""
  pending = ""
}

{
  sub(/\r$/, "", $0)
  if (lang == "sh") {
    scan_sh($0, length($0))
  } else if (lang == "sql") {
    scan_sql($0, length($0))
  } else {
    scan_c($0, length($0))
  }
}

END {
  exit found ? 1 : 0
}
