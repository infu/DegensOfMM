use std::cell::RefCell;

use candid::Principal as CandidPrincipal;
use domm_degens_schema::schema::{FactionDefinition, GameParticipant, GameSession, PlayerAccount};
use domm_game::{ApiError, ParticipantView, ResourceCost, SessionSummary, SessionView};
use icydb::{
    traits::EntityValue,
    types::{Id, Principal, Ulid},
};

use crate::repos::{content, players, sessions};

use super::session_turn_runtime;

#[derive(Clone, Debug)]
pub(crate) struct SessionCallerContext {
    pub session: GameSession,
    pub participant: GameParticipant,
}

#[derive(Clone, Debug)]
struct ActiveSessionCallerCacheEntry {
    caller_text: String,
    session_id: String,
    context: SessionCallerContext,
}

thread_local! {
    static ACTIVE_SESSION_CALLER_CACHE_A: RefCell<Option<ActiveSessionCallerCacheEntry>> =
        const { RefCell::new(None) };
    static ACTIVE_SESSION_CALLER_CACHE_B: RefCell<Option<ActiveSessionCallerCacheEntry>> =
        const { RefCell::new(None) };
}

pub(crate) fn require_session_caller(
    caller: CandidPrincipal,
    session_id: &str,
) -> Result<SessionCallerContext, ApiError> {
    reject_anonymous(caller)?;
    if let Some((session, participant)) =
        session_turn_runtime::caller_context_rows(&caller.to_text(), session_id)
    {
        return Ok(SessionCallerContext {
            session,
            participant,
        });
    }
    let player = require_player(caller)?;
    let session = load_session_from_text(session_id)?;
    let participant = sessions::find_participant_by_session_player(session.id(), player.id())?
        .ok_or_else(|| {
            public_error(
                "participant_not_found",
                "caller is not a participant in this session",
                false,
            )
        })?;
    let participant = session_turn_runtime::participant_snapshot(
        &session.id().to_string(),
        &participant.id().to_string(),
    )
    .unwrap_or(participant);
    Ok(SessionCallerContext {
        session,
        participant,
    })
}

pub(crate) fn require_active_session_caller(
    caller: CandidPrincipal,
    session_id: &str,
) -> Result<SessionCallerContext, ApiError> {
    let context = require_session_caller(caller, session_id)?;
    if context.session.state != "active" {
        return Err(public_error(
            "session_not_active",
            "session is not active",
            false,
        ));
    }
    Ok(context)
}

pub(crate) fn require_cached_active_session_caller(
    caller: CandidPrincipal,
    session_id: &str,
) -> Result<SessionCallerContext, ApiError> {
    reject_anonymous(caller)?;
    let caller_text = caller.to_text();
    if let Some(context) = cached_active_session_caller(&caller_text, session_id) {
        return Ok(context);
    }
    let context = require_active_session_caller(caller, session_id)?;
    remember_active_session_caller(caller, &context);
    Ok(context)
}

pub(crate) fn cached_active_session_caller_context(
    caller: CandidPrincipal,
    session_id: &str,
) -> Option<SessionCallerContext> {
    cached_active_session_caller(&caller.to_text(), session_id)
}

pub(crate) fn remember_active_session_caller(
    caller: CandidPrincipal,
    context: &SessionCallerContext,
) {
    if context.session.state != "active" {
        return;
    }
    let entry = ActiveSessionCallerCacheEntry {
        caller_text: caller.to_text(),
        session_id: context.session.id().to_string(),
        context: context.clone(),
    };
    let cached_a = ACTIVE_SESSION_CALLER_CACHE_A.with(|cache| cache.borrow().clone());
    if cached_a.as_ref().is_none_or(|cached| {
        cached.caller_text == entry.caller_text && cached.session_id == entry.session_id
    }) {
        ACTIVE_SESSION_CALLER_CACHE_A.with(|cache| *cache.borrow_mut() = Some(entry));
    } else {
        ACTIVE_SESSION_CALLER_CACHE_B.with(|cache| *cache.borrow_mut() = Some(entry));
    }
}

fn cached_active_session_caller(
    caller_text: &str,
    session_id: &str,
) -> Option<SessionCallerContext> {
    if let Some(context) = ACTIVE_SESSION_CALLER_CACHE_A.with(|cache| {
        cache.borrow().as_ref().and_then(|entry| {
            (entry.caller_text == caller_text
                && entry.session_id == session_id
                && entry.context.session.state == "active")
                .then(|| entry.context.clone())
        })
    }) {
        return Some(context);
    }
    ACTIVE_SESSION_CALLER_CACHE_B.with(|cache| {
        cache.borrow().as_ref().and_then(|entry| {
            (entry.caller_text == caller_text
                && entry.session_id == session_id
                && entry.context.session.state == "active")
                .then(|| entry.context.clone())
        })
    })
}

pub(crate) fn require_player(caller: CandidPrincipal) -> Result<PlayerAccount, ApiError> {
    players::find_by_principal(Principal::from(caller))?.ok_or_else(|| {
        public_error(
            "player_not_registered",
            "caller does not have a registered player",
            false,
        )
    })
}

pub(crate) fn load_session_from_text(session_id: &str) -> Result<GameSession, ApiError> {
    let id = parse_id::<GameSession>(session_id, "session_id")?;
    sessions::load_session(id)?.ok_or_else(|| {
        public_error(
            "session_not_found",
            format!("session was not found: {session_id}"),
            false,
        )
    })
}

pub(crate) fn participants_for_session(
    session_id: Id<GameSession>,
) -> Result<Vec<GameParticipant>, ApiError> {
    sessions::page_participants_by_session_status(
        session_id,
        "active",
        domm_game::MAX_LIST_LIMIT,
        None,
    )
    .map(|page| page.items)
}

pub(crate) fn session_summary(session: &GameSession) -> Result<SessionSummary, ApiError> {
    let session_view = session_view(session)?;
    Ok(SessionSummary::from_session(
        session_view,
        session.current_turn,
    ))
}

pub(crate) fn session_view(session: &GameSession) -> Result<SessionView, ApiError> {
    let mut participants = participants_for_session(session.id())?;
    session_view_from_participants(session, &mut participants)
}

pub(crate) fn session_view_from_participants(
    session: &GameSession,
    participants: &mut [GameParticipant],
) -> Result<SessionView, ApiError> {
    participants.sort_by_key(|participant| participant.slot_index);
    Ok(SessionView {
        session_id: session.id().to_string(),
        state: session.state.clone(),
        participant_ids: participants
            .into_iter()
            .map(|participant| participant.id().to_string())
            .collect(),
    })
}

pub(crate) fn participant_view(participant: &GameParticipant) -> Result<ParticipantView, ApiError> {
    let faction = content::load_faction(Id::<FactionDefinition>::from_key(participant.faction_id))?
        .ok_or_else(|| {
            public_error(
                "faction_not_found",
                "participant faction was not found",
                false,
            )
        })?;
    Ok(ParticipantView {
        participant_id: participant.id().to_string(),
        session_id: Id::<GameSession>::from_key(participant.session_id).to_string(),
        player_id: Id::<PlayerAccount>::from_key(participant.player_id).to_string(),
        faction_slug: faction.slug,
        slot_index: participant.slot_index,
        status: participant.status.clone(),
        ready: participant.ready_turn > 0,
        resources: ResourceCost {
            gold: participant.gold.try_into().unwrap_or(u32::MAX),
            wood: participant.wood,
            stone: participant.stone,
            iron: participant.iron,
            crystal: participant.crystal,
            ember: participant.ember,
            aether: participant.aether,
        },
    })
}

pub(crate) fn parse_id<E>(value: &str, field_name: &str) -> Result<Id<E>, ApiError>
where
    E: icydb::traits::EntityKey<Key = Ulid>,
{
    Ulid::from_str(value).map(Id::from_key).map_err(|_| {
        public_error(
            "invalid_id",
            format!("{field_name} is not a valid Ulid"),
            false,
        )
    })
}

pub(crate) fn reject_anonymous(caller: CandidPrincipal) -> Result<(), ApiError> {
    if caller == CandidPrincipal::anonymous() {
        Err(public_error(
            "anonymous_not_allowed",
            "anonymous callers cannot use player session endpoints",
            false,
        ))
    } else {
        Ok(())
    }
}

pub(crate) fn public_error(
    code: impl Into<String>,
    message: impl Into<String>,
    retryable: bool,
) -> ApiError {
    ApiError::new(code, message, retryable)
}
