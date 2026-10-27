use harborvm::{
    Result, bail,
    profile::{GuestOS, Profile, SetupOptions, Store},
};
use serde_json::{Value, json};
use std::path::Path;

fn help() {
    println!(
        "HarborVM — local VM workspace\n\n  app                                  Open the desktop workspace\n  profile list | show ID               Inspect saved machines\n  profile create ID [--name NAME] [--cpus N] [--memory MIB] [--disk GIB] [--os linux|macos] [--without COMPONENTS]\n  profile assistant ID PROGRAM         Choose the local Codex executable\n  profile connection ID PROVIDER [--endpoint URL] [--model MODEL] [--key-env NAME]\n  assistant models ID                  Discover API models\n  assistant models-json ID             Discover models with stdin JSON key\n  prepare ID [--cache DIRECTORY]        Prepare a fresh guest\n  prepare ID --restore-image FILE.ipsw  Install macOS from an Apple restore image\n  start | status | pause | resume ID    Control a machine\n  stop ID                              Request graceful shutdown\n  poweroff ID                          Force stop; unsaved work may be lost\n  exec ID -- PROGRAM [ARG...]           Run a guest command (60 s timeout)\n  run ID SHELL-LINE                     Run a shell line inside the guest\n  chat ID PROMPT                        Ask the selected assistant provider\n  chat-json ID                          Read conversation JSON from stdin\n  screenshot ID OUTPUT.png [--app]      Capture the guest or workspace\n  input ID click X Y | key KEY | text TEXT\n\nHARBORVM_HOME selects the private data directory. HARBORVM_APP selects the native app.\nSetup components: desktop,browser,python,node,playwright or all. Disabling a prerequisite also disables its dependents.\nGuest commands never fall back to host execution."
    );
}
fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || matches!(args[0].as_str(), "help" | "--help" | "-h") {
        help();
        return Ok(());
    }
    if args[0] == "--version" {
        println!("harborvm {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let store = Store::open(Store::default_root()?)?;
    let arg = |i: usize| {
        args.get(i)
            .map(String::as_str)
            .ok_or_else(|| anyhow::anyhow!("Missing argument; run harborvm help"))
    };
    if args[0] == "app" {
        harborvm::bridge::launch(&store, true)?;
        println!("{{\"ok\":true}}");
        return Ok(());
    }
    let result = match (arg(0)?, arg(1)?) {
        ("prepare", id) => {
            let mut cache = None;
            let mut restore = None;
            let mut i = 2;
            while i < args.len() {
                match arg(i)? {
                    "--cache" => cache = Some(Path::new(arg(i + 1)?)),
                    "--restore-image" => restore = Some(Path::new(arg(i + 1)?)),
                    _ => bail!("Usage: prepare ID [--cache DIRECTORY | --restore-image FILE.ipsw]"),
                }
                i += 2;
            }
            if store.load(id)?.guest_os == GuestOS::Macos {
                if cache.is_some() {
                    bail!("Use --restore-image for a local macOS IPSW");
                }
                harborvm::provision::prepare_macos(&store, id, restore)?;
            } else {
                if restore.is_some() {
                    bail!("--restore-image is only for macOS profiles");
                }
                harborvm::provision::prepare(&store, id, cache)?;
            }
            store.descriptor(id)?
        }
        ("exec" | "run", id) => {
            let argv = if args[0] == "exec" {
                if arg(2)? != "--" {
                    bail!("Usage: exec ID -- PROGRAM [ARG...]");
                }
                args[3..].to_vec()
            } else {
                vec!["/bin/sh".into(), "-lc".into(), arg(2)?.into()]
            };
            let value = harborvm::guest::execute(&store, id, &argv, 60)?;
            println!("{}", serde_json::to_string_pretty(&value)?);
            if value["ok"] != true {
                std::process::exit(1);
            }
            return Ok(());
        }
        ("chat-json", id) => {
            let input = harborvm::assistant_api::read_input(std::io::stdin().lock())?;
            harborvm::assistant_api::chat(&store, id, input)?;
            return Ok(());
        }
        ("assistant", "models" | "models-json") => {
            let input = if args[1] == "models-json" {
                harborvm::assistant_api::read_input(std::io::stdin().lock())?
            } else {
                harborvm::assistant_api::ModelInput::default()
            };
            harborvm::assistant_api::models(&store, arg(2)?, input)?
        }
        ("chat", id) => {
            harborvm::assistant::run(&store, id, arg(2)?)?;
            return Ok(());
        }
        ("screenshot", id) => {
            let action = if args.get(3).map(String::as_str) == Some("--app") {
                "app-screenshot"
            } else {
                "screenshot"
            };
            let response = harborvm::bridge::request(&store, id, action, json!({}))?;
            let encoded = response["png_base64"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing screenshot data"))?;
            use std::{
                io::Write,
                process::{Command, Stdio},
            };
            let destination = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(arg(2)?)?;
            let mut process = Command::new("/usr/bin/base64")
                .arg("-D")
                .stdin(Stdio::piped())
                .stdout(destination)
                .spawn()?;
            process
                .stdin
                .take()
                .unwrap()
                .write_all(encoded.as_bytes())?;
            if !process.wait()?.success() {
                bail!("Screenshot decoding failed");
            }
            json!({"ok":true,"path":arg(2)?})
        }
        ("input", id) => {
            let action = arg(2)?;
            let payload = match action {
                "click" => json!({"x":arg(3)?.parse::<f64>()?,"y":arg(4)?.parse::<f64>()?}),
                "key" => json!({"key":arg(3)?}),
                "text" => json!({"text":arg(3)?}),
                _ => bail!("Input must be click, key, or text"),
            };
            harborvm::bridge::request(&store, id, action, payload)?
        }
        ("start" | "pause" | "resume" | "stop" | "poweroff" | "select" | "refresh", id) => {
            store.load(id)?;
            if args[0] == "start" {
                harborvm::bridge::launch(&store, false)?;
            }
            harborvm::bridge::request(&store, id, &args[0], json!({}))?
        }
        ("status", id) => {
            let profile = store.load(id)?;
            let mut value = if !store.root.join("control.sock").exists() {
                json!({"ok":true,"profile":id,"name":profile.name,"state":"stopped","ready":profile.ready})
            } else {
                harborvm::bridge::request(&store, id, "status", json!({}))?
            };
            value["guest_online"] = harborvm::guest::online(&store, id).into();
            value
        }
        ("profile", "list") => json!(store.list()?),
        ("profile", "show") => store.descriptor(arg(2)?)?,
        ("profile", "assistant") => {
            let mut profile = store.load(arg(2)?)?;
            profile.assistant_program = Some(arg(3)?.to_string());
            store.save(&profile)?;
            store.descriptor(&profile.id)?
        }
        ("profile", "connection") => {
            use harborvm::assistant_api::{Connection, Provider};
            let provider = match arg(3)? {
                "codex" => Provider::Codex,
                "ollama" => Provider::Ollama,
                "openai-compatible" => Provider::OpenaiCompatible,
                _ => bail!("Provider must be codex, ollama, or openai-compatible"),
            };
            let mut connection = Connection {
                provider,
                ..Connection::default()
            };
            let mut i = 4;
            while i < args.len() {
                match arg(i)? {
                    "--endpoint" => connection.endpoint = Some(arg(i + 1)?.into()),
                    "--model" => connection.model = Some(arg(i + 1)?.into()),
                    "--key-env" => connection.key_env = Some(arg(i + 1)?.into()),
                    _ => bail!("Unknown connection option"),
                }
                i += 2;
            }
            connection.validate()?;
            let mut profile = store.load(arg(2)?)?;
            profile.assistant_connection = Some(connection);
            store.save(&profile)?;
            store.descriptor(&profile.id)?
        }
        ("profile", "create") => {
            let id = arg(2)?;
            let mut name = id.to_string();
            let mut cpus = 2;
            let mut memory = 2048;
            let mut memory_set = false;
            let mut disk = None;
            let mut guest_os = GuestOS::Linux;
            let mut setup = SetupOptions::default();
            let mut i = 3;
            while i < args.len() {
                match arg(i)? {
                    "--name" => name = arg(i + 1)?.into(),
                    "--cpus" => cpus = arg(i + 1)?.parse()?,
                    "--memory" => {
                        memory = arg(i + 1)?.parse()?;
                        memory_set = true;
                    }
                    "--disk" => disk = Some(arg(i + 1)?.parse()?),
                    "--os" => {
                        guest_os = match arg(i + 1)? {
                            "linux" => GuestOS::Linux,
                            "macos" => GuestOS::Macos,
                            _ => bail!("Guest OS must be linux or macos"),
                        }
                    }
                    "--without" => {
                        for component in arg(i + 1)?.split(',') {
                            match component {
                                "desktop" => {
                                    setup.desktop = false;
                                    setup.browser = false;
                                    setup.playwright = false;
                                }
                                "browser" => {
                                    setup.browser = false;
                                    setup.playwright = false;
                                }
                                "python" => {
                                    setup.python = false;
                                    setup.playwright = false;
                                }
                                "node" => setup.node = false,
                                "playwright" => setup.playwright = false,
                                "all" => {
                                    setup = SetupOptions {
                                        desktop: false,
                                        browser: false,
                                        python: false,
                                        node: false,
                                        playwright: false,
                                    }
                                }
                                _ => bail!("Unknown setup component: {component}"),
                            }
                        }
                    }
                    other => bail!("Unknown option: {other}"),
                };
                i += 2;
            }
            if guest_os == GuestOS::Macos && !memory_set {
                memory = 4096;
            }
            let mut profile = Profile::new(id, &name, cpus, memory)?;
            profile.guest_os = guest_os;
            profile.disk_gib = disk.unwrap_or(if guest_os == GuestOS::Macos { 64 } else { 24 });
            profile.setup = setup;
            store.create(&profile)?;
            store.descriptor(id)?
        }
        _ => bail!("Unknown command; run harborvm help"),
    };
    print_json(&result)
}
fn print_json(value: &Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{}", json!({"ok":false,"error":format!("{error:#}")}));
        std::process::exit(1);
    }
}
