local M = {}

M.target_triple = os.getenv "WEZPLUG_TEST_TRIPLE" or "aarch64-apple-darwin"

local function wide(code)
  return (code >= 0x1100 and code <= 0x115f)
    or (code >= 0x2e80 and code <= 0xa4cf)
    or (code >= 0xac00 and code <= 0xd7a3)
    or (code >= 0xf900 and code <= 0xfaff)
    or (code >= 0xff00 and code <= 0xff60)
    or (code >= 0x1f300 and code <= 0x1faff)
end

function M.column_width(s)
  local w = 0
  for _, code in utf8.codes(s) do
    if code >= 32 and code ~= 127 and not (code >= 0x80 and code <= 0x9f) then
      w = w + (wide(code) and 2 or 1)
    end
  end
  return w
end

M.executable_dir = "/usr/local/bin"
function M.hostname()
  return "macie"
end
function M.run_child_process()
  return true, "", ""
end

M.nerdfonts = {}
M.home_dir = "/tmp"
M.GLOBAL = {}

local function rgb_from_hex(hex)
  local r, g, b = hex:match "^#(%x%x)(%x%x)(%x%x)$"
  return tonumber(r, 16), tonumber(g, 16), tonumber(b, 16)
end

local Color = {}
Color.__index = Color

local function color(r, g, b)
  return setmetatable({ r = r, g = g, b = b }, Color)
end

function Color:srgba_u8()
  return self.r, self.g, self.b, 255
end

function Color:lighten(f)
  local function up(c)
    return math.min(255, math.floor(c + (255 - c) * f))
  end
  return color(up(self.r), up(self.g), up(self.b))
end

function Color:darken(f)
  local function down(c)
    return math.floor(c * (1 - f))
  end
  return color(down(self.r), down(self.g), down(self.b))
end

M.color = {
  parse = function(s)
    if type(s) ~= "string" then
      return nil
    end
    local r, g, b = rgb_from_hex(s)
    if not r then
      error("bad color " .. s)
    end
    return color(r, g, b)
  end,
}

-- Like the real thing, `wezterm.action.X` works bare or called with an argument.
M.action = setmetatable({}, {
  __index = function(_, name)
    return setmetatable({ action = name }, {
      __call = function(_, arg)
        return { action = name, arg = arg }
      end,
    })
  end,
})

function M.action_callback(fn)
  return { callback = fn }
end

M.log = {}
function M.log_info(msg)
  M.log[#M.log + 1] = msg
end
M.log_warn = M.log_info
M.log_error = M.log_info

M.handlers = {}
function M.on(name, fn)
  M.handlers[name] = M.handlers[name] or {}
  table.insert(M.handlers[name], fn)
end

M.time = {
  call_after = function(_, fn)
    fn()
  end,
  now = function()
    return {
      format = function()
        return tostring(os.time()) .. ".000"
      end,
    }
  end,
}

-- Mutable mux state; tests install fake_mux windows/domains here.
local mux_state = { windows = {}, domains = {}, spawned = {} }

local function each_pane(fn)
  for _, window in ipairs(mux_state.windows) do
    for _, tab in ipairs(window.tab_list or {}) do
      for _, pane in ipairs(tab.pane_list or {}) do
        local result = fn(window, tab, pane)
        if result ~= nil then
          return result
        end
      end
    end
  end
  return nil
end

M.mux = {
  state = mux_state,
  set = function(windows, domains)
    mux_state.windows = windows or {}
    mux_state.domains = domains or {}
    mux_state.spawned = {}
  end,
  all_windows = function()
    return mux_state.windows
  end,
  all_domains = function()
    return mux_state.domains
  end,
  get_domain = function(name)
    for _, domain in ipairs(mux_state.domains) do
      if domain:name() == name then
        return domain
      end
    end
    return nil
  end,
  get_pane = function(id)
    return each_pane(function(_, _, pane)
      if pane:pane_id() == id then
        return pane
      end
    end)
  end,
  get_window = function(id)
    for _, window in ipairs(mux_state.windows) do
      if window:window_id() == id then
        return window
      end
    end
    error("window " .. tostring(id) .. " not found")
  end,
  get_workspace_names = function()
    return { "default" }
  end,
  get_active_workspace = function()
    return "default"
  end,
  spawn_window = function(opts)
    mux_state.spawned[#mux_state.spawned + 1] = opts
    local fake = require "fake_mux"
    local window = fake.window()
    local tab = window:add_tab { process = opts.args and opts.args[1] or "sh" }
    tab.spawn = opts
    mux_state.windows[#mux_state.windows + 1] = window
    return tab, tab.pane_list[1], window
  end,
}

M.gui = {
  screens = function()
    return { active = { name = "main", x = 0, y = 0, width = 2560, height = 1440 } }
  end,
}

function M.glob(pattern)
  local dir, tail = pattern:match "^(.*)/([^/]*)$"
  if not dir then
    return {}
  end
  local lua_pattern = "^" .. tail:gsub("[%%%.%+%-%?%[%]%^%$%(%)]", "%%%0"):gsub("%*", ".*") .. "$"
  local out = {}
  local pipe = io.popen("ls -1 " .. string.format("%q", dir) .. " 2>/dev/null")
  if pipe then
    for line in pipe:lines() do
      if line:match(lua_pattern) then
        out[#out + 1] = dir .. "/" .. line
      end
    end
    pipe:close()
  end
  return out
end
M.plugin = {
  list = function()
    return {}
  end,
}

local function encode(v)
  local t = type(v)
  if t == "table" then
    local parts = {}
    for k, val in pairs(v) do
      parts[#parts + 1] = string.format("%q:%s", tostring(k), encode(val))
    end
    return "{" .. table.concat(parts, ",") .. "}"
  elseif t == "string" then
    return string.format("%q", v)
  end
  return tostring(v)
end
M.json_encode = encode

local function decode(s, i)
  i = s:find("%S", i)
  local c = s:sub(i, i)
  if c == "{" then
    local out = {}
    i = i + 1
    while true do
      i = s:find("%S", i)
      if s:sub(i, i) == "}" then
        return out, i + 1
      end
      local key
      key, i = decode(s, i)
      i = s:find(":", i) + 1
      out[key], i = decode(s, i)
      i = s:find("%S", i)
      if s:sub(i, i) == "," then
        i = i + 1
      end
    end
  elseif c == "[" then
    local out = {}
    i = i + 1
    while true do
      i = s:find("%S", i)
      if s:sub(i, i) == "]" then
        return out, i + 1
      end
      out[#out + 1], i = decode(s, i)
      i = s:find("%S", i)
      if s:sub(i, i) == "," then
        i = i + 1
      end
    end
  elseif c == '"' then
    local j = i + 1
    local buf = {}
    while s:sub(j, j) ~= '"' do
      if s:sub(j, j) == "\\" then
        j = j + 1
      end
      buf[#buf + 1] = s:sub(j, j)
      j = j + 1
    end
    return table.concat(buf), j + 1
  elseif s:sub(i, i + 3) == "true" then
    return true, i + 4
  elseif s:sub(i, i + 4) == "false" then
    return false, i + 5
  elseif s:sub(i, i + 3) == "null" then
    return nil, i + 4
  end
  local num = s:match("^-?%d+%.?%d*[eE]?[-+]?%d*", i)
  return tonumber(num), i + #num
end

function M.json_parse(s)
  local value = decode(s, 1)
  return value
end

return M
