local util = require "nardo.util"
local id = require "nardo.id"
local platform = require "nardo.platform"

local M = {}

M.defaults = {
  debug = false,
  presentation = {
    mode = "overlay",
    width = 0.72,
    height = 0.7,
    max_width = 128,
    max_height = 42,
    backdrop = "dim",
    animations = true,
    split = { direction = "Bottom", size = 0.5 },
  },
  theme = "auto", -- or { accent = "#hex", background = "#hex", ... }
  hooks = {}, -- { [action] = fun(window, pane, args) }
  backend = {
    path = nil, -- string | table keyed by host/domain | fun(domain, host): string
    repo = id.repo,
    version = nil, -- defaults to plugin/nardo/version.lua
    build = true,
    uservar = id.ns,
    log = nil, -- absolute path; the backend appends debug lines there
    class = nil, -- GUI `--class`, lets the launcher find a non-default GUI socket
  },
  sessions = {
    enabled = true,
    key = { key = "k", mods = platform.SUPER },
    confirm_kill = true,
    preview = true,
    preview_lines = 200,
    mru = true,
    scope = "all",
    show_self = false,
    keys = {}, -- { [action] = "key" | { "key", ... } | false }
  },
  palette = {
    enabled = true,
    key = { key = "p", mods = platform.SUPER2 },
    commands = {}, -- { { label = "..", action = wezterm.action.X | fun(window, pane), hint = ".." } }
  },
  quick = {
    enabled = true,
    key = { key = "t", mods = platform.is_mac and "ALT|CMD" or "CTRL|ALT" },
    height = 0.4,
    args = nil, -- program, nil = default shell
  },
}

local TABLES =
  { "presentation", "presentation.split", "hooks", "backend", "sessions", "sessions.keys", "palette", "quick" }

local ENUMS = {
  ["presentation.mode"] = { overlay = true, tab = true, window = true, split = true },
  ["presentation.backdrop"] = { dim = true, none = true },
  ["presentation.split.direction"] = { Left = true, Right = true, Top = true, Bottom = true },
  ["sessions.scope"] = { all = true, windows = true, tabs = true, panes = true, domains = true },
}

local RANGES = {
  ["presentation.width"] = { min = 0.1 },
  ["presentation.height"] = { min = 0.1 },
  ["presentation.max_width"] = { min = 20, integer = true },
  ["presentation.max_height"] = { min = 8, integer = true },
  ["presentation.split.size"] = { min = 0.05 },
  ["sessions.preview_lines"] = { min = 0, max = 100000, integer = true },
  ["quick.height"] = { min = 0.05, max = 1 },
}

local KEYS = { "sessions.key", "palette.key", "quick.key" }

local function get(t, path)
  for part in path:gmatch "[^.]+" do
    if type(t) ~= "table" then
      return nil
    end
    t = t[part]
  end
  return t
end

local function set(t, path, value)
  local last
  for part in path:gmatch "[^.]+" do
    if last then
      t = t[last]
    end
    last = part
  end
  t[last] = value
end

local function reset(cfg, path, reason)
  util.warn("%s %s, using default", path, reason)
  set(cfg, path, util.merge({}, { value = get(M.defaults, path) }).value)
end

local function check_tables(cfg)
  for _, path in ipairs(TABLES) do
    if type(get(cfg, path)) ~= "table" then
      reset(cfg, path, "must be a table")
    end
  end
end

local function check_enums(cfg)
  for path, allowed in pairs(ENUMS) do
    if not allowed[get(cfg, path)] then
      reset(cfg, path, "is not one of " .. table.concat(util.sorted_keys(allowed), "/"))
    end
  end
end

local function check_ranges(cfg)
  for path, range in pairs(RANGES) do
    local value = get(cfg, path)
    local bad = type(value) ~= "number" or (range.min and value < range.min) or (range.max and value > range.max)
    if bad then
      reset(cfg, path, string.format("must be a number in [%s, %s]", range.min or "-inf", range.max or "inf"))
    elseif range.integer then
      set(cfg, path, math.floor(value))
    end
  end
end

---A binding is `{ key, mods }`, a bare key string, or `false` to leave the chord alone.
local function check_keys(cfg)
  for _, path in ipairs(KEYS) do
    local spec = get(cfg, path)
    if type(spec) == "string" and spec ~= "" then
      set(cfg, path, { key = spec, mods = get(M.defaults, path).mods })
    elseif spec ~= false and not (type(spec) == "table" and type(spec.key) == "string" and spec.key ~= "") then
      reset(cfg, path, "must be { key = .., mods = .. } or false")
    end
  end
end

local function check_theme(cfg)
  if cfg.theme ~= "auto" and type(cfg.theme) ~= "table" then
    reset(cfg, "theme", 'must be "auto" or a table')
  end
end

local function check_hooks(cfg)
  for name, fn in pairs(cfg.hooks) do
    if type(fn) ~= "function" then
      util.warn("hooks.%s must be a function, ignored", tostring(name))
      cfg.hooks[name] = nil
    end
  end
end

local function check_backend(cfg)
  if type(cfg.backend.uservar) ~= "string" or cfg.backend.uservar == "" then
    reset(cfg, "backend.uservar", "must be a non-empty string")
  end
end

local function check_session_keys(cfg)
  for action, keys in pairs(cfg.sessions.keys) do
    local ok = keys == false or type(keys) == "string" or type(keys) == "table"
    if not ok then
      util.warn("sessions.keys.%s must be a key, a list of keys or false, ignored", tostring(action))
      cfg.sessions.keys[action] = nil
    end
  end
end

local current = nil

function M.setup(opts)
  local cfg = util.merge(M.defaults, opts or {})
  check_tables(cfg)
  check_enums(cfg)
  check_ranges(cfg)
  check_keys(cfg)
  check_theme(cfg)
  check_hooks(cfg)
  check_backend(cfg)
  check_session_keys(cfg)
  util.debug_enabled = cfg.debug == true
  current = cfg
  return cfg
end

function M.get()
  return current or M.setup {}
end

return M
