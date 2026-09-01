local here = arg[0]:match "^(.*)[/\\]" or "."
package.path = here .. "/../?.lua;" .. here .. "/?.lua;" .. package.path
package.preload.wezterm = function()
  return require "wezterm_stub"
end

local passed, failed = 0, 0
local function test(name, fn)
  local ok, err = pcall(fn)
  if ok then
    passed = passed + 1
  else
    failed = failed + 1
    print("FAIL " .. name .. ": " .. tostring(err))
  end
end

local function eq(a, b, msg)
  if a ~= b then
    error((msg or "") .. string.format(" expected %s got %s", tostring(b), tostring(a)), 2)
  end
end

local wezterm = require "wezterm"
local fake = require "fake_mux"
local util = require "nardo.util"
local config = require "nardo.config"
local backend = require "nardo.backend"
local context = require "nardo.context"
local launcher = require "nardo.launcher"
local palette = require "nardo.palette"
local sessions = require "nardo.sessions"
local id = require "nardo.id"
local platform = require "nardo.platform"

backend.root = here .. "/.."

-- A --no-backend plugin has no bootstrap; its spawn tests do not apply.
local has_backend = (function()
  local f = io.open(backend.root .. "/bin/bootstrap.sh")
  if f then
    f:close()
    return true
  end
  return false
end)()

local function backend_test(name, fn)
  if has_backend then
    test(name, fn)
  end
end

---One origin window with two panes plus a tls domain pair; returns gui, origin pane, mux window.
local function rig(domains)
  wezterm.GLOBAL[id.ns .. "_launchers"] = nil
  wezterm.GLOBAL[id.ns .. "_sessions"] = nil
  local window = fake.window()
  local tab = window:add_tab { title = "work" }
  local origin = tab.pane_list[1]
  origin.domain = "localmux"
  fake.pane(tab, { title = "vim", domain = "localmux" })
  wezterm.mux.set({ window }, domains or {
    fake.domain { name = "local", kind = "local" },
    fake.domain { name = "localmux", has_panes = true },
    fake.domain { name = "archie-wifi", state = "Detached" },
  })
  window.gui.config = {
    unix_domains = { { name = "localmux" } },
    tls_clients = { { name = "archie-wifi" } },
    resolved_palette = {
      background = "#1E1E2E",
      foreground = "#cdd6f4",
      ansi = { "#45475a", "#f38ba8", "#a6e3a1", "#f9e2af", "#89b4fa", "#f5c2e7", "#94e2d5", "#bac2de" },
      brights = { "#585b70", "#f38ba8", "#a6e3a1", "#f9e2af", "#89b4fa", "#f5c2e7", "#94e2d5", "#a6adc8" },
      selection_bg = "#45475a",
    },
  }
  return window.gui, origin, window
end

test("id derives name, prefix and url from ns", function()
  eq(id.name, "wez-" .. id.ns)
  eq(id.prefix, (id.ns:upper():gsub("%-", "_")))
  eq(id.url, "https://github.com/" .. id.repo)
  assert(id.prefix:match "^[A-Z][A-Z0-9_]*$", "prefix is a valid env prefix: " .. id.prefix)
end)

test("merge nests tables and replaces lists", function()
  local out = util.merge({ a = { b = 1, c = 2 }, l = { 1, 2 } }, { a = { c = 3 }, l = { 9 } })
  eq(out.a.b, 1)
  eq(out.a.c, 3)
  eq(#out.l, 1)
end)

test("truncate keeps width budget for wide chars", function()
  eq(util.truncate("hello world", 5, "…"), "hell…")
  eq(util.truncate("hi", 5, "…"), "hi")
  eq(util.width(util.truncate("ünïcödé text", 6, "…")), 6)
  local cjk = util.truncate("日本語のタイトル", 7, "…")
  assert(util.width(cjk) <= 7, "cjk width " .. util.width(cjk))
  eq(util.truncate("abc", 0, "…"), "")
end)

test("sanitize strips control and C1 characters", function()
  eq(util.sanitize "a\27]52;c;xx\7b\194\133c", "a]52;c;xxbc")
  eq(util.sanitize(nil), "")
end)

test("config falls back to defaults for bad enums, ranges and shapes", function()
  local cfg = config.setup {
    presentation = { mode = "popup", backdrop = "blur", width = 0 },
    sessions = { scope = "bogus" },
    quick = { height = 3 },
    theme = "nope",
    hooks = { done = 5 },
  }
  eq(cfg.presentation.mode, "overlay")
  eq(cfg.presentation.backdrop, "dim")
  eq(cfg.presentation.width, 0.72)
  eq(cfg.sessions.scope, "all")
  eq(cfg.quick.height, 0.4)
  eq(cfg.theme, "auto")
  eq(cfg.hooks.done, nil)
  eq(cfg.backend.uservar, id.ns)
end)

test("config normalises key specs and honours false", function()
  local cfg = config.setup { sessions = { key = "j" }, palette = { key = false }, quick = { key = { mods = "ALT" } } }
  eq(cfg.sessions.key.key, "j")
  eq(cfg.sessions.key.mods, platform.SUPER)
  eq(cfg.palette.key, false)
  eq(cfg.quick.key.key, config.defaults.quick.key.key, "broken spec resets to the default")
end)

test("config.get returns the last setup", function()
  config.setup { presentation = { width = 0.5 } }
  eq(config.get().presentation.width, 0.5)
end)

test("backend path resolves per domain and host", function()
  local cfg = config.setup { backend = { path = "/bin/" .. id.name } }
  eq(backend.resolve_path(cfg, "local"), "/bin/" .. id.name)
  eq(backend.resolve_path(cfg, "desktop"), nil)
  backend.register_local_domains { unix_domains = { { name = "localmux" } } }
  eq(backend.resolve_path(cfg, "localmux"), "/bin/" .. id.name)
  eq(backend.resolve_path(cfg, "localmux", "macie"), "/bin/" .. id.name)
  eq(backend.resolve_path(cfg, "localmux", "archie"), nil, "proxied remote host is not local")

  cfg = config.setup { backend = { path = { ["local"] = "/l", archie = "/a", desktop = "/d" } } }
  eq(backend.resolve_path(cfg, "localmux", "archie"), "/a")
  eq(backend.resolve_path(cfg, "desktop"), "/d")

  cfg = config.setup {
    backend = {
      path = function(_, host)
        return host == "archie" and "/h" or "/m"
      end,
    },
  }
  eq(backend.spawn_args(cfg, "localmux", "archie")[1], "/h")
end)

backend_test("local spawn passes name and prefix to the bootstrap", function()
  local cfg = config.setup {}
  local args = backend.spawn_args(cfg, "local")
  eq(args[1], "sh")
  assert(args[2]:find("bin/bootstrap.sh", 1, true), "bootstrap path: " .. tostring(args[2]))
  eq(args[3], id.name)
  eq(args[4], id.prefix)
end)

test("env is prefixed for the bootstrap and neutral for the binary", function()
  local cfg = config.setup { backend = { path = "/bin/" .. id.name, log = "/tmp/x.log" } }
  local env = backend.env(cfg, "local")
  eq(env.WEZPLUG_USERVAR, id.ns)
  eq(env.WEZPLUG_LOG, "/tmp/x.log")
  eq(env[id.prefix .. "_TARGET"], wezterm.target_triple)
  eq(env[id.prefix .. "_BIN"], "/bin/" .. id.name)
  eq(env[id.prefix .. "_BUILD"], "1")
  eq(env[id.prefix .. "_REPO"], id.repo)
end)

test("context snapshot matches docs/protocol.md", function()
  local gui, origin = rig()
  local ctx = context.snapshot(gui, origin, "sessions", { options = { mru = true } })
  eq(ctx.v, 1)
  eq(ctx.app, "sessions")
  eq(ctx.origin.pane_id, origin:pane_id())
  eq(ctx.origin.domain, "localmux")
  eq(type(ctx.origin.window_id), "number")
  local by_name = {}
  for _, domain in ipairs(ctx.domains) do
    by_name[domain.name] = domain
  end
  eq(by_name["localmux"].kind, "unix")
  eq(by_name["archie-wifi"].kind, "tls")
  eq(by_name["archie-wifi"].state, "Detached")
  eq(by_name["local"].kind, "local")
  local extra = ctx.panes[tostring(origin:pane_id())]
  eq(extra.domain, "localmux")
  eq(extra.process, "zsh")
  eq(type(extra.cwd), "string")
  eq(ctx.workspaces.active, "default")
  eq(ctx.theme.background, "#1e1e2e")
  eq(ctx.theme.ansi[5], "#89b4fa")
  eq(ctx.presentation.mode, "overlay")
  eq(ctx.presentation.split, nil, "split stays lua-side")
  eq(ctx.options.mru, true)
end)

test("hex normalises colours and rejects junk", function()
  eq(context.hex "#AABBCC", "#aabbcc")
  eq(context.hex(nil), nil)
  eq(context.hex "junk", nil)
end)

local function open_rig(overrides)
  local gui, origin, window = rig()
  config.setup(util.merge({ backend = { path = "/bin/" .. id.name } }, overrides or {}))
  local pane = launcher.open(gui, origin, { app = "sessions", presentation = overrides and overrides.presentation })
  return gui, origin, window, pane
end

test("launcher spawns in the local domain with context, exe and uservar env", function()
  local _, _, window, pane = open_rig()
  assert(pane, "launcher pane")
  local tab = window.tab_list[#window.tab_list]
  eq(tab.spawn.domain.DomainName, "local")
  eq(tab.spawn.args[1], "/bin/" .. id.name)
  local env = tab.spawn.set_environment_variables
  eq(env[id.prefix .. "_WEZTERM"], wezterm.executable_dir .. "/wezterm")
  eq(env.WEZPLUG_USERVAR, id.ns)
  eq(type(env.WEZTERM_UNIX_SOCKET), "string")
  eq(tab.title, " ", "calm tab title")
  local file = assert(io.open(env[id.prefix .. "_CONTEXT"], "r"))
  local ctx = wezterm.json_parse(file:read "a")
  file:close()
  eq(ctx.app, "sessions")
  eq(ctx.presentation.mode, "overlay")
  os.remove(env[id.prefix .. "_CONTEXT"])
end)

test("double open focuses the live launcher instead of spawning", function()
  local gui, origin, window, pane = open_rig()
  local tabs_before = #window.tab_list
  local again = launcher.open(gui, origin, { app = "sessions" })
  eq(again, pane)
  eq(#window.tab_list, tabs_before)
end)

test("split and window presentations dispatch to split and spawn_window", function()
  local _, origin = open_rig { presentation = { mode = "split" } }
  local split = origin._tab.pane_list[1].split_args
  eq(split.direction, "Bottom")
  eq(split.size, 0.5)
  eq(split.domain.DomainName, "local")

  open_rig { presentation = { mode = "window" } }
  local spawned = wezterm.mux.state.spawned[1]
  assert(spawned, "spawn_window called")
  eq(spawned.domain.DomainName, "local")
  assert(spawned.width and spawned.height, "sized")
  local popup = wezterm.mux.state.windows[#wezterm.mux.state.windows]
  eq(popup.gui.overrides.enable_tab_bar, false)
end)

local function dispatch(gui, pane, action)
  launcher.dispatch(gui, pane, action)
end

test("user-var actions: focus, attach_domain, run, done", function()
  local gui, _, window, pane = open_rig()
  local other = window.tab_list[1].pane_list[1]
  dispatch(gui, pane, { t = "focus", pane_id = other:pane_id() })
  eq(window.tab_list[1].active, other)

  local domain = wezterm.mux.get_domain "archie-wifi"
  dispatch(gui, pane, { t = "attach_domain", domain = "archie-wifi" })
  eq(domain:state(), "Attached")
  eq(gui._mux.actions[#gui._mux.actions].action.action, "SpawnCommandInNewTab", "empty domain spawns")

  local got
  launcher.on_action("hello", function(_, _, args)
    got = args.x
  end)
  dispatch(gui, pane, { t = "run", name = "hello", args = { x = 7 } })
  eq(got, 7)

  local session = wezterm.GLOBAL[id.ns .. "_sessions"][tostring(pane:pane_id())]
  dispatch(gui, pane, { t = "done", exit = "activated" })
  eq(io.open(session.context, "r"), nil, "context file removed")
  eq(wezterm.GLOBAL[id.ns .. "_launchers"][tostring(gui:window_id())], nil)
end)

test("user-var handler survives malformed payloads and foreign vars", function()
  local gui, origin = rig()
  launcher.register { keys = {} }
  local handler = wezterm.handlers["user-var-changed"][#wezterm.handlers["user-var-changed"]]
  handler(gui, origin, "someone_else", "xxx")
  handler(gui, origin, id.ns, "not json at all")
  handler(gui, origin, id.ns .. "_role", "launcher")
  eq(wezterm.GLOBAL[id.ns .. "_launchers"][tostring(gui:window_id())], origin:pane_id())
end)

test("forwarded chords reach launcher panes only", function()
  local gui, origin, window = rig()
  local conf = { keys = { { key = "d", mods = "CMD|SHIFT", action = { action = "UserBinding" } } } }
  launcher.bind(conf, { key = "d", mods = "CMD|SHIFT" }, "D")
  local binding = conf.keys[#conf.keys]
  local target = fake.pane(window.tab_list[1], { vars = { [id.ns .. "_role"] = "launcher" } })
  binding.action.callback(gui, target)
  eq(target.sent[1], utf8.char(0xE000) .. "D")
  binding.action.callback(gui, origin)
  eq(gui._mux.actions[#gui._mux.actions].action.action, "UserBinding", "non-launcher runs the shadowed binding")
end)

test("palette builds ids and runs commands through perform_action", function()
  local gui, origin = rig()
  local hits = 0
  local cfg = config.setup {
    palette = {
      commands = {
        {
          label = "Custom",
          action = function()
            hits = hits + 1
          end,
        },
        { broken = true },
      },
    },
  }
  local list = palette.commands(cfg)
  eq(list[1].label, "Custom")
  eq(list[1].id, "p1")
  assert(#list > #palette.BUILTIN, "built-ins appended")
  palette.run(gui, origin, list[1])
  eq(hits, 1)
  palette.run(gui, origin, list[2])
  eq(gui._mux.actions[#gui._mux.actions].action.action, "ReloadConfiguration")
end)

test("sessions options follow config and omit empty keys", function()
  config.setup {}
  local options = sessions.options(config.get(), {})
  eq(options.confirm_kill, true)
  eq(options.preview_lines, 200)
  eq(options.keys, nil)
  options = sessions.options(config.get(), { keys = { kill = "x" }, scope = "panes" })
  eq(options.keys.kill, "x")
  eq(options.scope, "panes")
end)

print(string.format("%d passed, %d failed", passed, failed))
os.exit(failed == 0 and 0 or 1)
