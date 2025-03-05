use std::fmt::Write as _;

use anyhow::Context as _;
use frankenstein::api_params::SendMessageParams;
use frankenstein::client_ureq::Bot;
use frankenstein::objects::LinkPreviewOptions;
use frankenstein::{ReplyParameters, TelegramApi as _};
use ureq::ResponseExt as _;

mod http;
mod macros;
mod single;
mod telegram;
mod tiktok;

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

    let mut meta = format!("{:?}  {}\n", response.version(), response.status());
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
            .link_preview_options(LinkPreviewOptions::builder().is_disabled(true).build())
            .chat_id(chat_id)
            .reply_parameters(reply_params.clone())
            .text(meta)
            .build(),
    )?;

    let mut partial = String::new();
    for (key, value) in response.headers() {
        let header = value.to_str().map_or_else(
            |_| format!("{key}: {value:?}"),
            |value| format!("{key}: {value}"),
        );

        // 4096 + safety
        if partial.len().saturating_add(header.len()) > 4090 {
            bot.send_message(
                &SendMessageParams::builder()
                    .link_preview_options(LinkPreviewOptions::builder().is_disabled(true).build())
                    .chat_id(chat_id)
                    .reply_parameters(reply_params.clone())
                    .text(partial)
                    .build(),
            )?;
            partial = String::new();
        }
        partial += &header;
        partial += "\n\n";
    }
    bot.send_message(
        &SendMessageParams::builder()
            .link_preview_options(LinkPreviewOptions::builder().is_disabled(true).build())
            .chat_id(chat_id)
            .reply_parameters(reply_params.clone())
            .text(partial)
            .build(),
    )?;

    let host = target_uri.host().context("Target URI should have a host")?;

    let Ok(body) = body else {
        return Ok(());
    };

    if host.ends_with("tiktok.com") {
        tiktok::analyze(bot, chat_id, reply_params, &body).context("tiktok::analyze")?;
    }

    Ok(())
}
