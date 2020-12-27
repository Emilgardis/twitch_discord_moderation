use structopt::StructOpt;
pub mod not_bash;

#[derive(Debug, StructOpt)]
#[structopt(name = "xtask", about = "xtask action for automating tasks")]
pub struct Opt {
    #[structopt(subcommand)]
    command: Command,
    #[structopt(long, default_value = "rpi", global = true)]
    target: Target,
    /// Build as a release target
    #[structopt(parse(from_flag), long, global = true)]
    release: CompileMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileMode {
    Debug,
    Release,
}

impl CompileMode {
    fn to_flag(&self) -> &'static str {
        match self {
            CompileMode::Debug => "",
            CompileMode::Release => "--release",
        }
    }
}

impl From<bool> for CompileMode {
    fn from(flag: bool) -> Self {
        match flag {
            true => CompileMode::Release,
            false => CompileMode::Debug,
        }
    }
}
#[derive(Debug, StructOpt)]
enum Command {
    /// Build the crate
    Build(Build),
    /// Check the crate
    Check(Check),
    /// Build and push the crate to remote
    Push(Push),
    /// Build docker image
    BuildDocker(BuildDocker),
}

#[derive(Debug, StructOpt)]
pub struct BuildDocker {}
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Target {
    Rpi,
    Local,
}

impl Target {
    pub fn to_triple(&self) -> String {
        match self {
            Target::Rpi => "armv7-unknown-linux-gnueabihf".to_string(),
            Target::Local => get_default_target().unwrap(),
        }
    }
}

impl std::str::FromStr for Target {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.trim().to_lowercase().as_str() {
            "local" => Target::Local,
            "rpi" => Target::Rpi,
            _ => return Err("no such target"),
        })
    }
}

#[derive(Debug, StructOpt)]
pub struct Build {
    #[structopt(default_value = "twitch-discord-moderation")]
    binary: String,
}

impl Build {
    pub fn new(binary: String) -> Self { Build { binary } }
}

impl Default for Build {
    fn default() -> Self { Build::new(String::from("twitch-discord-moderation")) }
}
#[derive(Debug, StructOpt)]
pub struct Check {}

#[derive(Debug, StructOpt)]
pub struct Push {
    /// Where to push the binary
    pub target_scp: Option<String>,
}
fn main() -> anyhow::Result<()> {
    let opt = Opt::from_args();
    let _e = if opt.target == Target::Rpi {
        dbg!("doing thing");
        not_bash::pushenv("RUSTC_WRAPPER", "") // unset RUSTC_WRAPPER
    } else {
        not_bash::pushenv("__A", "")
    };
    match &opt.command {
        Command::Build(b) => build(&opt, &b)?,
        Command::Check(c) => check(&opt, &c)?,
        Command::Push(p) => push(&opt, &p)?,
        Command::BuildDocker(_) => {
            run!("docker build -t local/openssl-rpi1:tag .")?;
        }
    }
    Ok(())
}

pub fn build_json(opt: &Opt, build: &Build) -> anyhow::Result<String> {
    Ok(if opt.target == Target::Rpi {
        run!(
            "cross build -v --message-format=json --format-version=2 --target armv7-unknown-linux-gnueabihf --bin {} {}",
            build.binary,
            opt.release.to_flag() ;
            echo = false
        )?
    } else {
        run!(
            "cargo build -v --message-format=json --format-version=2 --bin {} {}",
            build.binary,
            opt.release.to_flag() ;
            echo = false
        )?
    })
}

pub fn build(opt: &Opt, build: &Build) -> anyhow::Result<()> {
    if opt.target == Target::Rpi {
        run!(
            "cross build -v --target armv7-unknown-linux-gnueabihf --bin {} {}",
            build.binary,
            opt.release.to_flag()
        )?;
    } else {
        run!(
            "cargo build -v --bin {} {}",
            build.binary,
            opt.release.to_flag()
        )?;
    }
    Ok(())
}

pub fn check(opt: &Opt, _: &Check) -> anyhow::Result<()> {
    if opt.target == Target::Rpi {
        run!(
            "cross check --target armv7-unknown-linux-gnueabihf {}",
            opt.release.to_flag()
        )?;
    } else {
        run!("cargo check {}", opt.release.to_flag())?;
    }
    Ok(())
}

pub fn push(opt: &Opt, _: &Push) -> anyhow::Result<()> {
    //build(opt, &Build::default())?;
    //let binary = dbg!(compile_and_find_binary(&opt.target, opt))?;
    let root = get_workspace_root(&Target::Local)?;
    run!(
        "rsync -av -e ssh --exclude='target/' --exclude='.cargo/' --exclude='docker/' {}/server alarm@rpi1:/home/alarm/docker/twitch-discord-moderation-log/",
        root.display()
    )?;
    Ok(())
}

pub fn compile_and_find_binary(
    target: &Target,
    opt: &Opt,
) -> anyhow::Result<Option<std::path::PathBuf>> {
    let mut path = dbg!(get_workspace_root(target))?;
    match target {
        Target::Rpi => {
            path.push(target.to_triple());
        }
        Target::Local => {}
    }
    match opt.release {
        CompileMode::Debug => {
            path.push("debug");
        }
        CompileMode::Release => {
            path.push("release");
        }
    }
    let b = build_json(opt, &Build::default())?;
    if let Some(p) = b.lines().rev().nth(1) {
        let r = json::parse(p)?;
        if let json::JsonValue::Object(obj) = r {
            if let Some(exe) = obj.get("executable") {
                if let Some(s) = exe.as_str() {
                    path.extend(std::path::Path::new(s));
                    return Ok(Some(path));
                }
            } else {
                return Ok(None);
            }
        }
    }
    Err(anyhow::anyhow!("oops"))
}

fn get_default_target() -> anyhow::Result<String> {
    let output = run!("rustc -Vv" ; echo = false )?;

    for line in output.lines() {
        if line.starts_with("host:") {
            return Ok(line[6..].to_owned());
        }
    }

    Err(anyhow::anyhow!("rustc failed"))
}

fn get_workspace_root(target: &Target) -> anyhow::Result<std::path::PathBuf> {
    let stdout = match target {
        Target::Rpi => {
            run!("cross metadata -- --target armv7-unknown-linux-gnueabihf" ; echo = false)?
        }
        Target::Local => run!("cargo metadata" ; echo = false)?,
    };

    let meta = json::parse(&stdout)?;
    let root = meta["workspace_root"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("no workspace root found"))?;
    Ok(root.to_string().into())
}
