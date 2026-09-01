-- Plugin identity. Must agree with plugin.conf; CI asserts it.
local ns = "nardo"
local repo = "fredrir/wezterm-nardo"

return {
  ns = ns,
  repo = repo,
  name = "wez-" .. ns,
  prefix = (ns:upper():gsub("%-", "_")),
  url = "https://github.com/" .. repo,
}
