use vergen::{vergen, Config};

fn main() -> anyhow::Result<()> {
    let mut c = Config::default();
    *c.git_mut().sha_kind_mut() = vergen::ShaKind::Short;
    *c.git_mut().sha_mut() = true;
    vergen(c)?;
    println!("cargo:rustc-env=GIT_SHA={}", git_version::git_version!());
    Ok(())
}
