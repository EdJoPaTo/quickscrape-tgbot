use anyhow::Context as _;
use frankenstein::TelegramApi as _;
use frankenstein::methods::{
    DeleteMessageParams, EditMessageTextParams, SendChatActionParams, SendDocumentParams,
    SendMessageParams, SendVideoParams,
};
use frankenstein::types::ChatAction;

use crate::ffmpeg::VideoStats;

#[expect(clippy::too_many_lines)]
pub fn analyze(
    bot: &frankenstein::client_ureq::Bot,
    chat_id: i64,
    reply_params: &frankenstein::types::ReplyParameters,
    url: &str,
) -> anyhow::Result<()> {
    let tempdir = tempfile::tempdir().expect("Should be able to create tempdir");

    let start_message = bot
        .send_message(
            &SendMessageParams::builder()
                .chat_id(chat_id)
                .reply_parameters(reply_params.clone())
                .text("Start yt-dlp…")
                .build(),
        )?
        .result
        .message_id;

    let output = std::process::Command::new("yt-dlp")
        .current_dir(tempdir.path())
        .arg("--write-description")
        .arg("--embed-chapters")
        .arg("--embed-metadata")
        .arg("--embed-subs")
        .arg("--sub-langs=all")
        .arg("--sponsorblock-remove=default")
        .arg("--no-progress")
        .arg("--no-playlist")
        .arg("--restrict-filenames")
        .arg("--trim-filenames=80")
        .arg("--format-sort=vcodec:h264,+size,+br,+res,+fps")
        .arg(url)
        .output()
        .expect("Should be able to spawn yt-dlp");

    for entry in std::fs::read_dir(tempdir.path()).expect("Should be able to read tempdir") {
        let entry = entry.expect("Should be able to read file in tempdir");
        let path = entry.path();
        let filesize = entry
            .metadata()
            .expect("Should be able to read file metadata")
            .len();
        if filesize == 0 {
            eprintln!("Skip 0 byte sized file: {}", path.display());
            continue;
        }

        if path.extension().and_then(std::ffi::OsStr::to_str) == Some("description") {
            if let Some(description) = std::fs::read_to_string(&path)
                .ok()
                .filter(|description| description.encode_utf16().count() < 4000)
            {
                crate::telegram::send_expandable_blockquote_without_linkpreview(
                    bot,
                    chat_id,
                    reply_params,
                    &description,
                )
                .context("send description as blockquote")?;
            } else {
                bot.send_document(
                    &SendDocumentParams::builder()
                        .chat_id(chat_id)
                        .reply_parameters(reply_params.clone())
                        .document(path)
                        .build(),
                )
                .context("send description as document")?;
            }
            continue;
        }

        bot.send_chat_action(
            &SendChatActionParams::builder()
                .chat_id(chat_id)
                .action(ChatAction::UploadVideo)
                .build(),
        )?;

        let stats = match VideoStats::load(&path) {
            Ok(stats) => stats,
            Err(err) => {
                bot.send_message(
                    &SendMessageParams::builder()
                        .chat_id(chat_id)
                        .reply_parameters(reply_params.clone())
                        .text(format!(
                            "Failed to get video stats from: {} {err}",
                            path.display()
                        ))
                        .build(),
                )?;

                continue;
            }
        };

        if let Err(error) = bot.send_video(
            &SendVideoParams::builder()
                .chat_id(chat_id)
                .reply_parameters(reply_params.clone())
                .width(stats.width)
                .height(stats.height)
                .duration(stats.duration)
                .video(path)
                .build(),
        ) {
            bot.send_message(
                &SendMessageParams::builder()
                    .chat_id(chat_id)
                    .reply_parameters(reply_params.clone())
                    .text(format!("Failed to send_video from yt-dlp output: {error}"))
                    .build(),
            )?;
        }
    }

    if output.status.success() {
        bot.delete_message(
            &DeleteMessageParams::builder()
                .chat_id(chat_id)
                .message_id(start_message)
                .build(),
        )
        .context("delete start message")?;
    } else {
        bot.edit_message_text(
            &EditMessageTextParams::builder()
                .chat_id(chat_id)
                .message_id(start_message)
                .text(format!("yt-dlp {}", output.status))
                .build(),
        )
        .context("edit start message into error status")?;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    crate::telegram::send_stdout_stderr(bot, chat_id, reply_params, "yt-dlp", &stdout, &stderr)?;

    Ok(())
}
