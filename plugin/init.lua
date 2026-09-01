local wezterm = require "wezterm" ---@type Wezterm

local function usable(root)
  return package.searchpath("nardo.config", root .. "/?.lua;" .. root .. "/?/init.lua") ~= nil
end

---Prefers the checkout the modules already resolve from, so a stale plugin clone never shadows it.
local function plugin_root()
  local found = package.searchpath("nardo.config", package.path)
  if found then
    return found:match "^(.*)[/\\]nardo[/\\]config%.lua$"
  end
  for _, p in ipairs(wezterm.plugin.list()) do
    local candidate = p.plugin_dir .. "/plugin"
    if p.url:find("nardo", 1, true) and usable(candidate) then
      return candidate
    end
  end
  return nil
end

local root = plugin_root()
if root then
  local search = root .. "/?.lua;" .. root .. "/?/init.lua;"
  if not package.path:find(search, 1, true) then
    package.path = search .. package.path
  end
else
  wezterm.log_warn "nardo: plugin root not found; add plugin dir to package.path"
end

local backend = require "nardo.backend"
backend.root = root

local M = require "nardo"
M.root = root

return M
