//! Service actions: what the "service" messages of a chat describe.

use ferogram::tl;
use tl::enums as E;

use crate::model::*;
use crate::parse::{self, MediaCtx};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CallState {
    Unknown,
    Missed,
    Disconnect,
    Hangup,
    Busy,
    MigrateConferenceCall,
    Invitation,
    Active,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SecureType {
    PersonalDetails,
    Passport,
    DriverLicense,
    IdentityCard,
    InternalPassport,
    Address,
    UtilityBill,
    BankStatement,
    RentalAgreement,
    PassportRegistration,
    TemporaryRegistration,
    Phone,
    Email,
}

/// Stars or TON, as a whole part and a part in billionths.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Credits {
    pub whole: i64,
    pub nano: i64,
    pub ton: bool,
}

const NANO: i64 = 1_000_000_000;

impl Credits {
    fn new(whole: i64, nano: i64, ton: bool) -> Self {
        let mut c = Credits { whole, nano, ton };
        c.normalize();
        c
    }

    fn normalize(&mut self) {
        let carry = self.nano.div_euclid(NANO);
        self.whole += carry;
        self.nano -= carry * NANO;
    }

    pub fn is_empty(&self) -> bool {
        self.whole == 0 && self.nano == 0
    }
}

fn credits_from(amount: &E::StarsAmount) -> Credits {
    match amount {
        E::StarsAmount::StarsAmount(a) => Credits::new(a.amount, a.nanos as i64, false),
        E::StarsAmount::StarsTonAmount(a) => Credits::new(a.amount / NANO, a.amount % NANO, true),
    }
}

#[derive(Clone, Debug)]
pub struct TodoItem {
    pub text: Text,
    pub id: i32,
}

pub fn parse_todo_item(item: &E::TodoItem) -> TodoItem {
    let E::TodoItem::TodoItem(d) = item;
    TodoItem {
        text: text_of(&d.title),
        id: d.id,
    }
}

fn text_of(t: &E::TextWithEntities) -> Text {
    let E::TextWithEntities::TextWithEntities(d) = t;
    parse::parse_text(&d.text, &d.entities)
}

#[derive(Clone, Debug, Default)]
pub struct StarGift {
    pub gift_id: u64,
    pub stars: i64,
    pub text: Text,
    pub anonymous: bool,
    pub limited: bool,
    pub offer_price: Credits,
    pub offer_expire_at: i32,
    pub offer: bool,
    pub offer_accepted: bool,
    pub offer_declined: bool,
    pub offer_expired: bool,
}

fn star_gift(gift: &E::StarGift) -> StarGift {
    match gift {
        E::StarGift::StarGift(g) => StarGift {
            gift_id: g.id as u64,
            stars: g.stars,
            limited: g.limited,
            ..Default::default()
        },
        E::StarGift::Unique(g) => StarGift {
            gift_id: g.id as u64,
            ..Default::default()
        },
    }
}

#[derive(Clone, Debug)]
pub enum Action {
    ChatCreate {
        title: String,
        users: Vec<i64>,
    },
    ChatEditTitle {
        title: String,
    },
    ChatEditPhoto {
        photo: FileRef,
        width: i32,
        height: i32,
    },
    ChatDeletePhoto,
    ChatAddUser {
        users: Vec<i64>,
    },
    ChatDeleteUser {
        user: i64,
    },
    ChatJoinedByLink {
        inviter: i64,
    },
    ChannelCreate {
        title: String,
    },
    ChatMigrateTo {
        channel_id: i64,
    },
    ChannelMigrateFrom {
        title: String,
        chat_id: i64,
    },
    PinMessage,
    HistoryClear,
    GameScore {
        game_id: u64,
        score: i32,
    },
    PaymentSent {
        currency: String,
        amount: i64,
        recurring_init: bool,
        recurring_used: bool,
    },
    PhoneCall {
        conference_id: u64,
        state: CallState,
        duration: i32,
    },
    ScreenshotTaken,
    CustomAction {
        message: String,
    },
    BotAllowed {
        app_id: u64,
        app: String,
        domain: String,
        attach_menu: bool,
        from_request: bool,
    },
    SecureValuesSent {
        types: Vec<SecureType>,
    },
    ContactSignUp,
    GeoProximityReached {
        from: PeerId,
        to: PeerId,
        distance: i32,
        from_self: bool,
        to_self: bool,
    },
    GroupCall {
        duration: i32,
    },
    InviteToGroupCall {
        users: Vec<i64>,
    },
    SetMessagesTtl {
        period: i32,
    },
    GroupCallScheduled {
        date: i32,
    },
    SetChatTheme {
        emoji: String,
    },
    ChatJoinedByRequest,
    ChatJoinedViaCommunity {
        community_id: i64,
    },
    WebViewDataSent {
        text: String,
    },
    GiftPremium {
        cost: String,
        days: i32,
    },
    TopicCreate {
        title: String,
    },
    TopicEdit {
        title: String,
        icon_emoji_id: Option<u64>,
    },
    SuggestProfilePhoto {
        photo: FileRef,
        width: i32,
        height: i32,
    },
    SetChatWallPaper {
        same: bool,
        both: bool,
    },
    GiftCode {
        code: String,
        boost_peer: Option<PeerId>,
        days: i32,
        via_giveaway: bool,
        unclaimed: bool,
    },
    RequestedPeer {
        peers: Vec<PeerId>,
        button_id: i32,
    },
    GiveawayLaunch,
    GiveawayResults {
        winners: i32,
        unclaimed: i32,
        credits: bool,
    },
    BoostApply {
        boosts: i32,
    },
    PaymentRefunded {
        peer: PeerId,
        currency: String,
        amount: i64,
        transaction_id: String,
    },
    GiftCredits {
        cost: String,
        amount: Credits,
    },
    PrizeStars {
        peer: PeerId,
        amount: i64,
        transaction_id: String,
        giveaway_msg_id: i32,
        unclaimed: bool,
    },
    StarGift(StarGift),
    PaidMessagesRefunded {
        messages: i32,
        stars: i64,
    },
    PaidMessagesPrice {
        stars: i32,
        broadcast_allowed: bool,
    },
    TodoCompletions {
        completed: Vec<i32>,
        incompleted: Vec<i32>,
    },
    TodoAppendTasks {
        items: Vec<TodoItem>,
    },
    PollAppendAnswer {
        option: String,
    },
    PollDeleteAnswer {
        option: String,
    },
    SuggestedPostApproval {
        reject_comment: String,
        schedule_date: i32,
        price: Credits,
        rejected: bool,
        balance_too_low: bool,
    },
    SuggestedPostSuccess {
        price: Credits,
    },
    SuggestedPostRefund {
        payer_initiated: bool,
    },
    SuggestBirthday {
        day: i32,
        month: i32,
        year: i32,
    },
    NoForwardsToggle {
        new_value: bool,
    },
    NoForwardsRequest {
        expired: bool,
        new_value: bool,
    },
    NewCreatorPending {
        new_creator: i64,
    },
    ChangeCreator {
        new_creator: i64,
    },
    ManagedBotCreated {
        bot: i64,
    },
    GramTransfer {
        amount: i64,
        peer_address: String,
        transaction_id: String,
        comment: String,
        comment_encrypted: bool,
    },
    WalletTonConnectRequest {
        session_id: u64,
        expires: i32,
        topic: String,
        trace_id: String,
        accepted: bool,
        declined: bool,
    },
    NoPaidMessagesTokenUsed,
}

/// Cost as text. Telegram Desktop uses its own currency table for this.
fn cost(amount: i64, currency: &str) -> String {
    format!("{:.2} {}", amount as f64 / 100.0, currency)
}

fn photo_action(ctx: &mut MediaCtx, photo: &E::Photo, date: i32) -> Option<(FileRef, i32, i32)> {
    ctx.photos += 1;
    let name = format!("photo_{}{}.jpg", ctx.photos, parse::file_date_part(date));
    parse::photo_file(photo, format!("{}photos/{}", ctx.folder, name))
}

fn call_state(reason: &E::PhoneCallDiscardReason) -> CallState {
    match reason {
        E::PhoneCallDiscardReason::Missed => CallState::Missed,
        E::PhoneCallDiscardReason::Disconnect => CallState::Disconnect,
        E::PhoneCallDiscardReason::Hangup => CallState::Hangup,
        E::PhoneCallDiscardReason::Busy => CallState::Busy,
        E::PhoneCallDiscardReason::MigrateConferenceCall(_) => CallState::MigrateConferenceCall,
    }
}

fn secure_type(t: &E::SecureValueType) -> SecureType {
    match t {
        E::SecureValueType::PersonalDetails => SecureType::PersonalDetails,
        E::SecureValueType::Passport => SecureType::Passport,
        E::SecureValueType::DriverLicense => SecureType::DriverLicense,
        E::SecureValueType::IdentityCard => SecureType::IdentityCard,
        E::SecureValueType::InternalPassport => SecureType::InternalPassport,
        E::SecureValueType::Address => SecureType::Address,
        E::SecureValueType::UtilityBill => SecureType::UtilityBill,
        E::SecureValueType::BankStatement => SecureType::BankStatement,
        E::SecureValueType::RentalAgreement => SecureType::RentalAgreement,
        E::SecureValueType::PassportRegistration => SecureType::PassportRegistration,
        E::SecureValueType::TemporaryRegistration => SecureType::TemporaryRegistration,
        E::SecureValueType::Phone => SecureType::Phone,
        E::SecureValueType::Email => SecureType::Email,
    }
}

fn poll_answer_text(a: &E::PollAnswer) -> String {
    match a {
        E::PollAnswer::PollAnswer(d) => {
            let E::TextWithEntities::TextWithEntities(t) = &d.text;
            t.text.clone()
        }
        _ => String::new(),
    }
}

/// `None` for actions Telegram Desktop does not export (the message is then
/// treated as an ordinary, empty one).
pub fn parse_action(ctx: &mut MediaCtx, a: &E::MessageAction, date: i32) -> Option<Action> {
    use E::MessageAction as M;
    Some(match a {
        M::Empty
        | M::PaymentSentMe(_)
        | M::SecureValuesSentMe(_)
        | M::WebViewDataSentMe(_)
        | M::RequestedPeerSentMe(_)
        | M::ChangeCommunity(_) => return None,
        M::ChatCreate(x) => Action::ChatCreate {
            title: x.title.clone(),
            users: x.users.clone(),
        },
        M::ChatEditTitle(x) => Action::ChatEditTitle {
            title: x.title.clone(),
        },
        M::ChatEditPhoto(x) => {
            let (photo, width, height) =
                photo_action(ctx, &x.photo, date).unwrap_or((FileRef::none(), 0, 0));
            Action::ChatEditPhoto {
                photo,
                width,
                height,
            }
        }
        M::ChatDeletePhoto => Action::ChatDeletePhoto,
        M::ChatAddUser(x) => Action::ChatAddUser {
            users: x.users.clone(),
        },
        M::ChatDeleteUser(x) => Action::ChatDeleteUser { user: x.user_id },
        M::ChatJoinedByLink(x) => Action::ChatJoinedByLink {
            inviter: x.inviter_id,
        },
        M::ChannelCreate(x) => Action::ChannelCreate {
            title: x.title.clone(),
        },
        M::ChatMigrateTo(x) => Action::ChatMigrateTo {
            channel_id: x.channel_id,
        },
        M::ChannelMigrateFrom(x) => Action::ChannelMigrateFrom {
            title: x.title.clone(),
            chat_id: x.chat_id,
        },
        M::PinMessage => Action::PinMessage,
        M::HistoryClear => Action::HistoryClear,
        M::GameScore(x) => Action::GameScore {
            game_id: x.game_id as u64,
            score: x.score,
        },
        M::PaymentSent(x) => Action::PaymentSent {
            currency: x.currency.clone(),
            amount: x.total_amount,
            recurring_init: x.recurring_init,
            recurring_used: x.recurring_used,
        },
        M::PhoneCall(x) => Action::PhoneCall {
            conference_id: 0,
            state: x
                .reason
                .as_ref()
                .map(call_state)
                .unwrap_or(CallState::Unknown),
            duration: x.duration.unwrap_or(0),
        },
        M::ConferenceCall(x) => {
            let duration = x.duration.unwrap_or(0);
            Action::PhoneCall {
                conference_id: x.call_id as u64,
                state: if x.missed {
                    CallState::Missed
                } else if x.active {
                    CallState::Active
                } else if duration != 0 {
                    CallState::Hangup
                } else {
                    CallState::Invitation
                },
                duration,
            }
        }
        M::ScreenshotTaken => Action::ScreenshotTaken,
        M::CustomAction(x) => Action::CustomAction {
            message: x.message.clone(),
        },
        M::BotAllowed(x) => {
            let (app_id, app) = match &x.app {
                Some(E::BotApp::BotApp(b)) => (b.id as u64, b.title.clone()),
                _ => (0, String::new()),
            };
            Action::BotAllowed {
                app_id,
                app,
                domain: x.domain.clone().unwrap_or_default(),
                attach_menu: x.attach_menu,
                from_request: x.from_request,
            }
        }
        M::SecureValuesSent(x) => Action::SecureValuesSent {
            types: x.types.iter().map(secure_type).collect(),
        },
        M::ContactSignUp => Action::ContactSignUp,
        M::GeoProximityReached(x) => {
            let from = peer_from_tl(&x.from_id);
            let to = peer_from_tl(&x.to_id);
            Action::GeoProximityReached {
                from,
                to,
                distance: x.distance,
                from_self: from == PeerId::User(ctx.self_id),
                to_self: to == PeerId::User(ctx.self_id),
            }
        }
        M::GroupCall(x) => Action::GroupCall {
            duration: x.duration.unwrap_or(0),
        },
        M::InviteToGroupCall(x) => Action::InviteToGroupCall {
            users: x.users.clone(),
        },
        M::SetMessagesTtl(x) => Action::SetMessagesTtl { period: x.period },
        M::GroupCallScheduled(x) => Action::GroupCallScheduled {
            date: x.schedule_date,
        },
        M::SetChatTheme(x) => Action::SetChatTheme {
            emoji: match &x.theme {
                E::ChatTheme::ChatTheme(t) => t.emoticon.clone(),
                E::ChatTheme::UniqueGift(_) => String::new(),
            },
        },
        M::ChatJoinedByRequest => Action::ChatJoinedByRequest,
        M::ChatJoinedViaCommunity(x) => Action::ChatJoinedViaCommunity {
            community_id: x.community_id,
        },
        M::WebViewDataSent(x) => Action::WebViewDataSent {
            text: x.text.clone(),
        },
        M::GiftPremium(x) => Action::GiftPremium {
            cost: cost(x.amount, &x.currency),
            days: x.days,
        },
        M::TopicCreate(x) => Action::TopicCreate {
            title: x.title.clone(),
        },
        M::TopicEdit(x) => Action::TopicEdit {
            title: x.title.clone().unwrap_or_default(),
            icon_emoji_id: x.icon_emoji_id.map(|v| v as u64),
        },
        M::SuggestProfilePhoto(x) => {
            let (photo, width, height) =
                photo_action(ctx, &x.photo, date).unwrap_or((FileRef::none(), 0, 0));
            Action::SuggestProfilePhoto {
                photo,
                width,
                height,
            }
        }
        M::SetChatWallPaper(x) => Action::SetChatWallPaper {
            same: x.same,
            both: x.for_both,
        },
        M::RequestedPeer(x) => Action::RequestedPeer {
            peers: x.peers.iter().map(peer_from_tl).collect(),
            button_id: x.button_id,
        },
        M::GiftCode(x) => Action::GiftCode {
            code: x.slug.clone(),
            boost_peer: x.boost_peer.as_ref().map(peer_from_tl),
            days: x.days,
            via_giveaway: x.via_giveaway,
            unclaimed: x.unclaimed,
        },
        M::GiveawayLaunch(_) => Action::GiveawayLaunch,
        M::GiveawayResults(x) => Action::GiveawayResults {
            winners: x.winners_count,
            unclaimed: x.unclaimed_count,
            credits: x.stars,
        },
        M::BoostApply(x) => Action::BoostApply { boosts: x.boosts },
        M::PaymentRefunded(x) => {
            let E::PaymentCharge::PaymentCharge(c) = &x.charge;
            Action::PaymentRefunded {
                peer: peer_from_tl(&x.peer),
                currency: x.currency.clone(),
                amount: x.total_amount,
                transaction_id: c.id.clone(),
            }
        }
        M::GiftStars(x) => Action::GiftCredits {
            cost: cost(x.amount, &x.currency),
            amount: Credits::new(x.stars, 0, false),
        },
        M::GiftTon(x) => Action::GiftCredits {
            cost: cost(x.amount, &x.currency),
            amount: Credits::new(x.crypto_amount / NANO, x.crypto_amount % NANO, true),
        },
        M::PrizeStars(x) => Action::PrizeStars {
            peer: peer_from_tl(&x.boost_peer),
            amount: x.stars,
            transaction_id: x.transaction_id.clone(),
            giveaway_msg_id: x.giveaway_msg_id,
            unclaimed: x.unclaimed,
        },
        M::StarGift(x) => {
            let mut g = star_gift(&x.gift);
            g.text = x.message.as_ref().map(text_of).unwrap_or_default();
            g.anonymous = x.name_hidden;
            Action::StarGift(g)
        }
        M::StarGiftUnique(x) => Action::StarGift(star_gift(&x.gift)),
        M::StarGiftPurchaseOffer(x) => {
            let mut g = star_gift(&x.gift);
            g.offer = true;
            g.offer_price = credits_from(&x.price);
            g.offer_expire_at = x.expires_at;
            g.offer_accepted = x.accepted;
            g.offer_declined = x.declined;
            Action::StarGift(g)
        }
        M::StarGiftPurchaseOfferDeclined(x) => {
            let mut g = star_gift(&x.gift);
            g.offer = true;
            g.offer_declined = true;
            g.offer_expired = x.expired;
            g.offer_price = credits_from(&x.price);
            Action::StarGift(g)
        }
        M::PaidMessagesRefunded(x) => Action::PaidMessagesRefunded {
            messages: x.count,
            stars: x.stars,
        },
        M::PaidMessagesPrice(x) => Action::PaidMessagesPrice {
            stars: x.stars as i32,
            broadcast_allowed: x.broadcast_messages_allowed,
        },
        M::TodoCompletions(x) => Action::TodoCompletions {
            completed: x.completed.clone(),
            incompleted: x.incompleted.clone(),
        },
        M::TodoAppendTasks(x) => Action::TodoAppendTasks {
            items: x.list.iter().map(parse_todo_item).collect(),
        },
        M::PollAppendAnswer(x) => Action::PollAppendAnswer {
            option: poll_answer_text(&x.answer),
        },
        M::PollDeleteAnswer(x) => Action::PollDeleteAnswer {
            option: poll_answer_text(&x.answer),
        },
        M::SuggestedPostApproval(x) => Action::SuggestedPostApproval {
            reject_comment: x.reject_comment.clone().unwrap_or_default(),
            schedule_date: x.schedule_date.unwrap_or(0),
            price: x.price.as_ref().map(credits_from).unwrap_or_default(),
            rejected: x.rejected,
            balance_too_low: x.balance_too_low,
        },
        M::SuggestedPostSuccess(x) => Action::SuggestedPostSuccess {
            price: credits_from(&x.price),
        },
        M::SuggestedPostRefund(x) => Action::SuggestedPostRefund {
            payer_initiated: x.payer_initiated,
        },
        M::SuggestBirthday(x) => {
            let E::Birthday::Birthday(b) = &x.birthday;
            Action::SuggestBirthday {
                day: b.day,
                month: b.month,
                year: b.year.unwrap_or(0),
            }
        }
        M::NewCreatorPending(x) => Action::NewCreatorPending {
            new_creator: x.new_creator_id,
        },
        M::ChangeCreator(x) => Action::ChangeCreator {
            new_creator: x.new_creator_id,
        },
        M::NoForwardsToggle(x) => Action::NoForwardsToggle {
            new_value: x.new_value,
        },
        M::NoForwardsRequest(x) => Action::NoForwardsRequest {
            expired: x.expired,
            new_value: x.new_value,
        },
        M::ManagedBotCreated(x) => Action::ManagedBotCreated { bot: x.bot_id },
        M::GramTransfer(x) => Action::GramTransfer {
            amount: x.amount,
            peer_address: x.peer_address.clone(),
            transaction_id: x.transaction_id.clone(),
            comment: x.comment.clone().unwrap_or_default(),
            comment_encrypted: x.comment_encrypted,
        },
        M::WalletTonConnectRequest(x) => Action::WalletTonConnectRequest {
            session_id: x.session_id as u64,
            expires: x.expires,
            topic: x.topic.clone().unwrap_or_default(),
            trace_id: x.trace_id.clone().unwrap_or_default(),
            accepted: x.accepted,
            declined: x.declined,
        },
        M::NoPaidMessagesTokenUsed => Action::NoPaidMessagesTokenUsed,
        _ => return None,
    })
}
