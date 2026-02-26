use std::io::{self, Write};

use crate::application::use_cases;
use crate::domain::video::VideoNumber;
use crate::infrastructure::{
    config::load_config, fallback_fetcher::FallbackFetcher, json_store::JsonVideoStore,
    mpv_player::MpvPlayer, rss_fetcher::RssFeedFetcher, shorts_checker::HttpShortsChecker,
    ytdlp_fetcher::YtDlpFetcher,
};

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
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
        print!("\nEnter number to play, w<number> to mark watched, q to quit: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() || input == "q" {
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
