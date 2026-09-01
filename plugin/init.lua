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

local config_mod = require "nardo.config"
local backend = require "nardo.backend"
local util = require "nardo.util"

backend.root = root

local M = {}

M.version = require "nardo.version"
M.id = require "nardo.id"

local function alive(window)
  return pcall(function()
    return window:mux_window()
  end)
end

-- WezTerm reports closed windows with this text; there is no structured error to match.
local function window_gone(err)
  return tostring(err):find("not found in mux", 1, true) ~= nil
end

---Wraps a handler so a dead window is skipped and a thrown error warns instead of breaking WezTerm.
local function guarded(name, fn)
  return function(window, ...)
    if not alive(window) then
      return
    end
    local ok, err = pcall(fn, window, ...)
    if not ok and not window_gone(err) then
      util.warn("%s: %s", name, tostring(err))
    end
  end
end

local registered = false

local function register_events(cfg)
  if registered then
    return
  end
  registered = true

  local last_poll = {}
  local min_gap = math.max(50, math.floor(cfg.poll_ms / 4))
  wezterm.on(
    "update-status",
    guarded("update-status", function(window)
      local wid = window:window_id()
      local now = util.now_ms()
      if last_poll[wid] and now - last_poll[wid] < min_gap then
        return
      end
      last_poll[wid] = now
      M.tick(window)
    end)
  )

  wezterm.on(
    "window-config-reloaded",
    guarded("window-config-reloaded", function(window)
      M.tick(window)
    end)
  )
end

---Called on every throttled status update. Replace with your plugin's work.
function M.tick(_window) end

local MODULES = {
  "backend",
  "config",
  "id",
  "platform",
  "util",
  "version",
}

---Edits to the plugin reload the config like edits to the user's own files do.
local function watch_plugin_files()
  if not root or not wezterm.add_to_config_reload_watch_list then
    return
  end
  wezterm.add_to_config_reload_watch_list(root .. "/init.lua")
  for _, name in ipairs(MODULES) do
    wezterm.add_to_config_reload_watch_list(root .. "/nardo/" .. name .. ".lua")
  end
end

---@param config Config
---@param opts table|nil
function M.apply_to_config(config, opts)
  local cfg = config_mod.setup(opts)
  config.status_update_interval = math.min(config.status_update_interval or 1000, cfg.poll_ms)
  backend.register_local_domains(config)
  watch_plugin_files()
  register_events(cfg)
  return config
end

return M
