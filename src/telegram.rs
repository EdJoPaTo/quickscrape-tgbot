use anyhow::Context as _;
use frankenstein::api_params::{GetUpdatesParams, SendMessageParams};
use frankenstein::client_ureq::Bot;
use frankenstein::objects::MessageEntityType;
use frankenstein::{
    ChatType, LeaveChatParams, LinkPreviewOptions, Message, MessageEntity, MethodResponse,
    ReplyParameters, TelegramApi as _, UpdateContent,
};

pub const LINK_PREVIEW_DISABLED: LinkPreviewOptions = LinkPreviewOptions {
    is_disabled: Some(true),
    url: None,
    prefer_small_media: None,
    prefer_large_media: None,
    show_above_text: None,
};

pub struct Telegram {
    bot: Bot,
    allowed_users: Vec<i64>,
}

impl Telegram {
    pub fn new() -> Self {
        let bot_token = std::env::var("BOT_TOKEN").expect("Should have BOT_TOKEN in environment");
        let allowed_users = std::env::var("USERS")
            .ok()
            .map(|var| {
                var.split(char::is_whitespace)
                    .filter_map(|user| user.parse().ok())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        dbg!(&allowed_users);
        if allowed_users.is_empty() {
            #[cfg(not(debug_assertions))]
            panic!("Should have USERS specified in environment");
            #[cfg(debug_assertions)]
            println!("Warning: USERS empty, allowing everyone!");
        }

        let bot = Bot::new(&bot_token);

        let me = bot
            .get_me()
            .expect("Should be able to get_me with the BOT_TOKEN");
        let username = me.result.username.expect("Bot should have a username");
        println!("Telegram Bot acts as @{username}");

        bot.delete_my_commands(&frankenstein::DeleteMyCommandsParams::builder().build())
            .expect("Should be able to delete_my_commands");

        Self { bot, allowed_users }
    }

    pub fn start_polling_loop<F>(&self, inspect_url: F) -> !
    where
        F: Fn(&Bot, i64, &ReplyParameters, &str) -> anyhow::Result<()>,
    {
        let mut get_updates_params = GetUpdatesParams::builder().timeout(300).build();
        loop {
            let updates = self
                .bot
                .get_updates(&get_updates_params)
                .expect("Should be able to get updates");
            for update in updates.result {
                get_updates_params.offset = Some(i64::from(update.update_id).saturating_add(1));
                match update.content {
                    UpdateContent::ChannelPost(message)
                    | UpdateContent::EditedChannelPost(message) => {
                        if matches!(message.chat.type_field, ChatType::Channel) {
                            self.leave_channel(message.chat.id);
                        }
                    }
                    UpdateContent::MyChatMember(chat_member_updated)
                    | UpdateContent::ChatMember(chat_member_updated) => {
                        if matches!(chat_member_updated.chat.type_field, ChatType::Channel) {
                            self.leave_channel(chat_member_updated.chat.id);
                        }
                    }
                    UpdateContent::Message(message) | UpdateContent::EditedMessage(message) => {
                        let chat_id = message.chat.id;
                        if !self.allowed_users.is_empty() && !self.allowed_users.contains(&chat_id)
                        {
                            let text = format!(
                                "This bot does not have any information about you ({chat_id}) and therefore doesn't serve you. If you think this is a mistake you need to message the admins about this yourself. This usage attempt is not stored."
                            );
                            let send_message_params = SendMessageParams::builder()
                                .chat_id(chat_id)
                                .text(text)
                                .build();
                            self.bot
                                .send_message(&send_message_params)
                                .expect("Should be able to respond to non allowed users");
                            continue;
                        }

                        if let Err(error) = self.analyze_message(&message, &inspect_url) {
                            self.bot
                                .send_message(
                                    &SendMessageParams::builder()
                                        .chat_id(message.chat.id)
                                        .reply_parameters(
                                            ReplyParameters::builder()
                                                .chat_id(message.chat.id)
                                                .message_id(message.message_id)
                                                .build(),
                                        )
                                        .text(format!("{error:?}"))
                                        .build(),
                                )
                                .expect(
                                    "Should be able to send user error while analyzing the message",
                                );
                        }
                    }
                    _ => {} // Ignore
                }
            }
        }
    }

    /// Leave channel and ignore errors (like not being part of the channel anymore)
    fn leave_channel(&self, chat_id: i64) {
        let send_message_params = SendMessageParams::builder()
        .chat_id(chat_id)
        .text("Adding a random bot as an admin to your channel is maybe not the best idea…\n\nSincerely, a random bot, added as an admin to this channel.'")
        .build();
        let _: Result<MethodResponse<_>, _> = self.bot.send_message(&send_message_params);
        let _: Result<MethodResponse<_>, _> = self.bot.leave_chat(&LeaveChatParams {
            chat_id: frankenstein::ChatId::Integer(chat_id),
        });
    }

    fn analyze_message<F>(&self, message: &Message, inspect_url: &F) -> anyhow::Result<()>
    where
        F: Fn(&Bot, i64, &ReplyParameters, &str) -> anyhow::Result<()>,
    {
        let urls = get_text_urls(message)?;
        anyhow::ensure!(!urls.is_empty(), "No url found in message");
        for url in urls {
            let reply_params = ReplyParameters::builder()
                .chat_id(message.chat.id)
                .message_id(message.message_id)
                .quote(url)
                .build();
            if let Err(error) = inspect_url(&self.bot, message.chat.id, &reply_params, url)
                .context("Failed to inspect url")
            {
                self.bot
                    .send_message(
                        &SendMessageParams::builder()
                            .chat_id(message.chat.id)
                            .reply_parameters(reply_params)
                            .text(format!("{error:?}"))
                            .build(),
                    )
                    .expect("Should be able to send error message back to user");
            }
        }
        Ok(())
    }
}

fn get_text_urls(message: &Message) -> anyhow::Result<Vec<&str>> {
    let (Some(text), Some(entities)) = (&message.text, &message.entities) else {
        return Ok(Vec::new());
    };
    let entities = entities
        .iter()
        .filter(|entity| matches!(entity.type_field, MessageEntityType::Url));
    let mut urls = Vec::new();
    for entity in entities {
        let start = entity.offset as usize;
        let length = entity.length as usize;
        let end = start.saturating_add(length);
        let url = text
            .get(start..end)
            .with_context(|| format!("There should be an url at the given entity: {entity:?}"))?;
        urls.push(url);
    }
    Ok(urls)
}

#[expect(clippy::cast_possible_truncation)]
pub fn send_code(
    bot: &Bot,
    chat_id: i64,
    reply_params: &ReplyParameters,
    header: &str,
    language: Option<&str>,
    code: &str,
) -> anyhow::Result<()> {
    let entity = MessageEntity::builder()
        .type_field(MessageEntityType::Pre)
        .maybe_language(language)
        .offset(header.encode_utf16().count().saturating_add(":\n".len()) as u16)
        .length(code.encode_utf16().count() as u16)
        .build();
    bot.send_message(
        &SendMessageParams::builder()
            .link_preview_options(LINK_PREVIEW_DISABLED)
            .chat_id(chat_id)
            .reply_parameters(reply_params.clone())
            .entities(vec![entity])
            .text(format!("{header}:\n{code}"))
            .build(),
    )
    .context("Should be able to send_message")?;
    Ok(())
}
