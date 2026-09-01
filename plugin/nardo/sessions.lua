local config = require "nardo.config"
local launcher = require "nardo.launcher"
local util = require "nardo.util"

local M = {}

M.APP = "sessions"

local OPTION_KEYS = { "confirm_kill", "preview", "preview_lines", "mru", "scope", "show_self" }

---App options for the context json; an empty `keys` table is left out so it never encodes as a list.
function M.options(cfg, overrides)
  local merged = util.merge(cfg.sessions, overrides or {})
  local options = {}
  for _, key in ipairs(OPTION_KEYS) do
    options[key] = merged[key]
  end
  if type(merged.keys) == "table" and next(merged.keys) ~= nil then
    options.keys = merged.keys
  end
  return options
end

---@param window Window
---@param pane Pane
---@param overrides table|nil sessions options plus an optional `presentation` table
function M.open(window, pane, overrides)
  overrides = overrides or {}
  return launcher.open(window, pane, {
    app = M.APP,
    options = M.options(config.get(), overrides),
    presentation = overrides.presentation,
  })
end

function M.apply(config_table, cfg)
  if not cfg.sessions.enabled then
    return
  end
  launcher.bind(config_table, cfg.sessions.key, "k", M.open)
end

return M
