use std::fs;
use std::io::Read;

use structopt::StructOpt;

mod config;
mod git;

#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
struct Options {
    commits: String,

    #[structopt(parse(from_os_str), default_value = ".")]
    work_tree: std::path::PathBuf,

    #[structopt(parse(from_os_str))]
    config: Option<std::path::PathBuf>,
}

fn load_toml(path: &std::path::Path) -> Result<config::Config, failure::Error> {
    let mut f = fs::File::open(path)?;
    let mut text = String::new();
    f.read_to_string(&mut text)?;
    toml::from_str(&text).map_err(|e| e.into())
}

fn check_subject_length(message: &str, max_length: usize) -> Result<(), failure::Error> {
    let subject = message
        .split('\n')
        .next()
        .ok_or_else(|| failure::Context::new("Commit cannot be empty"))?;
    let subject = subject.trim_end();
    let count = unicode_segmentation::UnicodeSegmentation::graphemes(subject, true).count();
    if max_length < count {
        failure::bail!(
            "Commit subject is {}, exceeding the max length of {}",
            count,
            max_length
        );
    }
    Ok(())
}

fn check_line_length(message: &str, max_length: usize) -> Result<(), failure::Error> {
    for line in message.split('\n') {
        let line = line.trim_end();
        let count = unicode_segmentation::UnicodeSegmentation::graphemes(line, true).count();
        if max_length < count {
            failure::bail!(
                "Commit line is {}, exceeding the max length of {}",
                count,
                max_length
            );
        }
    }
    Ok(())
}

fn check_capitalized_subject(subject: &str) -> Result<(), failure::Error> {
    let first = subject
        .chars()
        .next()
        .ok_or_else(|| failure::Context::new("Subject cannot be empty"))?;
    if !first.is_uppercase() {
        failure::bail!("Subject must be capitalized: `{}`", subject);
    }
    Ok(())
}

fn run() -> Result<i32, failure::Error> {
    let options = Options::from_args();

    let repo = options.work_tree.canonicalize()?;

    let repo = git2::Repository::discover(repo)?;
    let config = if let Some(config_path) = options.config.as_ref() {
        load_toml(config_path)?
    } else {
        let config_path = repo
            .workdir()
            .ok_or_else(|| failure::Context::new("Cannot work on bare repo"))?
            .join("committed.toml");
        if config_path.is_file() {
            load_toml(&config_path)?
        } else {
            config::Config::default()
        }
    };

    let revspec = git::RevSpec::parse(&repo, &options.commits)?;
    let style = config.style();
    let subject_length = config.subject_length();
    let line_length = config.line_length();
    let subject_capitalized = config.subject_capitalized();
    for commit in revspec.iter() {
        let message = commit.message().unwrap();
        match style {
            config::Style::Conventional => {
                let parsed = committed::conventional::Message::parse(message).unwrap();
                if subject_capitalized {
                    check_capitalized_subject(parsed.description)?;
                }
            }
            config::Style::None => {
                let parsed = committed::no_style::Message::parse(message).unwrap();
                if subject_capitalized {
                    check_capitalized_subject(parsed.subject)?;
                }
            }
        }
        if subject_length != 0 {
            check_subject_length(message, subject_length)?;
        }
        if line_length != 0 {
            check_line_length(message, line_length)?;
        }
    }

    Ok(0)
}

fn main() {
    let code = run().unwrap();
    std::process::exit(code);
}
