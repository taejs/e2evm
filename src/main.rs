use harborvm::{Result, bail, profile::{Profile, Store}};
use serde_json::{Value,json};
use std::path::Path;

fn help() {
    println!("HarborVM — local VM workspace\n\n  app                                  Open the desktop workspace\n  profile list | show ID               Inspect saved machines\n  profile create ID [--name NAME] [--cpus N] [--memory MIB]\n  profile assistant ID PROGRAM         Choose the local Codex executable\n  prepare ID [--cache DIRECTORY]        Prepare fresh Ubuntu ARM64\n  start | status | pause | resume ID    Control a machine\n  stop ID                              Request graceful shutdown\n  poweroff ID                          Force stop; unsaved work may be lost\n  exec ID -- PROGRAM [ARG...]           Run a guest command (60 s timeout)\n  run ID SHELL-LINE                     Run a shell line inside the guest\n  chat ID PROMPT                        Ask your local Codex CLI\n  screenshot ID OUTPUT.png [--app]      Capture the guest or workspace\n  input ID click X Y | key KEY | text TEXT\n\nHARBORVM_HOME selects the private data directory. HARBORVM_APP selects the native app.\nGuest commands never fall back to host execution.");
}
fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || matches!(args[0].as_str(), "help" | "--help" | "-h") { help(); return Ok(()); }
    if args[0]=="--version" {println!("harborvm {}",env!("CARGO_PKG_VERSION"));return Ok(());}
    let store = Store::open(Store::default_root()?)?;
    let arg = |i: usize| args.get(i).map(String::as_str).ok_or_else(|| anyhow::anyhow!("Missing argument; run harborvm help"));
    if args[0]=="app" {harborvm::bridge::launch(&store)?; println!("{{\"ok\":true}}"); return Ok(());}
    let result = match (arg(0)?, arg(1)?) {
        ("prepare", id) => {
            let cache = if args.len()==4 && arg(2)?=="--cache" {Some(Path::new(arg(3)?))} else if args.len()==2 {None} else {bail!("Usage: prepare ID [--cache DIRECTORY]");};
            harborvm::provision::prepare(&store,id,cache)?;
            store.descriptor(id)?
        }
        ("exec" | "run", id) => {
            let argv = if args[0]=="exec" { if arg(2)?!="--" {bail!("Usage: exec ID -- PROGRAM [ARG...]");} args[3..].to_vec() } else {vec!["/bin/sh".into(),"-lc".into(),arg(2)?.into()]};
            let value=harborvm::guest::execute(&store,id,&argv,60)?;
            println!("{}",serde_json::to_string_pretty(&value)?);
            if value["ok"]!=true {std::process::exit(1);} return Ok(());
        }
        ("chat", id) => {harborvm::assistant::run(&store,id,arg(2)?)?; return Ok(());}
        ("screenshot", id) => {
            let action=if args.get(3).map(String::as_str)==Some("--app") {"app-screenshot"} else {"screenshot"};
            let response=harborvm::bridge::request(&store,id,action,json!({}))?;
            let encoded=response["png_base64"].as_str().ok_or_else(|| anyhow::anyhow!("Missing screenshot data"))?;
            // The platform's decoder avoids another runtime dependency.
            use std::{io::Write,process::{Command,Stdio}};
            let destination=std::fs::OpenOptions::new().write(true).create_new(true).open(arg(2)?)?;
            let mut process=Command::new("/usr/bin/base64").arg("-D").stdin(Stdio::piped()).stdout(destination).spawn()?;
            process.stdin.take().unwrap().write_all(encoded.as_bytes())?;
            if !process.wait()?.success() {bail!("Screenshot decoding failed");}
            json!({"ok":true,"path":arg(2)?})
        }
        ("input",id) => {
            let action=arg(2)?;
            let payload=match action {"click"=>json!({"x":arg(3)?.parse::<f64>()?,"y":arg(4)?.parse::<f64>()?}),"key"=>json!({"key":arg(3)?}),"text"=>json!({"text":arg(3)?}),_=>bail!("Input must be click, key, or text")};
            harborvm::bridge::request(&store,id,action,payload)?
        }
        ("start"|"pause"|"resume"|"stop"|"poweroff"|"select"|"refresh",id) => {
            store.load(id)?;
            if args[0]=="start" {harborvm::bridge::launch(&store)?;}
            harborvm::bridge::request(&store,id,&args[0],json!({}))?
        }
        ("status",id) => {
            let profile=store.load(id)?;
            let mut value=if !store.root.join("control.sock").exists() {json!({"ok":true,"profile":id,"name":profile.name,"state":"stopped","ready":profile.ready})}else{harborvm::bridge::request(&store,id,"status",json!({}))?};
            value["guest_online"]=harborvm::guest::online(&store,id).into(); value
        }
        ("profile", "list") => json!(store.list()?),
        ("profile", "show") => store.descriptor(arg(2)?)?,
        ("profile", "assistant") => {
            let mut profile=store.load(arg(2)?)?; profile.assistant_program=Some(arg(3)?.to_string()); store.save(&profile)?; store.descriptor(&profile.id)?
        }
        ("profile", "create") => {
            let id=arg(2)?; let mut name=id.to_string(); let mut cpus=2; let mut memory=2048; let mut i=3;
            while i<args.len() {match arg(i)? {"--name"=>name=arg(i+1)?.into(),"--cpus"=>cpus=arg(i+1)?.parse()?,"--memory"=>memory=arg(i+1)?.parse()?,other=>bail!("Unknown option: {other}")};i+=2;}
            store.create(&Profile::new(id,&name,cpus,memory)?)?; store.descriptor(id)?
        }
        _=>bail!("Unknown command; run harborvm help"),
    };
    print_json(&result)
}
fn print_json(value:&Value)->Result<()> {println!("{}",serde_json::to_string_pretty(value)?);Ok(())}
fn main() {if let Err(error)=run() {eprintln!("{}",json!({"ok":false,"error":format!("{error:#}")}));std::process::exit(1);}}
