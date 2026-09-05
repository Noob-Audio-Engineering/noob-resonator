//! Noob Resonator without a DAW: a fake audio thread runs a demo source
//! (impulses, clicks, noise bursts, a saw, a sine) through the resonator at
//! 48 kHz / 256 samples, publishes the meter, partials, readout and response
//! streams, and serves the SPA from `web/dist` (or lets `vite` proxy to it).
//!
//! ```text
//! cargo run --bin noob-resonator-standalone -- [--port N] [--open] [--dir path]
//! ```
//!
//! | flag | meaning |
//! |---|---|
//! | `--port N` | insist on port `N` (otherwise 4246, walking up if taken) |
//! | `--open` | open the page in the system browser |
//! | `--dir path` | serve this directory instead of `web/dist` |
//!
//! The page's own state persists in a file through noob-vst-webgui-framework's
//! `FileStore`; the plug-in keeps the same data inside its host state. **The
//! per-mode override table lives in that store**, under the key `modes`, so
//! editing a partial here survives a restart exactly as it survives a project
//! reload in a host.
//!
//! A `status` message goes out once a second with the client count, block
//! count, edit count and how far a pending mode search has got.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use noob_resonator::dsp::{self, ModeTable, Processor, Source};
use noob_vst_webgui_framework::{AudioHandle, FileStore, ServerConfig};
use serde_json::json;

const SR: f32 = 48_000.0;
const BLOCK: usize = 256;

struct Stats {
    blocks: AtomicU64,
    /// How far a pending mode search has got, in thousandths, so it can live
    /// in an atomic.
    build: AtomicUsize,
    modes: AtomicUsize,
}

/// The fake audio thread: generate, resonate, publish, sleep until the next
/// block.
fn audio_thread(
    mut audio: AudioHandle,
    ix: dsp::ParamIx,
    table: Arc<ModeTable>,
    stats: Arc<Stats>,
) {
    let mut processor = Processor::with_table(SR, table);
    let mut source = Source::new(0x9E37_79B9);
    let mut l = vec![0.0f32; BLOCK];
    let mut r = vec![0.0f32; BLOCK];
    let block_dur = Duration::from_secs_f64(BLOCK as f64 / SR as f64);
    let mut next = Instant::now();
    let mut n: u64 = 0;
    loop {
        let settings = dsp::read_settings(&audio, &ix);
        processor.configure(&settings);
        let kind = ix
            .src_kind
            .map(|i| audio.param(i).round() as usize)
            .unwrap_or(0);
        let rate = ix.src_freq.map(|i| audio.param(i)).unwrap_or(2.0);
        let level = ix.src_level.map(|i| audio.param(i)).unwrap_or(0.5);
        for i in 0..BLOCK {
            let x = source.next(kind, rate, SR) * level;
            l[i] = x;
            // A touch of decorrelation, so the width control has something to
            // work on even from a single generator.
            r[i] = x * 0.97;
        }
        processor.process(&mut l, &mut r);
        processor.publish(&mut audio);
        n += 1;
        stats.blocks.store(n, Ordering::Relaxed);
        let info = processor.engine().info_frame();
        stats
            .build
            .store((info[10] * 1000.0) as usize, Ordering::Relaxed);
        stats.modes.store(info[0] as usize, Ordering::Relaxed);

        next += block_dur;
        let now = Instant::now();
        if next > now {
            thread::sleep(next - now);
        } else if now - next > Duration::from_millis(200) {
            next = now;
        }
    }
}

/// Say what went wrong on **both** streams, and leave a non-zero status.
///
/// A panic goes to stderr alone, so a caller capturing stdout sees an empty
/// run and a zero exit and has nothing at all to go on. Whatever kills this
/// program should be readable wherever the reader happened to be looking.
fn fail(what: &str) -> ! {
    println!("noob-resonator standalone: {what}");
    eprintln!("noob-resonator standalone: {what}");
    std::process::exit(1);
}

fn open_browser(url: &str) {
    #[cfg(target_os = "windows")]
    let r = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let r = std::process::Command::new("xdg-open").arg(url).spawn();
    if let Err(e) = r {
        log::warn!("could not open browser: {e}");
    }
}

fn main() {
    // Before anything that can fail, so that "it exited with no output" can
    // only ever mean "it did not start", and never "it started and died
    // quietly". An hour went today to a silent exit that turned out to be
    // cargo refusing a *different* binary: `noob-resonator-host` needs
    // `--features hostapp`, says so as a cargo error rather than as program
    // output, and sits next to this one in the manifest.
    println!(
        "noob-resonator standalone {} — starting",
        env!("CARGO_PKG_VERSION")
    );
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let mut port: Option<u16> = None;
    let mut open = false;
    let mut dir: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--port" | "-p" => port = args.next().and_then(|v| v.parse().ok()),
            "--open" | "-o" => open = true,
            "--dir" | "-d" => dir = args.next().map(PathBuf::from),
            "-h" | "--help" => {
                println!("noob-resonator standalone [--port N] [--open] [--dir path]");
                return;
            }
            other => log::warn!("ignoring argument {other}"),
        }
    }
    let web = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/web"));
    let dist = web.join("dist");
    let (dir, built) = match dir {
        Some(d) => (d, true),
        None => (dist.clone(), dist.join("index.html").is_file()),
    };
    let dir = dir.canonicalize().unwrap_or(dir);

    let (bridge, ix) = dsp::build_bridge("noob-resonator", SR);
    let Some(audio) = bridge.take_audio() else {
        fail("the bridge would not yield its audio handle, so nothing could be published");
    };

    // The store has to be attached before the mode table reads from it, so
    // that a table saved last time is in place before the first block.
    let store = FileStore::attach(&bridge, FileStore::default_path("noob-resonator"));
    let table = Arc::new(ModeTable::new());
    dsp::attach_mode_table(&bridge, table.clone());

    let stats = Arc::new(Stats {
        blocks: AtomicU64::new(0),
        build: AtomicUsize::new(0),
        modes: AtomicUsize::new(0),
    });
    {
        let stats = stats.clone();
        let table = table.clone();
        thread::Builder::new()
            .name("fake-audio".into())
            .spawn(move || audio_thread(audio, ix, table, stats))
            .expect("spawn audio thread");
    }

    let cfg = match port {
        Some(p) => ServerConfig::default().port(p),
        None => ServerConfig::default().prefer_port(4246),
    };
    let server = match noob_vst_webgui_framework::serve(&bridge, cfg.assets_dir(&dir)) {
        Ok(s) => s,
        Err(e) => fail(&format!("the server would not start: {e}")),
    };
    println!();
    println!("  noob-resonator standalone     {}", server.url());
    println!("  websocket                     {}", server.ws_url());
    println!("  assets                        {}", dir.display());
    println!("  ui store                      {}", store.path().display());
    if !built {
        println!();
        println!("  web/dist not found. Either build the SPA once:");
        println!("      cd web && npm install && npm run build");
        println!("  or develop with hot reload (proxies /ws to this server):");
        println!(
            "      cd web && NOOB_VST_WEBGUI_FRAMEWORK_PORT={} npm run dev",
            server.port()
        );
    }
    println!();
    if open {
        open_browser(&server.url());
    }

    let mut last_status = Instant::now();
    let mut edits = 0u64;
    loop {
        bridge.drain_edits(|_| edits += 1);
        while let Some(m) = bridge.poll_message() {
            match m.topic.as_str() {
                "reset" => {
                    for i in 0..bridge.param_count() {
                        let d = bridge.spec(i).map(|s| s.default).unwrap_or(0.0);
                        bridge.set_param(i, d);
                    }
                }
                "resize" | "fullscreen" => {}
                other => log::info!("message from client {}: {other} {}", m.client, m.data),
            }
        }
        if last_status.elapsed() >= Duration::from_secs(1) {
            last_status = Instant::now();
            bridge.send_json(
                "status",
                json!({
                    "clients": server.client_count(),
                    "blocks": stats.blocks.load(Ordering::Relaxed),
                    "edits": edits,
                    "dropped": bridge.dropped_ui_changes(),
                    "sample_rate": SR,
                    "block": BLOCK,
                    "latency_samples": 0,
                    "latency_ms": 0.0,
                    "modes": stats.modes.load(Ordering::Relaxed),
                    "build": stats.build.load(Ordering::Relaxed) as f32 / 1000.0,
                }),
            );
        }
        if let Err(e) = store.flush() {
            log::warn!("could not save the UI store: {e}");
        }
        thread::sleep(Duration::from_millis(5));
    }
}
