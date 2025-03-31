use frankenstein::TelegramApi;
use frankenstein::methods::{
    DeleteMessageParams, EditMessageTextParams, SendChatActionParams, SendMessageParams,
    SendVideoParams,
};
use frankenstein::types::ChatAction;

use crate::ffmpeg::VideoStats;

#[expect(clippy::too_many_lines)]
pub fn send_video(
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
        .arg("--embed-chapters")
        .arg("--embed-metadata")
        .arg("--embed-subs")
        .arg("--sub-langs=all")
        .arg("--sponsorblock-remove=default")
        .arg("--paths=temp:/tmp/yt-dlp")
        .arg("--no-progress")
        .arg("--no-playlist")
        .arg("--restrict-filenames")
        .arg("--trim-filenames=80")
        .arg("--format-sort=vcodec:h264,+size,+br,+res,+fps")
        .arg(url)
        .output()
        .expect("Should be able to spawn yt-dlp");

    for entry in std::fs::read_dir(tempdir.path()).expect("Should be able to read tempdir") {
        let path = entry
            .expect("Should be able to read file in tempdir")
            .path();

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
        )?;
    } else {
        bot.edit_message_text(
            &EditMessageTextParams::builder()
                .chat_id(chat_id)
                .message_id(start_message)
                .text(format!("yt-dlp {}", output.status))
                .build(),
        )?;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.is_empty() {
        crate::telegram::send_code(
            bot,
            chat_id,
            reply_params,
            Some("yt-dlp stdout"),
            Some("plaintext"),
            &stdout,
        )?;
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.is_empty() {
        crate::telegram::send_code(
            bot,
            chat_id,
            reply_params,
            Some("yt-dlp stderr"),
            Some("plaintext"),
            &stderr,
        )?;
    }

    Ok(())
}
