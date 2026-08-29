use std::io::{self, Write};

use crate::application::use_cases;
use crate::domain::video::VideoNumber;
use crate::infrastructure::{
    channel_resolver::resolve_channel,
    config::{add_channel, load_config},
    fallback_fetcher::FallbackFetcher,
    json_store::JsonVideoStore,
    mpv_player::MpvPlayer,
    rss_fetcher::RssFeedFetcher,
    shorts_checker::HttpShortsChecker,
    ytdlp_fetcher::YtDlpFetcher,
};

const HELP: &str = "Blepo — watch YouTube without ads, distractions, or tracking

Usage:
  blepo
  blepo add <CHANNEL>

CHANNEL can be an @handle, a YouTube channel URL, or a UC channel ID.

Examples:
  blepo add @Fireship
  blepo add https://www.youtube.com/@Fireship
  blepo add UCsBjURrPoezykLs9EqgamOA";

#[derive(Debug, PartialEq)]
enum CliCommand {
    Watch,
    Add(String),
    Help,
}

#[derive(Debug)]
struct CliError(String);

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CliError {}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    match parse_command(std::env::args().skip(1))? {
        CliCommand::Watch => run_watch(),
        CliCommand::Add(input) => run_add(&input),
        CliCommand::Help => {
            println!("{HELP}");
            Ok(())
        }
    }
}

fn parse_command(args: impl IntoIterator<Item = String>) -> Result<CliCommand, CliError> {
    let args: Vec<String> = args.into_iter().collect();
    match args.as_slice() {
        [] => Ok(CliCommand::Watch),
        [flag] if flag == "--help" || flag == "-h" || flag == "help" => Ok(CliCommand::Help),
        [command, flag] if command == "add" && (flag == "--help" || flag == "-h") => {
            Ok(CliCommand::Help)
        }
        [command, input] if command == "add" => Ok(CliCommand::Add(input.clone())),
        [command] if command == "add" => Err(CliError(format!(
            "missing channel for `blepo add`\n\n{HELP}"
        ))),
        [command, ..] if command == "add" => Err(CliError(format!(
            "`blepo add` accepts exactly one channel\n\n{HELP}"
        ))),
        [command, ..] => Err(CliError(format!("unknown command: {command}\n\n{HELP}"))),
    }
}

fn run_add(input: &str) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("Resolving channel...");
    let channel = resolve_channel(input)?;
    let path = add_channel(&channel)?;

    println!("Added {} ({})", channel.name, channel.id);
    println!("Config: {}", path.display());
    Ok(())
}

fn run_watch() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config()?;
    let store = JsonVideoStore::new(&config.data_dir)?;
    let fetcher = FallbackFetcher::new(RssFeedFetcher::new(), YtDlpFetcher::new());
    let shorts_checker = HttpShortsChecker::new();

    let videos = use_cases::fetch_videos(
        &config.channels,
        &fetcher,
        &store,
        &shorts_checker,
        config.fetch_window_days,
    )?;

    if videos.is_empty() {
        println!("No unwatched videos.");
        return Ok(());
    }

    for (i, video) in videos.iter().enumerate() {
        let date = video.published.format("%Y-%m-%d");
        println!(
            "{:>3}. [{}] {} — {}",
            i + 1,
            date,
            video.channel_name,
            video.title
        );
    }

    loop {
        print!("\nEnter number to play, w<number> to mark watched, wa to mark all watched, q to quit: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() || input == "q" {
            return Ok(());
        }

        if input == "wa" {
            use_cases::mark_all_as_watched(&videos, &store)?;
            return Ok(());
        }

        let (mark_only, num_str) = if let Some(rest) = input.strip_prefix('w') {
            (true, rest)
        } else {
            (false, input)
        };

        let number: usize = num_str
            .parse()
            .map_err(|_| format!("invalid number: {input}"))?;
        let number = VideoNumber::parse(number)?;

        let video = videos.get(number.to_index()).ok_or_else(|| {
            format!(
                "video #{} not found (have {} unwatched videos)",
                input,
                videos.len()
            )
        })?;

        if mark_only {
            use_cases::mark_as_watched(video, &store)?;
            println!(
                "Marked as watched: {} [{}]",
                video.title, video.channel_name
            );
        } else {
            let player = MpvPlayer::new()?;
            use_cases::mark_and_play(video, &store, &player)?;
            break;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<CliCommand, CliError> {
        parse_command(args.iter().map(|arg| arg.to_string()))
    }

    #[test]
    fn no_arguments_runs_watch_flow() {
        assert_eq!(parse(&[]).unwrap(), CliCommand::Watch);
    }

    #[test]
    fn parses_add_command() {
        assert_eq!(
            parse(&["add", "@Fireship"]).unwrap(),
            CliCommand::Add("@Fireship".to_string())
        );
    }

    #[test]
    fn parses_help_flags() {
        assert_eq!(parse(&["--help"]).unwrap(), CliCommand::Help);
        assert_eq!(parse(&["add", "--help"]).unwrap(), CliCommand::Help);
    }

    #[test]
    fn rejects_add_without_channel() {
        assert!(parse(&["add"]).is_err());
    }

    #[test]
    fn rejects_unknown_command() {
        assert!(parse(&["search", "Fireship"]).is_err());
    }
}
