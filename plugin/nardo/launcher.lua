local wezterm = require "wezterm" ---@type Wezterm
local backend = require "nardo.backend"
local config = require "nardo.config"
local context = require "nardo.context"
local id = require "nardo.id"
local platform = require "nardo.platform"
local present = require "nardo.present"
local util = require "nardo.util"

local M = {}

M.FORWARD = utf8.char(0xE000)
M.ROLE = "launcher"
M.STALE_SECONDS = 3600

local handlers = {}
local ACTIONS = {}
local registered = false

---Registers `fn(window, pane, args)` for `run` actions named `name`; `window`/`pane` are the origin.
function M.on_action(name, fn)
  handlers[name] = fn
end

local function global_table(key)
  if wezterm.GLOBAL[key] == nil then
    wezterm.GLOBAL[key] = {}
  end
  return wezterm.GLOBAL[key]
end

local function launchers()
  return global_table(id.ns .. "_launchers")
end

local function sessions()
  return global_table(id.ns .. "_sessions")
end

function M.temp_dir()
  local dir = os.getenv "TMPDIR" or os.getenv "TEMP" or os.getenv "TMP"
  if dir and dir ~= "" then
    return (dir:gsub("[/\\]+$", ""))
  end
  return platform.is_windows and "C:\\Windows\\Temp" or "/tmp"
end

local function context_pattern(dir)
  return dir .. "/" .. id.ns .. "-*.json"
end

---Context files carry their creation time in the name, so a sweep needs no stat.
local function sweep_stale(dir)
  local now = os.time()
  for _, path in ipairs(util.try(wezterm.glob, context_pattern(dir)) or {}) do
    local created = tonumber(path:match(id.ns .. "%-(%d+)%-[%x]+%.json$"))
    if created and now - created > M.STALE_SECONDS then
      os.remove(path)
    end
  end
end

local function write_context(ctx)
  local dir = M.temp_dir()
  sweep_stale(dir)
  local path = string.format("%s/%s-%d-%s.json", dir, id.ns, os.time(), util.random_token())
  local file, err = io.open(path, "w")
  if not file then
    return nil, err
  end
  file:write(wezterm.json_encode(ctx))
  file:close()
  return path
end

local function wezterm_exe()
  local exe = (wezterm.executable_dir or "") .. "/wezterm"
  if platform.is_windows then
    exe = exe .. ".exe"
  end
  return exe
end

---Backend env plus what the launcher needs to find its context and the GUI mux.
function M.env(cfg, context_path)
  local env = backend.env(cfg, "local")
  env[id.prefix .. "_CONTEXT"] = context_path
  env[id.prefix .. "_WEZTERM"] = wezterm_exe()
  env.WEZPLUG_USERVAR = cfg.backend.uservar
  -- The GUI exports its own socket; an empty value makes `wezterm cli` discover the GUI instead.
  env.WEZTERM_UNIX_SOCKET = os.getenv "WEZTERM_UNIX_SOCKET" or ""
  if cfg.backend.class then
    env[id.prefix .. "_WEZTERM_CLASS"] = cfg.backend.class
  end
  return env
end

local function notify(window, message)
  util.warn("%s", message)
  util.try(window.toast_notification, window, id.ns, message, nil, 4000)
end

local function pane_id_of(pane)
  return util.try(pane.pane_id, pane)
end

local function mux_window_id(mux_window)
  return mux_window and util.try(mux_window.window_id, mux_window) or nil
end

---The launcher pane already open for this window, if it is still alive.
function M.find_launcher(window)
  local key = tostring(window:window_id())
  local launcher_id = launchers()[key]
  if not launcher_id then
    return nil
  end
  local pane = util.try(wezterm.mux.get_pane, launcher_id)
  if not pane then
    launchers()[key] = nil
    sessions()[tostring(launcher_id)] = nil
  end
  return pane
end

local function record(window, pane, launcher, mux_window, path, mode)
  local origin_window = window:window_id()
  local launcher_id = pane_id_of(launcher)
  launchers()[tostring(origin_window)] = launcher_id
  sessions()[tostring(launcher_id)] = {
    origin_window = origin_window,
    origin_pane = pane_id_of(pane),
    context = path,
    mode = mode,
    window = mode == "window" and mux_window_id(mux_window) or nil,
  }
end

---Snapshots the mux, writes the context file and spawns `wez-nardo <app>` in the GUI's local domain.
---@param window Window
---@param pane Pane
---@param spec { app: string, options: table|nil, presentation: table|nil }
---@return Pane|nil launcher
function M.open(window, pane, spec)
  local cfg = config.get()
  local existing = M.find_launcher(window)
  if existing then
    util.try(existing.activate, existing)
    return existing
  end
  local args = util.try(backend.spawn_args, cfg, "local")
  if not args then
    notify(window, "backend not found")
    return nil
  end
  local presentation = util.merge(cfg.presentation, spec.presentation or {})
  local ctx = context.snapshot(window, pane, spec.app, { options = spec.options, presentation = presentation })
  local path, err = write_context(ctx)
  if not path then
    notify(window, "context file: " .. tostring(err))
    return nil
  end
  local ok, launcher, _, mux_window = pcall(present.open, window, pane, {
    args = args,
    env = M.env(cfg, path),
    presentation = presentation,
  })
  if not ok or not launcher then
    os.remove(path)
    notify(window, "spawn failed: " .. tostring(launcher))
    return nil
  end
  record(window, pane, launcher, mux_window, path, presentation.mode)
  util.debug("opened %s in pane %s (%s)", spec.app, tostring(pane_id_of(launcher)), presentation.mode)
  return launcher
end

local function origin_pane(session)
  return session and util.try(wezterm.mux.get_pane, session.origin_pane) or nil
end

local function origin_mux_window(session)
  return session and util.try(wezterm.mux.get_window, session.origin_window) or nil
end

local function origin_gui_window(session, fallback)
  local mux_window = origin_mux_window(session)
  return mux_window and util.try(mux_window.gui_window, mux_window) or fallback
end

local function focus_pane(pane)
  util.try(pane.activate, pane)
  local mux_window = util.try(pane.window, pane)
  local gui = mux_window and util.try(mux_window.gui_window, mux_window)
  if gui then
    util.try(gui.focus, gui)
  end
end

local function first_pane_in(domain_name)
  for _, window in ipairs(util.try(wezterm.mux.all_windows) or {}) do
    for _, tab in ipairs(util.try(window.tabs, window) or {}) do
      for _, pane in ipairs(util.try(tab.panes, tab) or {}) do
        if util.try(pane.get_domain_name, pane) == domain_name then
          return pane
        end
      end
    end
  end
  return nil
end

local function domain_named(name)
  local domain = util.try(wezterm.mux.get_domain, name)
  if not domain then
    error("unknown domain " .. tostring(name), 0)
  end
  return domain
end

---Mirrors WezTerm's `AttachDomain`: attach into the origin window, then show or spawn a pane.
function ACTIONS.attach_domain(window, pane, action, session)
  local domain = domain_named(action.domain)
  local ok, err = pcall(domain.attach, domain, origin_mux_window(session))
  if not ok then
    error(tostring(err), 0)
  end
  if util.try(domain.has_any_panes, domain) then
    local first = first_pane_in(action.domain)
    if first then
      focus_pane(first)
    end
    return
  end
  local gui = origin_gui_window(session, window)
  gui:perform_action(wezterm.action.SpawnCommandInNewTab { domain = { DomainName = action.domain } }, pane)
end

function ACTIONS.detach_domain(_, _, action)
  local domain = domain_named(action.domain)
  local ok, err = pcall(domain.detach, domain)
  if not ok then
    error(tostring(err), 0)
  end
end

function ACTIONS.focus(_, _, action)
  local ok, target = pcall(wezterm.mux.get_pane, action.pane_id)
  if not ok or not target then
    error("pane " .. tostring(action.pane_id) .. " not found", 0)
  end
  focus_pane(target)
end

---Handlers act on the origin, so the origin pane is made active before they run.
function ACTIONS.run(window, pane, action, session)
  local fn = handlers[action.name]
  if not fn then
    error("no handler for " .. tostring(action.name), 0)
  end
  local origin = origin_pane(session)
  if origin then
    util.try(origin.activate, origin)
  end
  fn(origin_gui_window(session, window), origin or pane, action.args)
end

local function restore_origin(session)
  local origin = origin_pane(session)
  if origin then
    focus_pane(origin)
  end
end

function ACTIONS.done(window, pane, action, session)
  if not session then
    return
  end
  os.remove(session.context)
  launchers()[tostring(session.origin_window)] = nil
  sessions()[tostring(pane_id_of(pane))] = nil
  if session.mode == "window" then
    util.try(window.perform_action, window, wezterm.action.CloseCurrentTab { confirm = false }, pane)
  end
  if action.exit == "cancelled" then
    restore_origin(session)
  end
end

function ACTIONS.error(window, _, action)
  local message = tostring(action.message or "unknown error")
  wezterm.log_error(id.ns .. ": " .. message)
  util.try(window.toast_notification, window, id.ns, message, nil, 4000)
end

---Runs the built-in handler for `action.t`, then the user's `hooks[t]` if any.
function M.dispatch(window, pane, action)
  local cfg = config.get()
  local session = sessions()[tostring(pane_id_of(pane))]
  local handler = ACTIONS[action.t]
  if handler then
    local ok, err = pcall(handler, window, pane, action, session)
    if not ok then
      notify(window, action.t .. ": " .. tostring(err))
    end
  else
    util.warn("unknown action %s", tostring(action.t))
  end
  local hook = cfg.hooks[action.t]
  if hook then
    local ok, err = pcall(hook, window, pane, action)
    if not ok then
      util.warn_once("hook " .. action.t, "hook %s: %s", action.t, tostring(err))
    end
  end
end

local function role_var(cfg)
  return cfg.backend.uservar .. "_role"
end

---Launchers found this way (after a config reload) still get the double-open guard.
local function note_launcher(window, pane)
  local launcher_id = pane_id_of(pane)
  if launcher_id and not sessions()[tostring(launcher_id)] then
    launchers()[tostring(window:window_id())] = launcher_id
  end
end

local function on_user_var(window, pane, name, value)
  local cfg = config.get()
  if name == role_var(cfg) or name == id.ns .. "_role" then
    if value == M.ROLE then
      note_launcher(window, pane)
    end
    return
  end
  if name ~= cfg.backend.uservar then
    return
  end
  local ok, action = pcall(wezterm.json_parse, value)
  if not ok or type(action) ~= "table" or type(action.t) ~= "string" then
    util.warn("bad action payload: %s", tostring(value))
    return
  end
  util.debug("action %s from pane %s", action.t, tostring(pane_id_of(pane)))
  M.dispatch(window, pane, action)
end

function M.is_launcher(pane)
  local vars = util.try(pane.get_user_vars, pane)
  if type(vars) ~= "table" then
    return false
  end
  return vars[role_var(config.get())] == M.ROLE or vars[id.ns .. "_role"] == M.ROLE
end

local MOD_ALIASES = { CMD = "SUPER", WIN = "SUPER", OPT = "ALT", META = "ALT" }

local function chord(spec)
  local mods = {}
  for mod in tostring(spec.mods or ""):upper():gmatch "[^|%s]+" do
    mods[#mods + 1] = MOD_ALIASES[mod] or mod
  end
  table.sort(mods)
  return tostring(spec.key) .. "+" .. table.concat(mods, "|")
end

local function shadowed_binding(keys, spec)
  local wanted = chord(spec)
  for _, binding in ipairs(keys) do
    if type(binding) == "table" and binding.key and chord(binding) == wanted then
      return binding
    end
  end
  return nil
end

---Appends a binding that forwards `U+E000 .. char` to a focused launcher and otherwise runs
---`fallback(window, pane)` or the user's own binding for the same chord.
function M.bind(config_table, spec, char, fallback)
  if not spec then
    return
  end
  config_table.keys = config_table.keys or {}
  local shadowed = shadowed_binding(config_table.keys, spec)
  local action = wezterm.action_callback(function(window, pane)
    if M.is_launcher(pane) then
      pane:send_text(M.FORWARD .. char)
    elseif fallback then
      fallback(window, pane)
    elseif shadowed then
      window:perform_action(shadowed.action, pane)
    end
  end)
  table.insert(config_table.keys, { key = spec.key, mods = spec.mods, action = action })
end

---Registers the user-var handler once and, on macOS, the chords WezTerm would otherwise swallow.
function M.register(config_table)
  if not registered then
    registered = true
    wezterm.on("user-var-changed", function(window, pane, name, value)
      local ok, err = pcall(on_user_var, window, pane, name, value)
      if not ok then
        util.warn("user-var-changed: %s", tostring(err))
      end
    end)
  end
  if platform.is_mac then
    M.bind(config_table, { key = "d", mods = "CMD|SHIFT" }, "D")
  end
end

return M
