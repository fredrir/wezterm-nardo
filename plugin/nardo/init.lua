local wezterm = require "wezterm" ---@type Wezterm
local backend = require "nardo.backend"
local config_mod = require "nardo.config"
local launcher = require "nardo.launcher"
local palette = require "nardo.palette"
local quick = require "nardo.quick"
local sessions = require "nardo.sessions"

local M = {
  version = require "nardo.version",
  id = require "nardo.id",
  config = config_mod,
  context = require "nardo.context",
  launcher = launcher,
  sessions = sessions,
  palette = palette,
  quick = quick,
  root = nil,
}

local MODULES = {
  "backend",
  "config",
  "context",
  "id",
  "init",
  "launcher",
  "palette",
  "platform",
  "present",
  "quick",
  "sessions",
  "util",
  "version",
}

---Edits to the plugin reload the config like edits to the user's own files do.
local function watch_plugin_files(root)
  if not root or not wezterm.add_to_config_reload_watch_list then
    return
  end
  wezterm.add_to_config_reload_watch_list(root .. "/init.lua")
  for _, name in ipairs(MODULES) do
    wezterm.add_to_config_reload_watch_list(root .. "/" .. M.id.ns .. "/" .. name .. ".lua")
  end
end

---@param config Config
---@param opts table|nil see docs/configuration.md
function M.apply_to_config(config, opts)
  local cfg = config_mod.setup(opts)
  backend.register_local_domains(config)
  launcher.register(config)
  sessions.apply(config, cfg)
  palette.apply(config, cfg)
  quick.apply(config, cfg)
  watch_plugin_files(M.root)
  return config
end

return M
