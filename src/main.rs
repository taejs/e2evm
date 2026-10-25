use harborvm::{Result, bail, profile::{Profile, Store}};
use serde_json::json;
fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || matches!(args[0].as_str(), "help" | "--help" | "-h") {
        println!("HarborVM — local VM workspace\n\nprofile list\nprofile show ID\nprofile create ID [--name NAME] [--cpus N] [--memory MIB]\n\nSet HARBORVM_HOME to choose the private data directory."); return Ok(());
    }
    let store = Store::open(Store::default_root()?)?;
    let arg = |i: usize| args.get(i).map(String::as_str).ok_or_else(|| anyhow::anyhow!("Missing argument; run harborvm help"));
    let result = match (arg(0)?, arg(1)?) {
        ("profile", "list") => json!(store.list()?),
        ("profile", "show") => store.descriptor(arg(2)?)?,
        ("profile", "create") => {
            let id = arg(2)?;
            let mut name = id.to_string(); let mut cpus = 2; let mut memory = 2048;
            let mut i = 3;
            while i < args.len() { match arg(i)? { "--name" => name = arg(i+1)?.into(), "--cpus" => cpus = arg(i+1)?.parse()?, "--memory" => memory = arg(i+1)?.parse()?, other => bail!("Unknown option: {other}") }; i+=2; }
            store.create(&Profile::new(id, &name, cpus, memory)?)?;
            store.descriptor(id)?
        }
        _ => bail!("Unknown command; run harborvm help"),
    };
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
fn main() {
    if let Err(error) = run() { eprintln!("{}", json!({"ok":false,"error":format!("{error:#}")})); std::process::exit(1); }
}
