//! JSON for service actions, in the layout Telegram Desktop uses.

use crate::actions::*;
use crate::json_out::*;
use crate::model::*;

type Fields = Vec<(&'static str, J)>;

fn name_or_null(name: String) -> J {
    if name.is_empty() { J::Null } else { s(name) }
}

fn user_names(peers: &Peers, ids: &[i64]) -> J {
    // Like all arrays of this serializer, an empty one is written as `[\n]`.
    J::Arr(
        ids.iter()
            .map(|u| name_or_null(peers.name(PeerId::User(*u))))
            .collect(),
    )
}

fn int_array(values: &[i32]) -> J {
    let list: Vec<String> = values.iter().map(|v| v.to_string()).collect();
    J::Raw(format!("[{}]", list.join(",")))
}

fn credits_fields(v: &mut Fields, c: &Credits) {
    v.push(("price_amount_whole", s(c.whole.to_string())));
    v.push(("price_amount_nano", s(c.nano.to_string())));
    v.push(("price_currency", s(if c.ton { "TON" } else { "Stars" })));
}

fn photo_fields(v: &mut Fields, photo: &FileRef, width: i32, height: i32) {
    if let Some(p) = path_text(photo) {
        v.push(("photo", s(p)));
    }
    v.push(("photo_file_size", n(photo.size)));
    if width != 0 && height != 0 {
        v.push(("width", n(width as i64)));
        v.push(("height", n(height as i64)));
    }
}

fn path_text(file: &FileRef) -> Option<String> {
    if !file.relative_path.is_empty() {
        return Some(file.relative_path.clone());
    }
    match file.skip {
        Some(SkipReason::Unavailable) => Some("(File unavailable, please try again later)".into()),
        Some(SkipReason::FileSize) => {
            Some("(File exceeds maximum size. Change data exporting settings to download.)".into())
        }
        Some(SkipReason::FileType) => {
            Some("(File not included. Change data exporting settings to download.)".into())
        }
        None => None,
    }
}

fn secure_name(t: SecureType) -> &'static str {
    match t {
        SecureType::PersonalDetails => "personal_details",
        SecureType::Passport => "passport",
        SecureType::DriverLicense => "driver_license",
        SecureType::IdentityCard => "identity_card",
        SecureType::InternalPassport => "internal_passport",
        SecureType::Address => "address_information",
        SecureType::UtilityBill => "utility_bill",
        SecureType::BankStatement => "bank_statement",
        SecureType::RentalAgreement => "rental_agreement",
        SecureType::PassportRegistration => "passport_registration",
        SecureType::TemporaryRegistration => "temporary_registration",
        SecureType::Phone => "phone_number",
        SecureType::Email => "email",
    }
}

/// Adds the keys of a service message after its common ones (id, type, date).
pub fn action_fields(v: &mut Fields, m: &Message, a: &Action, peers: &Peers) {
    let from = |v: &mut Fields, label: &'static str, id_label: &'static str| {
        if let Some(f) = m.from {
            v.push((label, name_or_null(peers.name(f))));
            v.push((id_label, s(f.tag())));
        }
    };
    let actor = |v: &mut Fields| from(v, "actor", "actor_id");
    let reply = |v: &mut Fields, label: &'static str| {
        if m.reply_to != 0 {
            v.push((label, n(m.reply_to as i64)));
            if let Some(p) = m.reply_to_peer {
                v.push(("reply_to_peer_id", s(p.tag())));
            }
        }
    };
    let action = |v: &mut Fields, name: &'static str| v.push(("action", s(name)));
    let user = |id: i64| name_or_null(peers.name(PeerId::User(id)));
    let peer_name = |id: PeerId| name_or_null(peers.name(id));

    match a {
        Action::ChatCreate { title, users } => {
            actor(v);
            action(v, "create_group");
            v.push(("title", s(title.clone())));
            v.push(("members", user_names(peers, users)));
        }
        Action::ChatEditTitle { title } => {
            actor(v);
            action(v, "edit_group_title");
            v.push(("title", s(title.clone())));
        }
        Action::ChatEditPhoto {
            photo,
            width,
            height,
        } => {
            actor(v);
            action(v, "edit_group_photo");
            photo_fields(v, photo, *width, *height);
        }
        Action::ChatDeletePhoto => {
            actor(v);
            action(v, "delete_group_photo");
        }
        Action::ChatAddUser { users } => {
            actor(v);
            action(v, "invite_members");
            v.push(("members", user_names(peers, users)));
        }
        Action::ChatDeleteUser { user: id } => {
            actor(v);
            action(v, "remove_members");
            v.push(("members", user_names(peers, &[*id])));
        }
        Action::ChatJoinedByLink { inviter } => {
            actor(v);
            action(v, "join_group_by_link");
            v.push(("inviter", user(*inviter)));
        }
        Action::ChannelCreate { title } => {
            actor(v);
            action(v, "create_channel");
            v.push(("title", s(title.clone())));
        }
        Action::ChatMigrateTo { .. } => {
            actor(v);
            action(v, "migrate_to_supergroup");
        }
        Action::ChannelMigrateFrom { title, .. } => {
            actor(v);
            action(v, "migrate_from_group");
            v.push(("title", s(title.clone())));
        }
        Action::PinMessage => {
            actor(v);
            action(v, "pin_message");
            reply(v, "message_id");
        }
        Action::HistoryClear => {
            actor(v);
            action(v, "clear_history");
        }
        Action::GameScore { score, .. } => {
            actor(v);
            action(v, "score_in_game");
            reply(v, "game_message_id");
            v.push(("score", n(*score as i64)));
        }
        Action::PaymentSent {
            currency,
            amount,
            recurring_init,
            recurring_used,
        } => {
            action(v, "send_payment");
            v.push(("amount", n(*amount)));
            v.push(("currency", s(currency.clone())));
            reply(v, "invoice_message_id");
            if *recurring_used {
                v.push(("recurring", s("used")));
            } else if *recurring_init {
                v.push(("recurring", s("init")));
            }
        }
        Action::PhoneCall {
            conference_id,
            state,
            duration,
        } => {
            actor(v);
            action(
                v,
                if *conference_id != 0 {
                    "conference_call"
                } else {
                    "phone_call"
                },
            );
            if *duration != 0 {
                v.push(("duration_seconds", n(*duration as i64)));
            }
            if *conference_id != 0 {
                v.push(("is_active", b(*state == CallState::Active)));
                v.push(("is_missed", b(*state == CallState::Missed)));
            } else {
                let reason = match state {
                    CallState::Busy => "busy",
                    CallState::Disconnect => "disconnect",
                    CallState::Hangup => "hangup",
                    CallState::Missed => "missed",
                    CallState::MigrateConferenceCall => "migrate_conference_all",
                    _ => "",
                };
                v.push(("discard_reason", s(reason)));
            }
        }
        Action::ScreenshotTaken => {
            actor(v);
            action(v, "take_screenshot");
        }
        Action::CustomAction { message } => {
            actor(v);
            v.push(("information_text", s(message.clone())));
        }
        Action::BotAllowed {
            app_id,
            app,
            domain,
            attach_menu,
            from_request,
        } => {
            if *attach_menu {
                action(v, "attach_menu_bot_allowed");
            } else if *from_request {
                action(v, "web_app_bot_allowed");
            } else if *app_id != 0 {
                action(v, "allow_sending_messages");
                v.push(("reason_app_id", n(*app_id as i64)));
                v.push(("reason_app_name", s(app.clone())));
            } else {
                action(v, "allow_sending_messages");
                v.push(("reason_domain", s(domain.clone())));
            }
        }
        Action::SecureValuesSent { types } => {
            action(v, "send_passport_values");
            v.push((
                "values",
                J::Arr(types.iter().map(|t| s(secure_name(*t))).collect()),
            ));
        }
        Action::ContactSignUp => {
            actor(v);
            action(v, "joined_telegram");
        }
        Action::GeoProximityReached {
            from: f,
            to,
            distance,
            ..
        } => {
            action(v, "proximity_reached");
            v.push(("from", peer_name(*f)));
            v.push(("from_id", s(f.tag())));
            v.push(("to", peer_name(*to)));
            v.push(("to_id", s(to.tag())));
            v.push(("distance", n(*distance as i64)));
        }
        Action::GroupCall { duration } => {
            actor(v);
            action(v, "group_call");
            if *duration != 0 {
                v.push(("duration", n(*duration as i64)));
            }
        }
        Action::InviteToGroupCall { users } => {
            actor(v);
            action(v, "invite_to_group_call");
            v.push(("members", user_names(peers, users)));
        }
        Action::SetMessagesTtl { period } => {
            actor(v);
            action(v, "set_messages_ttl");
            v.push(("period", n(*period as i64)));
        }
        Action::GroupCallScheduled { date } => {
            actor(v);
            action(v, "group_call_scheduled");
            v.push(("schedule_date", n(*date as i64)));
        }
        Action::SetChatTheme { emoji } => {
            actor(v);
            action(v, "edit_chat_theme");
            if !emoji.is_empty() {
                v.push(("emoticon", s(emoji.clone())));
            }
        }
        Action::ChatJoinedByRequest => {
            actor(v);
            action(v, "join_group_by_request");
        }
        Action::ChatJoinedViaCommunity { community_id } => {
            actor(v);
            action(v, "join_group_via_community");
            v.push(("community_id", n(*community_id)));
        }
        Action::WebViewDataSent { text } => {
            action(v, "send_webview_data");
            v.push(("text", s(text.clone())));
        }
        Action::GiftPremium { cost, days } => {
            actor(v);
            action(v, "send_premium_gift");
            if !cost.is_empty() {
                v.push(("cost", s(cost.clone())));
            }
            if *days != 0 {
                v.push(("days", n(*days as i64)));
            }
        }
        Action::TopicCreate { title } => {
            actor(v);
            action(v, "topic_created");
            v.push(("title", s(title.clone())));
        }
        Action::TopicEdit {
            title,
            icon_emoji_id,
        } => {
            actor(v);
            action(v, "topic_edit");
            if !title.is_empty() {
                v.push(("new_title", s(title.clone())));
            }
            if let Some(id) = icon_emoji_id {
                v.push(("new_icon_emoji_id", n(*id as i64)));
            }
        }
        Action::SuggestProfilePhoto {
            photo,
            width,
            height,
        } => {
            actor(v);
            action(v, "suggest_profile_photo");
            photo_fields(v, photo, *width, *height);
        }
        Action::RequestedPeer {
            peers: list,
            button_id,
        } => {
            actor(v);
            action(v, "requested_peer");
            v.push(("button_id", n(*button_id as i64)));
            v.push((
                "peers",
                J::Arr(list.iter().map(|p| J::Raw(p.bare().to_string())).collect()),
            ));
        }
        Action::GiftCode {
            code,
            boost_peer,
            days,
            via_giveaway,
            unclaimed,
        } => {
            action(v, "gift_code_prize");
            v.push(("gift_code", s(code.clone())));
            if let Some(p) = boost_peer {
                v.push(("boost_peer_id", s(p.tag())));
            }
            v.push(("days", n(*days as i64)));
            v.push(("unclaimed", b(*unclaimed)));
            v.push(("via_giveaway", b(*via_giveaway)));
        }
        Action::GiveawayLaunch => action(v, "giveaway_launch"),
        Action::GiveawayResults {
            winners,
            unclaimed,
            credits,
        } => {
            action(v, "giveaway_results");
            v.push(("winners", n(*winners as i64)));
            v.push(("unclaimed", n(*unclaimed as i64)));
            v.push(("stars", b(*credits)));
        }
        Action::SetChatWallPaper { same, .. } => {
            actor(v);
            action(
                v,
                if *same {
                    "set_same_chat_wallpaper"
                } else {
                    "set_chat_wallpaper"
                },
            );
            reply(v, "message_id");
        }
        Action::BoostApply { boosts } => {
            actor(v);
            action(v, "boost_apply");
            v.push(("boosts", n(*boosts as i64)));
        }
        Action::PaymentRefunded {
            peer,
            currency,
            amount,
            transaction_id,
        } => {
            action(v, "refunded_payment");
            v.push(("amount", n(*amount)));
            v.push(("currency", s(currency.clone())));
            v.push(("peer_name", peer_name(*peer)));
            v.push(("peer_id", s(peer.tag())));
            v.push(("charge_id", s(transaction_id.clone())));
        }
        Action::GiftCredits { cost, amount } => {
            actor(v);
            action(
                v,
                if amount.ton {
                    "send_ton_gift"
                } else {
                    "send_stars_gift"
                },
            );
            if !cost.is_empty() {
                v.push(("cost", s(cost.clone())));
            }
            if !amount.is_empty() {
                v.push(("amount_whole", n(amount.whole)));
                v.push(("amount_nano", n(amount.nano)));
            }
        }
        Action::PrizeStars {
            peer,
            amount,
            transaction_id,
            giveaway_msg_id,
            unclaimed,
        } => {
            actor(v);
            action(v, "stars_prize");
            v.push(("boost_peer_id", s(peer.tag())));
            v.push(("boost_peer_name", peer_name(*peer)));
            v.push(("stars", n(*amount)));
            v.push(("is_unclaimed", b(*unclaimed)));
            v.push(("giveaway_msg_id", n(*giveaway_msg_id as i64)));
            v.push(("transaction_id", s(transaction_id.clone())));
        }
        Action::StarGift(g) => {
            actor(v);
            action(v, "send_star_gift");
            v.push(("gift_id", n(g.gift_id as i64)));
            v.push(("stars", n(g.stars)));
            v.push(("is_limited", b(g.limited)));
            v.push(("is_anonymous", b(g.anonymous)));
            v.push(("gift_text", text_json(&g.text, false)));
        }
        Action::PaidMessagesRefunded { messages, stars } => {
            actor(v);
            action(v, "paid_messages_refund");
            v.push(("messages_count", n(*messages as i64)));
            v.push(("stars_count", n(*stars)));
        }
        Action::PaidMessagesPrice {
            stars,
            broadcast_allowed,
        } => {
            actor(v);
            action(v, "paid_messages_price_change");
            v.push(("price_stars", n(*stars as i64)));
            v.push(("is_broadcast_messages_allowed", b(*broadcast_allowed)));
        }
        Action::TodoCompletions {
            completed,
            incompleted,
        } => {
            actor(v);
            action(v, "todo_completions");
            v.push(("completed", int_array(completed)));
            v.push(("incompleted", int_array(incompleted)));
        }
        Action::TodoAppendTasks { items } => {
            actor(v);
            action(v, "todo_append_tasks");
            v.push((
                "items",
                J::Arr(
                    items
                        .iter()
                        .map(|i| {
                            J::Obj(vec![
                                ("text", text_json(&i.text, false)),
                                ("id", J::Raw(i.id.to_string())),
                            ])
                        })
                        .collect(),
                ),
            ));
        }
        Action::PollAppendAnswer { option } => {
            actor(v);
            action(v, "poll_append_answer");
            v.push(("option", s(option.clone())));
        }
        Action::PollDeleteAnswer { option } => {
            actor(v);
            action(v, "poll_delete_answer");
            v.push(("option", s(option.clone())));
        }
        Action::SuggestedPostApproval {
            reject_comment,
            schedule_date,
            price,
            rejected,
            ..
        } => {
            actor(v);
            action(v, "process_suggested_post");
            if *rejected {
                v.push(("rejected", J::Raw("true".into())));
                if !reject_comment.is_empty() {
                    v.push(("comment", s(reject_comment.clone())));
                }
            } else {
                credits_fields(v, price);
                v.push(("scheduled_date", n(*schedule_date as i64)));
            }
        }
        Action::SuggestedPostSuccess { price } => {
            actor(v);
            action(v, "suggested_post_success");
            credits_fields(v, price);
        }
        Action::SuggestedPostRefund { payer_initiated } => {
            actor(v);
            action(v, "suggested_post_refund");
            v.push(("user_initiated", b(*payer_initiated)));
        }
        Action::SuggestBirthday { day, month, year } => {
            actor(v);
            action(v, "suggest_birthday");
            v.push(("day", n(*day as i64)));
            v.push(("month", n(*month as i64)));
            if *year != 0 {
                v.push(("year", n(*year as i64)));
            }
        }
        Action::NoForwardsToggle { new_value } => {
            actor(v);
            action(v, "no_forwards_toggle");
            v.push(("new_value", b(*new_value)));
        }
        Action::NoForwardsRequest { expired, new_value } => {
            actor(v);
            action(v, "no_forwards_request");
            v.push(("expired", b(*expired)));
            v.push(("new_value", b(*new_value)));
        }
        Action::NewCreatorPending { new_creator } => {
            actor(v);
            action(v, "new_creator_pending");
            v.push(("new_creator", user(*new_creator)));
        }
        Action::ChangeCreator { new_creator } => {
            actor(v);
            action(v, "change_creator");
            v.push(("new_creator", user(*new_creator)));
        }
        Action::ManagedBotCreated { bot } => {
            actor(v);
            action(v, "managed_bot_created");
            v.push(("bot", user(*bot)));
        }
        Action::GramTransfer {
            amount,
            peer_address,
            transaction_id,
            comment,
            comment_encrypted,
        } => {
            actor(v);
            action(v, "gram_transfer");
            v.push(("amount_whole", n(amount / 1_000_000_000)));
            v.push(("amount_nano", n(amount % 1_000_000_000)));
            v.push(("peer_address", s(peer_address.clone())));
            v.push(("transaction_id", s(transaction_id.clone())));
            v.push(("comment_encrypted", b(*comment_encrypted)));
            if *comment_encrypted {
                v.push(("encrypted_comment_base64", s(comment.clone())));
            } else {
                v.push(("comment", s(comment.clone())));
            }
        }
        Action::WalletTonConnectRequest {
            session_id,
            expires,
            topic,
            trace_id,
            accepted,
            declined,
        } => {
            actor(v);
            action(v, "ton_connect_request");
            v.push(("session_id", n(*session_id as i64)));
            v.push(("expires", n(*expires as i64)));
            v.push(("topic", s(topic.clone())));
            v.push(("trace_id", s(trace_id.clone())));
            v.push(("accepted", b(*accepted)));
            v.push(("declined", b(*declined)));
        }
        Action::NoPaidMessagesTokenUsed => {
            actor(v);
            action(v, "no_paid_messages_token_used");
        }
    }
}
