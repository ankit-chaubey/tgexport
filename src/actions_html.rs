/*
 * Copyright (c) 2026 Ankit Chaubey <ankitchaubey.dev@gmail.com>
 * https://github.com/ankit-chaubey
 *
 * Project: tgexport
 * Repository: https://github.com/ankit-chaubey/tgexport
 *
 * Licensed under either the Apache License, Version 2.0
 * (LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0)
 * or the MIT License (LICENSE-MIT or https://opensource.org/licenses/MIT),
 * at your option.
 */

//! The text of service messages on the HTML pages.

use crate::actions::*;
use crate::html_out::{date_text, format_text, time_text};
use crate::model::*;
use crate::rich_html::{Attrs, Html, attrs, escape};

fn wrap_peer_name(peers: &Peers, id: PeerId) -> String {
    let name = peers.name(id);
    if name.is_empty() {
        "Deleted".into()
    } else {
        escape(&name)
    }
}

fn wrap_user_name(peers: &Peers, id: i64) -> String {
    let name = peers.name(PeerId::User(id));
    if name.is_empty() {
        "Deleted Account".into()
    } else {
        escape(&name)
    }
}

/// `a`, `a and b`, `a, b and c`.
fn serialize_list(values: &[String]) -> String {
    match values.len() {
        0 => String::new(),
        1 => values[0].clone(),
        n => {
            let mut result = values[0].clone();
            for v in &values[1..n - 1] {
                result.push_str(", ");
                result.push_str(v);
            }
            result.push_str(" and ");
            result.push_str(&values[n - 1]);
            result
        }
    }
}

fn wrap_user_names(peers: &Peers, ids: &[i64]) -> String {
    serialize_list(
        &ids.iter()
            .map(|i| wrap_user_name(peers, *i))
            .collect::<Vec<_>>(),
    )
}

fn money(amount: i64, currency: &str) -> String {
    format!("{:.2} {}", amount as f64 / 100.0, currency)
}

fn secure_label(t: SecureType) -> &'static str {
    match t {
        SecureType::PersonalDetails => "Personal details",
        SecureType::Passport => "Passport",
        SecureType::DriverLicense => "Driver license",
        SecureType::IdentityCard => "Identity card",
        SecureType::InternalPassport => "Internal passport",
        SecureType::Address => "Address information",
        SecureType::UtilityBill => "Utility bill",
        SecureType::BankStatement => "Bank statement",
        SecureType::RentalAgreement => "Rental agreement",
        SecureType::PassportRegistration => "Passport registration",
        SecureType::TemporaryRegistration => "Temporary registration",
        SecureType::Phone => "Phone number",
        SecureType::Email => "Email",
    }
}

fn grams(nanos: i64) -> String {
    const NANOS: u64 = 1_000_000_000;
    let negative = nanos < 0;
    let absolute = nanos.unsigned_abs();
    let whole = absolute / NANOS;
    let mut fraction = format!("{:09}", absolute % NANOS);
    while fraction.ends_with('0') {
        fraction.pop();
    }
    format!(
        "{}{}{}{}",
        if negative { "-" } else { "" },
        whole,
        if fraction.is_empty() {
            String::new()
        } else {
            format!(".{fraction}")
        },
        if absolute == NANOS { " Gram" } else { " Grams" }
    )
}

fn credit_unit(c: &Credits, ton: &str, stars: &str) -> String {
    if c.ton {
        ton.to_string()
    } else {
        stars.to_string()
    }
}

/// The amount as a plain number, the way Telegram Desktop prints `value()`.
fn credit_value(c: &Credits) -> String {
    c.whole.to_string()
}

fn month_name(m: i32) -> &'static str {
    match m {
        1 => " January",
        2 => " February",
        3 => " March",
        4 => " April",
        5 => " May",
        6 => " June",
        7 => " July",
        8 => " August",
        9 => " September",
        10 => " October",
        11 => " November",
        12 => " December",
        _ => "",
    }
}

fn days_word(days: i32) -> String {
    format!("{} {}", days, if days > 1 { "days" } else { "day" })
}

/// The text of a service message, or an empty string when the message is
/// shown as an ordinary one (phone calls and the like).
pub fn service_text(
    m: &Message,
    a: &Action,
    dialog: &DialogInfo,
    peers: &Peers,
    base: &str,
    link: &dyn Fn(i32, &str) -> String,
) -> String {
    let is_channel = dialog.dtype.is_channel();
    let from_id = m.from.unwrap_or(dialog.peer);
    let from = wrap_peer_name(peers, from_id);
    let reply_link = |text: &str| link(m.reply_to, text);
    let quoted = |text: &str| format!("&laquo;{}&raquo;", escape(text));

    match a {
        Action::ChatCreate { title, users } => {
            let mut s = format!("{from} created group {}", quoted(title));
            if !users.is_empty() {
                s.push_str(&format!(" with members {}", wrap_user_names(peers, users)));
            }
            s
        }
        Action::ChatEditTitle { title } => {
            if is_channel {
                format!("Channel title changed to {}", quoted(title))
            } else {
                format!("{from} changed group title to {}", quoted(title))
            }
        }
        Action::ChatEditPhoto { .. } => {
            if is_channel { "Channel photo changed".into() } else { format!("{from} changed group photo") }
        }
        Action::ChatDeletePhoto => {
            if is_channel { "Channel photo removed".into() } else { format!("{from} removed group photo") }
        }
        Action::ChatAddUser { users } => format!("{from} invited {}", wrap_user_names(peers, users)),
        Action::ChatDeleteUser { user } => format!("{from} removed {}", wrap_user_name(peers, *user)),
        Action::ChatJoinedByLink { inviter } => {
            format!("{from} joined group by link from {}", wrap_user_name(peers, *inviter))
        }
        Action::ChannelCreate { title } => format!("Channel {} created", quoted(title)),
        Action::ChatMigrateTo => format!("{from} converted this group to a supergroup"),
        Action::ChannelMigrateFrom { title, .. } => {
            format!("{from} converted a basic group to this supergroup {}", quoted(title))
        }
        Action::PinMessage => format!("{from} pinned {}", reply_link("this message")),
        Action::HistoryClear => "History cleared".into(),
        Action::GameScore { score, .. } => {
            format!("{from} scored {score} in {}", reply_link("this game"))
        }
        Action::PaymentSent {
            currency,
            amount,
            recurring_init,
            recurring_used,
        } => {
            let amount = money(*amount, currency);
            if *recurring_used {
                return format!("You were charged {amount} via recurring payment");
            }
            let mut s = format!(
                "You have successfully transferred {amount} for {}",
                reply_link("this invoice")
            );
            if *recurring_init {
                s.push_str(" and allowed future recurring payments");
            }
            s
        }
        Action::PhoneCall { .. } => String::new(),
        Action::ScreenshotTaken => format!("{from} took a screenshot"),
        Action::CustomAction { message } => escape(message),
        Action::BotAllowed {
            app,
            domain,
            attach_menu,
            from_request,
            ..
        } => {
            if *attach_menu {
                "You allowed this bot to message you when you added it in the attachment menu."
                    .into()
            } else if *from_request {
                "You allowed this bot to message you in his web-app.".into()
            } else if app.is_empty() {
                format!(
                    "You allowed this bot to message you when you opened {}",
                    escape(app)
                )
            } else {
                format!(
                    "You allowed this bot to message you when you logged in on {}",
                    escape(domain)
                )
            }
        }
        Action::SecureValuesSent { types } => {
            let list: Vec<String> = types.iter().map(|t| secure_label(*t).to_string()).collect();
            format!(
                "You have sent the following documents: {}",
                serialize_list(&list)
            )
        }
        Action::ContactSignUp => format!("{from} joined Telegram"),
        Action::GeoProximityReached {
            from: f,
            to,
            distance,
            from_self,
            to_self,
        } => {
            let from_name = wrap_peer_name(peers, *f);
            let to_name = wrap_peer_name(peers, *to);
            let distance = if *distance >= 1000 {
                let km = (10 * (distance / 10)) as f64 / 1000.0;
                format!("{} km", km)
            } else if *distance == 1 {
                "1 meter".to_string()
            } else {
                format!("{distance} meters")
            };
            if *from_self {
                format!("You are now within {distance} from {to_name}")
            } else if *to_self {
                format!("{from_name} is now within {distance} from you")
            } else {
                format!("{from_name} is now within {distance} from {to_name}")
            }
        }
        Action::GroupCall { duration } => {
            let d = if *duration != 0 {
                format!(" ({duration} seconds)")
            } else {
                String::new()
            };
            if is_channel {
                format!("Voice chat{d}")
            } else {
                format!("{from} started voice chat{d}")
            }
        }
        Action::InviteToGroupCall { users } => format!(
            "{from} invited {} to the voice chat",
            wrap_user_names(peers, users)
        ),
        Action::SetMessagesTtl { period } => {
            let text = if *period == 7 * 86400 {
                "7 days"
            } else if *period == 86400 {
                "24 hours"
            } else {
                ""
            };
            if is_channel {
                if *period != 0 {
                    format!("New messages will auto-delete in {text}")
                } else {
                    "New messages will not auto-delete".into()
                }
            } else if *period != 0 {
                format!("{from} has set messages to auto-delete in {text}")
            } else {
                format!("{from} has set messages not to auto-delete")
            }
        }
        Action::GroupCallScheduled { date } => {
            let text = crate::html_out::date_time(*date, false);
            if is_channel {
                format!("Voice chat scheduled for {text}")
            } else {
                format!("{from} scheduled a voice chat for {text}")
            }
        }
        Action::SetChatTheme { emoji } => {
            if emoji.is_empty() {
                if is_channel {
                    "Channel theme was disabled".into()
                } else {
                    format!("{from} disabled chat theme")
                }
            } else if is_channel {
                format!("Channel theme was changed to {}", escape(emoji))
            } else {
                format!("{from} changed chat theme to {}", escape(emoji))
            }
        }
        Action::ChatJoinedByRequest => format!("{from} joined group by request"),
        Action::ChatJoinedViaCommunity { .. } => format!("{from} joined group via a community"),
        Action::WebViewDataSent { text } => format!(
            "You have just successfully transferred data from the {} button to the bot",
            quoted(text)
        ),
        Action::GiftPremium { cost, days } => {
            if *days == 0 || cost.is_empty() {
                format!("{from} sent you a gift.")
            } else {
                format!(
                    "{from} sent you a gift for {}: Telegram Premium for {days} days.",
                    escape(cost)
                )
            }
        }
        Action::TopicCreate { title } => format!("{from} created topic {}", quoted(title)),
        Action::TopicEdit { title, icon_emoji_id } => {
            let mut parts = Vec::new();
            if !title.is_empty() {
                parts.push(format!("title to {}", quoted(title)));
            }
            if let Some(id) = icon_emoji_id {
                parts.push(format!("icon to &laquo;{id}&raquo;"));
            }
            format!("{from} changed topic {}", parts.join(","))
        }
        Action::SuggestProfilePhoto { .. } => format!("{from} suggests to use this photo"),
        Action::RequestedPeer { .. } => "requested: ".into(),
        Action::SetChatWallPaper { same, .. } => {
            if *same {
                format!(
                    "{from} set {} for this chat",
                    reply_link("the same background")
                )
            } else {
                format!("{from} set a new background for this chat")
            }
        }
        Action::GiftCode {
            days,
            via_giveaway,
            unclaimed,
            ..
        } => {
            let d = days_word(*days);
            if *unclaimed {
                format!("This is an unclaimed Telegram Premium for {d} prize in a giveaway organized by a channel.")
            } else if *via_giveaway {
                format!("You won a Telegram Premium for {d} prize in a giveaway organized by a channel.")
            } else {
                format!("You've received a Telegram Premium for {d} gift from a channel.")
            }
        }
        Action::GiveawayLaunch => format!(
            "{from} just started a giveaway of Telegram Premium subscriptions to its followers."
        ),
        Action::GiveawayResults {
            winners,
            unclaimed,
            credits,
        } => {
            if *winners == 0 {
                "No winners of the giveaway could be selected.".into()
            } else if *credits && *unclaimed != 0 {
                "Some winners of the giveaway were randomly selected by Telegram and received their prize.".into()
            } else if !*credits && *unclaimed != 0 {
                "Some winners of the giveaway were randomly selected by Telegram and received private messages with giftcodes.".into()
            } else if *credits {
                format!("{winners} of the giveaway was randomly selected by Telegram and received their prize.")
            } else {
                format!("{winners} of the giveaway was randomly selected by Telegram and received private messages with giftcodes.")
            }
        }
        Action::BoostApply { boosts } => format!(
            "{from} boosted the group {boosts} {}",
            if *boosts > 1 { "times" } else { "time" }
        ),
        Action::PaymentRefunded {
            peer,
            currency,
            amount,
            ..
        } => format!(
            "{} refunded back {}",
            wrap_peer_name(peers, *peer),
            money(*amount, currency)
        ),
        Action::GiftCredits { cost, amount } => {
            if amount.is_empty() || cost.is_empty() {
                format!("{from} sent you a gift.")
            } else {
                format!(
                    "{from} sent you a gift for {}: {}{}",
                    escape(cost),
                    credit_value(amount),
                    credit_unit(amount, " TON.", " Telegram Stars.")
                )
            }
        }
        Action::PrizeStars { peer, amount, .. } => format!(
            "You won a prize in a giveaway organized by {}.\n Your prize is {amount} Telegram Stars.",
            wrap_peer_name(peers, *peer)
        )
        .replace('\n', "<br>"),
        Action::StarGift(g) => format!("{from} sent you a gift of {} Telegram Stars.", g.stars),
        Action::PaidMessagesRefunded { messages, stars } => {
            let dialog_name = wrap_peer_name(peers, dialog.peer);
            if m.out {
                format!("You refunded {stars} Stars for {messages} messages to {dialog_name}")
            } else {
                format!("{dialog_name} refunded {stars} Stars for {messages} messages to you")
            }
        }
        Action::PaidMessagesPrice {
            stars,
            broadcast_allowed,
        } => {
            if is_channel {
                if !*broadcast_allowed {
                    "Direct messages were disabled.".into()
                } else {
                    format!("Price per direct message changed to {stars} Telegram Stars.")
                }
            } else {
                format!("Price per message changed to {stars} Telegram Stars.")
            }
        }
        Action::TodoCompletions {
            completed,
            incompleted,
        } => {
            let list = |v: &[i32]| -> String {
                let numbers: Vec<String> = v.iter().map(|i| i.to_string()).collect();
                match numbers.len() {
                    0 => String::new(),
                    1 => numbers[0].clone(),
                    n => format!("{} and {}", numbers[..n - 1].join(", "), numbers[n - 1]),
                }
            };
            let todo = reply_link("this todo list");
            if completed.is_empty() && !incompleted.is_empty() {
                format!("{from} marked {} as not done yet in {todo}.", list(incompleted))
            } else if !completed.is_empty() && incompleted.is_empty() {
                format!("{from} marked {} as done in {todo}.", list(completed))
            } else {
                format!(
                    "{from} marked {} as done and {} as not done yet in {todo}.",
                    list(completed),
                    list(incompleted)
                )
            }
        }
        Action::TodoAppendTasks { items } => {
            let tasks: Vec<String> = items
                .iter()
                .map(|t| format!("&quot;{}&quot;", format_text(&t.text, base)))
                .collect();
            format!("{from} added tasks: {}", tasks.join(", "))
        }
        Action::PollAppendAnswer { option } => {
            format!("{from} added &quot;{}&quot; to the poll.", escape(option))
        }
        Action::PollDeleteAnswer { option } => {
            format!("{from} removed &quot;{}&quot; from the poll.", escape(option))
        }
        Action::SuggestedPostApproval {
            reject_comment,
            schedule_date,
            price,
            rejected,
        } => {
            let mut s = format!(
                "{from} {} your suggested post",
                if *rejected { "rejected" } else { "approved" }
            );
            if !price.is_empty() {
                s.push_str(&format!(
                    ", for {}{}",
                    credit_value(price),
                    credit_unit(price, " TON", " stars")
                ));
            }
            if *schedule_date != 0 {
                s.push_str(&format!(
                    ", {} at {}",
                    date_text(*schedule_date),
                    time_text(*schedule_date)
                ));
            }
            if reject_comment.is_empty() {
                s.push('.');
            } else {
                s.push_str(&format!(
                    ", with comment: &quot;{}&quot;",
                    escape(reject_comment)
                ));
            }
            s
        }
        Action::SuggestedPostSuccess { price } => format!(
            "The paid post was shown for 24 hours and {}{} were transferred to the channel.",
            credit_value(price),
            credit_unit(price, " TON", " stars")
        ),
        Action::SuggestedPostRefund { payer_initiated } => if *payer_initiated {
            "The user refunded the payment, post was deleted."
        } else {
            "The admin deleted the post early, the payment was refunded."
        }
        .into(),
        Action::SuggestBirthday { day, month, year } => format!(
            "{from} suggests to add a date of birth: {day}{}{}",
            month_name(*month),
            if *year != 0 { format!(" {year}") } else { String::new() }
        ),
        Action::NoForwardsToggle { new_value } => format!(
            "{from}{}",
            if *new_value {
                " disabled sharing in this chat"
            } else {
                " enabled sharing in this chat"
            }
        ),
        Action::NoForwardsRequest { .. } => format!("{from} requested to enable sharing in this chat"),
        Action::NewCreatorPending { new_creator } => format!(
            "{} will become the new main admin in 7 days if {from} does not return",
            wrap_user_name(peers, *new_creator)
        ),
        Action::ChangeCreator { new_creator } => format!(
            "{from} made {} the new main admin of the group",
            wrap_user_name(peers, *new_creator)
        ),
        Action::ManagedBotCreated { bot } => {
            format!("{from} created a bot {}", wrap_user_name(peers, *bot))
        }
        Action::GramTransfer {
            amount,
            peer_address,
            transaction_id,
            comment,
            comment_encrypted,
        } => {
            let amount = grams(*amount);
            let address = if peer_address.is_empty() {
                String::new()
            } else {
                format!(" ({})", escape(peer_address))
            };
            // Service accounts whose name is hidden, by their fixed ids.
            let hidden = dialog.peer == PeerId::User(333000) || dialog.peer == PeerId::User(777000);
            let sender = if hidden { "Someone".to_string() } else { from.clone() };
            let mut result = if m.out {
                format!(
                    "You sent {amount} to {}{address}",
                    wrap_peer_name(peers, dialog.peer)
                )
            } else {
                format!("{sender}{address} sent you {amount}")
            };
            if !transaction_id.is_empty() {
                result.push_str(&format!(", transaction {}", escape(transaction_id)));
            }
            if *comment_encrypted {
                result.push_str(", with an ");
                let mut html = Html::default();
                let a: Attrs = attrs(&[
                    ("class", "gram_transfer_encrypted_comment"),
                    ("data-encrypted-comment", comment),
                    ("inline", ""),
                ]);
                result.push_str(&html.push_tag("span", a));
                result.push_str("encrypted comment");
                result.push_str(&html.pop_tag());
            } else if !comment.is_empty() {
                result.push_str(&format!(", with comment: {}", quoted(comment)));
            }
            result
        }
        Action::WalletTonConnectRequest {
            topic,
            accepted,
            declined,
            ..
        } => {
            let topic = if topic.is_empty() {
                String::new()
            } else {
                format!(" {}", quoted(topic))
            };
            if *accepted {
                format!("You approved a TON Connect request{topic}.")
            } else if *declined {
                format!("You declined a TON Connect request{topic}.")
            } else {
                format!("You received a TON Connect request{topic}.")
            }
        }
        Action::NoPaidMessagesTokenUsed => format!("{from} started a chat via a free messages link"),
    }
}
