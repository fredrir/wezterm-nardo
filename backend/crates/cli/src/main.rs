use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use clap::{Args, Parser, Subcommand};
use nardo_core::context::Context;
use nardo_core::keys::parse_script;
use nardo_core::runtime::{self, Headless, RunOptions};
use nardo_core::wezterm::Cli as WeztermCli;

#[derive(Parser)]
#[command(name = "wez-nardo", version, about = "WezTerm launcher")]
struct Argv {
    #[command(subcommand)]
    app: AppCmd,
}

#[derive(Subcommand)]
enum AppCmd {
    /// Session explorer: domains, windows, tabs, panes
    Sessions(Common),
    /// Command palette
    Palette(Common),
}

#[derive(Args, Clone)]
struct Common {
    #[arg(long, env = "NARDO_CONTEXT")]
    context: Option<PathBuf>,
    #[arg(long, env = "NARDO_WEZTERM")]
    wezterm: Option<PathBuf>,
    /// Run without a tty against a TestBackend, print outcome json
    #[arg(long)]
    headless: bool,
    /// Key script for --headless, e.g. 'vim enter'
    #[arg(long, default_value = "")]
    keys: String,
    /// Include the view snapshot in the outcome
    #[arg(long)]
    dump: bool,
    /// COLSxROWS for --headless
    #[arg(long, default_value = "120x40")]
    size: String,
}

fn parse_size(s: &str) -> anyhow::Result<(u16, u16)> {
    let (c, r) = s.split_once('x').ok_or_else(|| anyhow::anyhow!("size: expected COLSxROWS, got {s}"))?;
    Ok((c.trim().parse()?, r.trim().parse()?))
}

fn options(common: &Common) -> anyhow::Result<RunOptions> {
    let context = Arc::new(Context::load(common.context.as_deref())?);
    let wezterm = Arc::new(WeztermCli::from_env(common.wezterm.clone()));
    let headless = common.headless.then(|| -> anyhow::Result<Headless> {
        Ok(Headless { size: parse_size(&common.size)?, script: parse_script(&common.keys)?, dump: common.dump })
    });
    Ok(RunOptions { context, wezterm, headless: headless.transpose()? })
}

fn run() -> anyhow::Result<()> {
    let argv = Argv::parse();
    let (common, report) = match &argv.app {
        AppCmd::Sessions(c) => (c, runtime::run(nardo_sessions::SessionsApp::default(), options(c)?)?),
        AppCmd::Palette(c) => (c, runtime::run(nardo_palette::PaletteApp::default(), options(c)?)?),
    };
    if common.headless {
        println!("{}", serde_json::to_string(&report)?);
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("wez-nardo: {err:#}");
            ExitCode::FAILURE
        }
    }
}
