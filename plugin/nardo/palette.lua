local wezterm = require "wezterm" ---@type Wezterm
local config = require "nardo.config"
local launcher = require "nardo.launcher"
local util = require "nardo.util"

local M = {}

M.APP = "palette"

local act = wezterm.action

M.BUILTIN = {
  { label = "Reload configuration", action = act.ReloadConfiguration },
  { label = "Show debug overlay", action = act.ShowDebugOverlay },
  { label = "Toggle full screen", action = act.ToggleFullScreen },
  { label = "WezTerm command palette", action = act.ActivateCommandPalette },
  { label = "WezTerm launcher menu", action = act.ShowLauncher },
  { label = "New tab", hint = "current domain", action = act.SpawnTab "CurrentPaneDomain" },
  { label = "Close pane", hint = "confirm", action = act.CloseCurrentPane { confirm = true } },
  { label = "Detach domain", hint = "current domain", action = act.DetachDomain "CurrentPaneDomain" },
}

local current = {}

local function valid(entry)
  return type(entry) == "table" and type(entry.label) == "string" and entry.action ~= nil
end

---User commands first, built-ins after; ids are positional (`p1`, `p2`, ...).
function M.commands(cfg)
  local list = {}
  for i, entry in ipairs(cfg.palette.commands or {}) do
    if valid(entry) then
      list[#list + 1] = entry
    else
      util.warn_once("palette.commands." .. i, "palette.commands[%d] needs label and action, ignored", i)
    end
  end
  for _, entry in ipairs(M.BUILTIN) do
    list[#list + 1] = entry
  end
  for i, entry in ipairs(list) do
    entry.id = "p" .. i
  end
  return list
end

local function describe(list)
  local out = {}
  for i, entry in ipairs(list) do
    out[i] = { id = entry.id, label = entry.label, hint = entry.hint }
  end
  return out
end

function M.run(window, pane, entry)
  if type(entry.action) == "function" then
    entry.action(window, pane)
  else
    window:perform_action(entry.action, pane)
  end
end

local function register(list)
  current = list
  for _, entry in ipairs(list) do
    launcher.on_action(entry.id, function(window, pane)
      for _, candidate in ipairs(current) do
        if candidate.id == entry.id then
          M.run(window, pane, candidate)
          return
        end
      end
    end)
  end
end

---@param window Window
---@param pane Pane
---@param overrides { presentation: table|nil }|nil
function M.open(window, pane, overrides)
  overrides = overrides or {}
  local list = M.commands(config.get())
  register(list)
  return launcher.open(window, pane, {
    app = M.APP,
    options = { commands = describe(list) },
    presentation = overrides.presentation,
  })
end

function M.apply(config_table, cfg)
  if not cfg.palette.enabled then
    return
  end
  register(M.commands(cfg))
  launcher.bind(config_table, cfg.palette.key, "k", M.open)
end

return M
