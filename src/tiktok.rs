use std::fmt::Write as _;

use anyhow::Context as _;
use chrono::{DateTime, Utc};
use frankenstein::TelegramApi as _;
use frankenstein::methods::{SendAudioParams, SendMessageParams, SendPhotoParams};
use scraper::Html;
use serde_json::Value;

use crate::macros::selector;
use crate::single::Single as _;

#[expect(clippy::too_many_lines)]
pub fn analyze(
    bot: &frankenstein::client_ureq::Bot,
    chat_id: i64,
    reply_params: &frankenstein::types::ReplyParameters,
    body: &str,
) -> anyhow::Result<()> {
    let json = extract_json(body)?;

    let share_desc = json
        .get("shareMeta")
        .and_then(|value| value.get("desc"))
        .and_then(Value::as_str);
    if let Some(share_desc) = share_desc {
        bot.send_message(
            &SendMessageParams::builder()
                .chat_id(chat_id)
                .reply_parameters(reply_params.clone())
                .text(format!("shareMeta.desc:\n\n{share_desc}"))
                .build(),
        )
        .context("Should be able to send share description")?;
    }

    let item = json
        .get("itemInfo")
        .context("Should have itemInfo")?
        .get("itemStruct")
        .context("Should have itemStruct")?;
    #[cfg(debug_assertions)]
    dbg!(item.as_object().unwrap().keys().collect::<Vec<_>>());

    let desc = item.get("desc").and_then(Value::as_str);
    if let Some(desc) = desc.filter(|desc| Some(*desc) != share_desc) {
        bot.send_message(
            &SendMessageParams::builder()
                .chat_id(chat_id)
                .reply_parameters(reply_params.clone())
                .text(format!("desc:\n\n{desc}"))
                .build(),
        )
        .context("Should be able to send description")?;
    }

    let contents = item
        .get("contents")
        .and_then(Value::as_array)
        .map(|contents| {
            contents
                .iter()
                .filter_map(|line| line.get("desc"))
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("\n")
        });
    if let Some(contents) = contents {
        bot.send_message(
            &SendMessageParams::builder()
                .chat_id(chat_id)
                .reply_parameters(reply_params.clone())
                .text(format!("contents:\n\n{contents}"))
                .build(),
        )
        .context("Should be able to send contents")?;
    }

    let mut times = String::new();
    if let Some(time) = item.get("createTime").and_then(parse_timestamp) {
        writeln!(times, "createTime: {time}").unwrap();
    }
    if let Some(time) = item.get("scheduleTime").and_then(parse_timestamp) {
        writeln!(times, "scheduleTime: {time}").unwrap();
    }
    if let Some(time) = item.get("takeDown").and_then(parse_timestamp) {
        // Not sure whether thats actually a timestamp
        writeln!(times, "takeDown: {time}").unwrap();
    }
    if !times.is_empty() {
        bot.send_message(
            &SendMessageParams::builder()
                .chat_id(chat_id)
                .reply_parameters(reply_params.clone())
                .text(times)
                .build(),
        )
        .context("Should be able to send times")?;
    }

    reply_json(bot, chat_id, reply_params, item, ["stats"])?;
    reply_json(bot, chat_id, reply_params, item, ["statsV2"])?;

    if let Some(author) = item.get("author").and_then(Value::as_object) {
        if let Some(avatar) = author.get("avatarLarger").and_then(Value::as_str) {
            bot.send_photo(
                &SendPhotoParams::builder()
                    .chat_id(chat_id)
                    .reply_parameters(reply_params.clone())
                    .photo(avatar.to_owned())
                    .caption("author")
                    .build(),
            )
            .expect("Should be able to send author avatar");
        }

        if let Some(signature) = author.get("signature").and_then(Value::as_str) {
            bot.send_message(
                &SendMessageParams::builder()
                    .chat_id(chat_id)
                    .reply_parameters(reply_params.clone())
                    .text(format!("author signature:\n\n{signature}"))
                    .build(),
            )
            .context("Should be able to send author signature")?;
        }
    }

    reply_json(bot, chat_id, reply_params, item, ["authorStats"])?;

    if let Some(music) = item.get("music").and_then(Value::as_object) {
        let mut caption = "Music behind video\n\n".to_owned();
        let title = music.get("title").and_then(Value::as_str);
        if let Some(title) = title {
            writeln!(caption, "title: {title}").unwrap();
        }
        let artist = music.get("authorName").and_then(Value::as_str);
        if let Some(artist) = artist {
            writeln!(caption, "authorName: {artist}").unwrap();
        }
        if let Some(album) = music.get("album").and_then(Value::as_str) {
            writeln!(caption, "album: {album}").unwrap();
        }

        if let Some(cover) = music.get("coverLarge").and_then(Value::as_str) {
            bot.send_photo(
                &SendPhotoParams::builder()
                    .chat_id(chat_id)
                    .reply_parameters(reply_params.clone())
                    .photo(cover.to_owned())
                    .caption(caption.clone())
                    .build(),
            )
            .expect("Should be able to send music cover");
        }

        if let Some(play_url) = music.get("playUrl").and_then(Value::as_str) {
            let duration = music
                .get("duration")
                .and_then(Value::as_i64)
                .and_then(|duration| duration.try_into().ok());
            bot.send_audio(
                &SendAudioParams::builder()
                    .chat_id(chat_id)
                    .reply_parameters(reply_params.clone())
                    .audio(play_url.to_owned())
                    .maybe_duration(duration)
                    .maybe_performer(artist)
                    .maybe_title(title)
                    .caption(caption)
                    .build(),
            )
            .context("Should be able to send music")?;
        }
    }

    reply_json(bot, chat_id, reply_params, item, ["video", "subtitleInfos"])?;
    reply_json(bot, chat_id, reply_params, item, ["contentLocation"])?;
    reply_json(bot, chat_id, reply_params, item, ["poi"])?;
    reply_json(bot, chat_id, reply_params, item, ["warnInfo"])?;

    Ok(())
}

fn reply_json<const N: usize>(
    bot: &frankenstein::client_ureq::Bot,
    chat_id: i64,
    reply_params: &frankenstein::types::ReplyParameters,
    json: &Value,
    keys: [&'static str; N],
) -> anyhow::Result<()> {
    let header = keys.join(".");
    if let Some(json) =
        deep_get(json, keys).and_then(|value| serde_json::to_string_pretty(value).ok())
    {
        if let Err(error) = crate::telegram::send_code(
            bot,
            chat_id,
            reply_params,
            Some(&header),
            Some("json"),
            &json,
        ) {
            let text = format!("failed to send_json_message {header}: {error:#}");
            let params = SendMessageParams::builder()
                .chat_id(chat_id)
                .reply_parameters(reply_params.clone())
                .text(text)
                .build();
            bot.send_message(&params)
                .context("Should be able to send send_json_message error to user")?;
        }
    } else {
        eprintln!("tiktok send_json_message key not there: {header}");
    }
    Ok(())
}

fn deep_get<'json, const N: usize>(
    mut json: &'json Value,
    keys: [&'static str; N],
) -> Option<&'json Value> {
    for key in keys {
        json = json.get(key)?;
    }
    Some(json)
}

fn parse_timestamp(value: &Value) -> Option<DateTime<Utc>> {
    let unix_timestamp = match value {
        Value::Number(number) => number.as_i64(),
        Value::String(str) => str.parse().ok(),
        _ => None,
    }?;
    if unix_timestamp == 0 {
        return None;
    }
    DateTime::from_timestamp(unix_timestamp, 0)
}

fn extract_json(html: &str) -> anyhow::Result<Value> {
    Html::parse_document(html)
        .select(selector!(r#"body script[type="application/json"]"#))
        .filter_map(|element| serde_json::from_str::<Value>(element.inner_html().trim()).ok())
        .filter_map(|mut value| value.get_mut("__DEFAULT_SCOPE__").map(Value::take))
        .filter_map(|mut value| value.get_mut("webapp.video-detail").map(Value::take))
        .single()
        .context("Should have a single relevant json script in body")
}

#[test]
fn znd_has_parseable_json() {
    let html = include_str!("../test/tiktok-ZNdJnFdqC.html");
    let json = extract_json(html).unwrap();
    let json = serde_json::to_string_pretty(&json).unwrap();
    println!("json character length: {}", json.len());
    // println!("{json}");
    // todo!();
}

#[test]
fn parse_timestamp_works() {
    let expected = chrono::NaiveDate::from_ymd_opt(2025, 1, 19)
        .unwrap()
        .and_hms_opt(16, 53, 21)
        .unwrap()
        .and_utc();
    assert_eq!(parse_timestamp(&0.into()), None);
    assert_eq!(parse_timestamp(&Value::String("0".to_owned())), None);
    assert_eq!(parse_timestamp(&1_737_305_601.into()), Some(expected));
    assert_eq!(
        parse_timestamp(&Value::String("1737305601".to_owned())),
        Some(expected)
    );
}
