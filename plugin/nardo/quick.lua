local wezterm = require "wezterm" ---@type Wezterm
local config = require "nardo.config"
local id = require "nardo.id"
local launcher = require "nardo.launcher"
local present = require "nardo.present"
local util = require "nardo.util"

local M = {}

M.WINDOW_OVERRIDES = { enable_tab_bar = false, window_decorations = "RESIZE" }

local WINDOW_KEY = id.ns .. "_quick"
local PREVIOUS_KEY = id.ns .. "_quick_prev"

local function quick_window()
  local window_id = wezterm.GLOBAL[WINDOW_KEY]
  local mux_window = window_id and util.try(wezterm.mux.get_window, window_id)
  if not mux_window then
    wezterm.GLOBAL[WINDOW_KEY] = nil
  end
  return mux_window
end

local function focus_previous()
  local mux_window = util.try(wezterm.mux.get_window, wezterm.GLOBAL[PREVIOUS_KEY])
  local gui = mux_window and util.try(mux_window.gui_window, mux_window)
  if gui then
    util.try(gui.focus, gui)
  end
end

---Cell metrics come from the caller's window; the dropdown spans the active screen's width.
local function size(window, pane, cfg)
  local tab = util.active_tab(window)
  local dims = (tab and util.try(tab.get_size, tab)) or util.try(pane.get_dimensions, pane) or {}
  local cols, rows = dims.cols or 120, dims.rows or dims.viewport_rows or 40
  local screens = util.try(wezterm.gui.screens)
  local screen = type(screens) == "table" and screens.active
  if not screen or not dims.pixel_width or dims.pixel_width == 0 or not dims.pixel_height or dims.pixel_height == 0 then
    return cols, math.max(5, math.floor(rows * cfg.quick.height))
  end
  local cell_w, cell_h = dims.pixel_width / cols, dims.pixel_height / rows
  return math.max(20, math.floor(screen.width / cell_w)),
    math.max(5, math.floor(screen.height * cfg.quick.height / cell_h))
end

local function spawn(window, pane, cfg)
  local width, height = size(window, pane, cfg)
  local _, _, mux_window = wezterm.mux.spawn_window {
    args = cfg.quick.args,
    domain = "DefaultDomain",
    width = width,
    height = height,
    position = { x = 0, y = 0, origin = "ActiveScreen" },
  }
  wezterm.GLOBAL[WINDOW_KEY] = util.try(mux_window.window_id, mux_window)
  present.with_gui_window(mux_window, function(gui)
    util.try(gui.set_config_overrides, gui, M.WINDOW_OVERRIDES)
    util.try(gui.focus, gui)
  end)
  return mux_window
end

---Shows the dropdown, or hands focus back to the window it was called from when it already has it.
---@param window Window
---@param pane Pane
function M.toggle(window, pane)
  local cfg = config.get()
  local existing = quick_window()
  local gui = existing and util.try(existing.gui_window, existing)
  if gui and util.try(gui.is_focused, gui) then
    focus_previous()
    return existing
  end
  wezterm.GLOBAL[PREVIOUS_KEY] = window:window_id()
  if gui then
    util.try(gui.focus, gui)
    return existing
  end
  return spawn(window, pane, cfg)
end

function M.apply(config_table, cfg)
  if not cfg.quick.enabled then
    return
  end
  launcher.bind(config_table, cfg.quick.key, "t", M.toggle)
end

return M
