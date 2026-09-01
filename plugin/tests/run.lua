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
local util = require "nardo.util"
local config = require "nardo.config"
local backend = require "nardo.backend"
local id = require "nardo.id"

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

test("config falls back to defaults for bad enums and out-of-range numbers", function()
  local cfg = config.setup { position = "top", width = 2, poll_ms = 10 }
  eq(cfg.position, "left")
  eq(cfg.width, 28)
  eq(cfg.poll_ms, 500)
  eq(config.setup({ width = 20 }).width, 20)
  eq(config.setup({ position = "right" }).position, "right")
  eq(config.setup({}).backend.uservar, id.ns)
end)

test("config.get returns the last setup", function()
  config.setup { width = 40 }
  eq(config.get().width, 40)
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

backend_test("remote spawn inlines the bootstrap and keeps args off $0", function()
  local cfg = config.setup {}
  local remote = backend.spawn_args(cfg, "desktop")
  eq(remote[1], "sh")
  eq(remote[2], "-c")
  assert(remote[3]:find("bootstrap.sh", 1, true) or remote[3]:find("backend not found", 1, true))
  eq(remote[4], "sh", "placeholder so the name lands on $1, not $0")
  eq(remote[5], id.name)
  eq(remote[6], id.prefix)
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

  local remote = backend.env(config.setup {}, "desktop")
  eq(remote[id.prefix .. "_TARGET"], nil, "no host triple for a remote domain")
  eq(remote[id.prefix .. "_BUILD"], "0")
end)

test("build=false disables the cargo fallback", function()
  local cfg = config.setup { backend = { build = false } }
  eq(backend.env(cfg, "local")[id.prefix .. "_BUILD"], "0")
end)

print(string.format("%d passed, %d failed", passed, failed))
os.exit(failed == 0 and 0 or 1)
