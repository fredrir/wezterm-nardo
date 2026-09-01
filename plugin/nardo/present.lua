local wezterm = require "wezterm" ---@type Wezterm
local util = require "nardo.util"

local M = {}

-- The launcher always runs on the GUI host so `wezterm cli` sees every attached domain.
M.DOMAIN = { DomainName = "local" }
M.TAB_TITLE = " "
M.WINDOW_OVERRIDES = { enable_tab_bar = false, window_decorations = "RESIZE", window_background_opacity = 0.96 }

local GUI_WINDOW_ATTEMPTS = 20

local function cells(value, available, max)
  local n = value <= 1 and math.floor(available * value) or math.floor(value)
  return math.max(1, math.min(n, max))
end

---Size of the origin tab in cells and pixels; the fallback keeps the launcher usable without metrics.
local function area(window, pane)
  local tab = util.active_tab(window)
  local size = tab and util.try(tab.get_size, tab)
  if type(size) == "table" and size.cols and size.rows then
    return size.cols, size.rows, size.pixel_width, size.pixel_height
  end
  local dims = util.try(pane.get_dimensions, pane) or {}
  return dims.cols or 120, dims.viewport_rows or 40, dims.pixel_width, dims.pixel_height
end

local function centred(cols, rows, cell_w, cell_h)
  local screens = util.try(wezterm.gui.screens)
  local screen = type(screens) == "table" and screens.active
  if not screen or not cell_w or not cell_h then
    return nil
  end
  return {
    x = math.max(0, math.floor((screen.width - cols * cell_w) / 2)),
    y = math.max(0, math.floor((screen.height - rows * cell_h) / 2)),
    origin = "ActiveScreen",
  }
end

---`gui_window()` errors until the OS window exists, so retry briefly off the current callback.
function M.with_gui_window(mux_window, fn, attempts)
  attempts = attempts or GUI_WINDOW_ATTEMPTS
  local gui = util.try(mux_window.gui_window, mux_window)
  if gui then
    fn(gui)
    return
  end
  local call_after = wezterm.time and wezterm.time.call_after
  if attempts <= 0 or not call_after then
    return
  end
  call_after(0.05, function()
    M.with_gui_window(mux_window, fn, attempts - 1)
  end)
end

local function open_window(window, pane, spec)
  local pres = spec.presentation
  local cols, rows, px_w, px_h = area(window, pane)
  local width = cells(pres.width, cols, pres.max_width)
  local height = cells(pres.height, rows, pres.max_height)
  local cell_w = px_w and px_w > 0 and px_w / cols or nil
  local cell_h = px_h and px_h > 0 and px_h / rows or nil
  local tab, launcher, mux_window = wezterm.mux.spawn_window {
    args = spec.args,
    set_environment_variables = spec.env,
    domain = M.DOMAIN,
    width = width,
    height = height,
    position = centred(width, height, cell_w, cell_h),
  }
  M.with_gui_window(mux_window, function(gui)
    util.try(gui.set_config_overrides, gui, M.WINDOW_OVERRIDES)
    util.try(gui.focus, gui)
  end)
  return launcher, tab, mux_window
end

local function open_split(pane, spec)
  local launcher = pane:split {
    direction = spec.presentation.split.direction,
    size = spec.presentation.split.size,
    args = spec.args,
    set_environment_variables = spec.env,
    domain = M.DOMAIN,
  }
  return launcher, util.try(launcher.tab, launcher), nil
end

local function open_tab(window, spec)
  local mux_window = window:mux_window()
  local tab, launcher = mux_window:spawn_tab {
    args = spec.args,
    set_environment_variables = spec.env,
    domain = M.DOMAIN,
  }
  util.try(tab.set_title, tab, M.TAB_TITLE)
  return launcher, tab, mux_window
end

---Spawns the launcher per `spec.presentation.mode`; returns launcher pane, tab, mux window.
---@param window Window
---@param pane Pane
---@param spec { args: string[], env: table, presentation: table }
function M.open(window, pane, spec)
  local mode = spec.presentation.mode
  if mode == "window" then
    return open_window(window, pane, spec)
  end
  if mode == "split" then
    return open_split(pane, spec)
  end
  return open_tab(window, spec)
end

return M
