local wezterm = require "wezterm" ---@type Wezterm
local config = require "nardo.config"
local util = require "nardo.util"

local M = {}

local DOMAIN_LISTS = {
  { key = "unix_domains", kind = "unix" },
  { key = "tls_clients", kind = "tls" },
  { key = "ssh_domains", kind = "ssh" },
  { key = "exec_domains", kind = "exec" },
  { key = "wsl_domains", kind = "wsl" },
  { key = "serial_ports", kind = "serial" },
}

local PRESENTATION_FIELDS = { "mode", "width", "height", "max_width", "max_height", "backdrop", "animations" }

---Domain name → kind; the mux does not expose it, the config lists do.
local function domain_kinds(effective)
  local kinds = { ["local"] = "local" }
  for _, list in ipairs(DOMAIN_LISTS) do
    for _, entry in ipairs(effective[list.key] or {}) do
      if type(entry) == "table" and type(entry.name) == "string" then
        kinds[entry.name] = list.kind
      end
    end
  end
  return kinds
end

---@param window Window
---@param pane Pane
function M.origin(window, pane)
  local tab = util.try(pane.tab, pane)
  return {
    pane_id = util.try(pane.pane_id, pane),
    tab_id = tab and util.try(tab.tab_id, tab) or nil,
    window_id = util.try(window.window_id, window),
    workspace = util.try(window.active_workspace, window),
    domain = util.try(pane.get_domain_name, pane),
  }
end

---Remote domains may be tardy, so every call is guarded and internal-only domains are skipped.
function M.domains(effective)
  local kinds = domain_kinds(effective or {})
  local out = {}
  for _, domain in ipairs(util.try(wezterm.mux.all_domains) or {}) do
    local name = util.try(domain.name, domain)
    local spawnable = util.try(domain.is_spawnable, domain) ~= false
    local has_panes = util.try(domain.has_any_panes, domain) == true
    if name and (spawnable or has_panes) then
      out[#out + 1] = {
        name = name,
        label = util.try(domain.label, domain) or name,
        kind = kinds[name] or "unknown",
        state = util.try(domain.state, domain) or "Detached",
        spawnable = spawnable,
        has_panes = has_panes,
      }
    end
  end
  return out
end

local function cwd_of(pane)
  local url = util.try(pane.get_current_working_dir, pane)
  if url == nil then
    return nil
  end
  if type(url) == "string" then
    return url
  end
  local text = tostring(url)
  if text:find "^%a[%w+.-]*://" then
    return text
  end
  return util.try(function()
    return url.file_path
  end)
end

local function process_of(pane)
  local name = util.try(pane.get_foreground_process_name, pane)
  if type(name) ~= "string" or name == "" then
    return nil
  end
  return util.basename(name)
end

---Per-pane fields `wezterm cli list` lacks, keyed by pane id as a string for json.
function M.panes()
  local out = {}
  for _, window in ipairs(util.try(wezterm.mux.all_windows) or {}) do
    for _, tab in ipairs(util.try(window.tabs, window) or {}) do
      for _, pane in ipairs(util.try(tab.panes, tab) or {}) do
        local pane_id = util.try(pane.pane_id, pane)
        if pane_id then
          out[tostring(pane_id)] = {
            domain = util.try(pane.get_domain_name, pane),
            process = process_of(pane),
            cwd = cwd_of(pane),
            unseen = util.try(pane.has_unseen_output, pane) == true,
            alt_screen = util.try(pane.is_alt_screen_active, pane) == true,
          }
        end
      end
    end
  end
  return out
end

function M.workspaces()
  return {
    active = util.try(wezterm.mux.get_active_workspace),
    names = util.try(wezterm.mux.get_workspace_names) or {},
  }
end

---Normalises any colour spec WezTerm accepts to `#rrggbb`, or nil.
function M.hex(value)
  if value == nil then
    return nil
  end
  local text = tostring(value):lower()
  local rgb = text:match "^#(%x%x%x%x%x%x)"
  if rgb then
    return "#" .. rgb
  end
  local parsed = util.try(function()
    return tostring(wezterm.color.parse(text)):lower()
  end)
  rgb = parsed and parsed:match "^#(%x%x%x%x%x%x)"
  return rgb and ("#" .. rgb) or nil
end

local function hex_list(list, count)
  local out = {}
  for i = 1, count do
    local value = M.hex(type(list) == "table" and list[i] or nil)
    if not value then
      return {}
    end
    out[i] = value
  end
  return out
end

function M.theme(effective, cfg)
  local palette = (effective or {}).resolved_palette or {}
  local theme = {
    background = M.hex(palette.background),
    foreground = M.hex(palette.foreground),
    ansi = hex_list(palette.ansi, 8),
    brights = hex_list(palette.brights, 8),
    selection_bg = M.hex(palette.selection_bg),
    selection_fg = M.hex(palette.selection_fg),
    cursor_bg = M.hex(palette.cursor_bg),
  }
  if type(cfg.theme) == "table" then
    for key, value in pairs(cfg.theme) do
      local colour = M.hex(value)
      if colour then
        theme[key] = colour
      else
        util.warn_once("theme." .. tostring(key), "theme.%s is not a colour, ignored", tostring(key))
      end
    end
  end
  return theme
end

function M.presentation(cfg, overrides)
  local merged = util.merge(cfg.presentation, overrides or {})
  local out = {}
  for _, field in ipairs(PRESENTATION_FIELDS) do
    out[field] = merged[field]
  end
  return out
end

---Everything the launcher needs from the GUI, per docs/protocol.md "Context json". No child processes.
---@param window Window
---@param pane Pane
---@param app string
---@param opts { options: table|nil, presentation: table|nil }|nil
function M.snapshot(window, pane, app, opts)
  opts = opts or {}
  local cfg = config.get()
  local effective = util.try(window.effective_config, window) or {}
  return {
    v = 1,
    app = app,
    origin = M.origin(window, pane),
    domains = M.domains(effective),
    panes = M.panes(),
    workspaces = M.workspaces(),
    theme = M.theme(effective, cfg),
    presentation = M.presentation(cfg, opts.presentation),
    options = opts.options or {},
  }
end

return M
