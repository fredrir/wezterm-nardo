local util = require "nardo.util"
local id = require "nardo.id"

local M = {}

M.defaults = {
  debug = false,
  poll_ms = 500,
  keys = {},
  hooks = {},
  -- Example options; replace `position` and `width` with your own.
  position = "left",
  width = 28,
  backend = {
    path = nil, -- string | table keyed by host/domain | fun(domain, host): string
    repo = id.repo,
    version = nil, -- defaults to plugin/<ns>/version.lua
    build = true, -- allow the cargo fallback when no release matches
    uservar = id.ns,
    log = nil, -- absolute path; the backend appends debug lines there
  },
}

local ENUMS = {
  position = { left = true, right = true },
}

local RANGES = {
  width = { min = 8 },
  poll_ms = { min = 50 },
}

local current = nil

function M.setup(opts)
  local cfg = util.merge(M.defaults, opts or {})
  for key, allowed in pairs(ENUMS) do
    if not allowed[cfg[key]] then
      util.warn("invalid %s=%s, using default", key, tostring(cfg[key]))
      cfg[key] = M.defaults[key]
    end
  end
  for key, range in pairs(RANGES) do
    local value = cfg[key]
    local bad = type(value) ~= "number" or (range.min and value < range.min) or (range.max and value > range.max)
    if bad then
      util.warn("%s must be a number in [%s, %s], using default", key, range.min or "-inf", range.max or "inf")
      cfg[key] = M.defaults[key]
    end
  end
  current = cfg
  return cfg
end

function M.get()
  return current or M.setup {}
end

return M
