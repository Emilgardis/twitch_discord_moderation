use std::time::Instant;

use xshell::{cmd, Shell};

fn main() -> xshell::Result<()> {
    let sh = Shell::new()?;

    {
        let _s = section("BUILD");
        cmd!(sh, "cargo test --workspace --no-run").run()?;
    }

    {
        let _s = section("CLIPPY");
        cmd!(
            sh,
            "cargo clippy --all-targets --no-default-features -- -D warnings"
        )
        .run()?;
        cmd!(
            sh,
            "cargo clippy --all-targets --all-features -- -D warnings"
        )
        .run()?;
    }

    {
        let _s = section("TEST");
        cmd!(sh, "cargo test --workspace -- --nocapture").run()?;
    }

    {
        let _s = section("TAG");

        let version = cmd!(sh, "cargo pkgid")
            .read()?
            .rsplit_once('@')
            .unwrap()
            .1
            .trim()
            .to_string();
        let tag = format!("v{version}");

        let has_tag = cmd!(sh, "git tag --list")
            .read()?
            .lines()
            .any(|it| it.trim() == tag);
        if !has_tag {
            let current_branch = cmd!(sh, "git branch --show-current").read()?;
            let dry_run = sh.var("CI").is_err() || current_branch != "main";
            eprintln!("Taging!{}!", if dry_run { " (dry run)" } else { "" });

            if dry_run {
                eprintln!("{}", cmd!(sh, "git tag {tag}"));
                eprintln!("{}", cmd!(sh, "git push --tags"));
            } else {
                cmd!(sh, "git tag {tag}").run()?;
                cmd!(sh, "git push origin {tag}").run()?;
            }
        }
    }
    Ok(())
}

fn section(name: &'static str) -> impl Drop {
    println!("::group::{name}");
    let start = Instant::now();
    defer(move || {
        let elapsed = start.elapsed();
        eprintln!("{name}: {elapsed:.2?}");
        println!("::endgroup::");
    })
}

fn defer<F: FnOnce()>(f: F) -> impl Drop {
    struct D<F: FnOnce()>(Option<F>);
    impl<F: FnOnce()> Drop for D<F> {
        fn drop(&mut self) {
            if let Some(f) = self.0.take() {
                f()
            }
        }
    }
    D(Some(f))
}
