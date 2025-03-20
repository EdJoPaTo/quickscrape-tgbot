use std::fmt::Write as _;

use anyhow::Context as _;
use frankenstein::TelegramApi as _;
use frankenstein::client_ureq::Bot;
use frankenstein::methods::SendMessageParams;
use frankenstein::types::{LinkPreviewOptions, ReplyParameters};
use ureq::ResponseExt as _;
use ureq::http::{HeaderName, header};

mod http;
mod macros;
mod single;
mod telegram;
mod tiktok;

const INTERESTING_HEADERS: &[HeaderName] = &[
    header::CACHE_CONTROL,
    header::CONTENT_LENGTH,
    header::CONTENT_TYPE,
    header::EXPIRES,
    header::LAST_MODIFIED,
    header::SERVER,
];

fn main() {
    let tg = telegram::Telegram::new();
    tg.start_polling_loop(inspect_url);
}

fn inspect_url(
    bot: &Bot,
    chat_id: i64,
    reply_params: &ReplyParameters,
    url: &str,
) -> anyhow::Result<()> {
    println!("inspect_url {chat_id:>10}: {url}");
    let mut response = http::get(url).context("HTTP GET request failed")?;
    let body = response.body_mut().read_to_string();

    let target_uri = response.get_uri();

    let mut meta = String::new();
    if let Some(history) = response
        .get_redirect_history()
        .filter(|history| history.len() > 1)
    {
        writeln!(meta, "\nRedirect history:").unwrap();
        for step in history {
            writeln!(meta, "- {step}").unwrap();
        }
    }
    if let Some(query) = target_uri.query() {
        let without_query = target_uri.to_string().replace(&format!("?{query}"), "");
        writeln!(meta, "\nWithout query: {without_query}").unwrap();
    }
    match &body {
        Ok(body) => writeln!(meta, "\nBody is a string with length {}", body.len()).unwrap(),
        Err(error) => writeln!(meta, "\nBody is not a string: {error:#}").unwrap(),
    }
    bot.send_message(
        &SendMessageParams::builder()
            .link_preview_options(LinkPreviewOptions::DISABLED)
            .chat_id(chat_id)
            .reply_parameters(reply_params.clone())
            .text(meta)
            .build(),
    )?;

    let mut partial = format!("{:?} {}\n", response.version(), response.status());
    for (key, value) in response.headers() {
        let header = value.to_str().map_or_else(
            |_| format!("{key}: {value:?}"),
            |value| {
                if value.len() <= 30 || INTERESTING_HEADERS.contains(key) {
                    format!("{key}: {value}")
                } else {
                    format!("{key}: <omitted>")
                }
            },
        );

        // 4096 + safety
        if partial.len().saturating_add(header.len()) > 4090 {
            telegram::send_code(bot, chat_id, reply_params, None, Some("http"), &partial)?;
            partial.clear();
        }
        partial += &header;
        partial += if header.len() > 45 { "\n\n" } else { "\n" };
    }
    if !partial.trim().is_empty() {
        telegram::send_code(bot, chat_id, reply_params, None, Some("http"), &partial)?;
    }
    drop(partial);

    let host = target_uri.host().context("Target URI should have a host")?;

    let Ok(body) = body else {
        return Ok(());
    };

    if host.ends_with("tiktok.com") {
        tiktok::analyze(bot, chat_id, reply_params, &body).context("tiktok::analyze")?;
    }

    Ok(())
}
