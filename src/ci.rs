use std::{
    fmt::Write,
    fs,
    io::{BufReader, Read},
    path::PathBuf,
};

use clap::Parser;
use pretty_assertions::assert_eq;

static MANIFEST_PATH: &str = env!("CARGO_MANIFEST_DIR");

fn assert_lines_contains<'a>(iter: impl Iterator<Item = &'a str>, items: &[&str]) -> bool {
    if items.is_empty() {
        return false;
    }
    let mut index = 0;

    for iter_item in iter {
        if iter_item == items[index] {
            index += 1;
            if index == items.len() {
                return true;
            }
        }
    }

    panic!("iterator does not contain {:?}", items[index]);
}

#[test]
fn readme_help_usage() {
    let opts = crate::Opts::try_parse_from(&["twitch-discord-moderation", "--help"]);
    let mut usage_help = String::new();
    write!(usage_help, "```text\n{}\n```\n", opts.unwrap_err()).unwrap();
    usage_help = usage_help
        .lines()
        .map(|s| s.trim_end().to_owned() + "\n")
        .collect();
    println!("{:?}", usage_help);
    let mut readme = String::new();
    BufReader::new(
        fs::File::open(PathBuf::from(MANIFEST_PATH).join("README.md"))
            .expect("could not open README.md"),
    )
    .read_to_string(&mut readme)
    .expect("can't read README.md");

    assert_lines_contains(
        readme.clone().lines(),
        &[
            "<!--BEGIN commandline options-->",
            "<!--END commandline options-->",
        ],
    );

    let readme_usage_help: String = readme
        .split_inclusive('\n')
        .skip_while(|line| !line.starts_with("<!--BEGIN commandline options-->"))
        .skip(1)
        .take_while(|line| !line.starts_with("<!--END commandline options-->"))
        .collect();
    assert_eq!(usage_help, readme_usage_help);
}
