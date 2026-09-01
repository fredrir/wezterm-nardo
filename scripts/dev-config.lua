-- Standalone config for the `just dev` sandbox WezTerm.
local wezterm = require "wezterm"
local root = os.getenv "WEZPLUG_ROOT"
package.path = root .. "/plugin/?.lua;" .. root .. "/plugin/?/init.lua;" .. package.path

local plugin = dofile(root .. "/plugin/init.lua")
local config = wezterm.config_builder()

config.initial_cols = 120
config.initial_rows = 34
config.window_close_confirmation = "NeverPrompt"
config.exit_behavior = "Close"
config.window_decorations = "RESIZE"
config.color_scheme = "Catppuccin Mocha"

local bin = os.getenv "WEZPLUG_BIN"
if bin == "" then
  bin = nil
end

-- CTRL|SHIFT chords behave the same on every OS, so the sandbox binds those explicitly.
plugin.apply_to_config(config, {
  debug = true,
  backend = { path = bin, build = false, log = "/tmp/wez-nardo-dev.log" },
  sessions = { key = { key = "k", mods = "CTRL|SHIFT" } },
  palette = { key = { key = "p", mods = "CTRL|SHIFT" } },
})

return config
