use candid::CandidType;
use domm_game::{
    ApiError, ApiEventPage, GameView, MapChunkView, ObjectView, PlayableError, Viewport,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ClientOpeningViewport {
    pub game_view: GameView,
    pub viewport: Viewport,
    pub chunks: Vec<MapChunkView>,
    pub objects: Vec<ObjectView>,
    pub events: ApiEventPage,
    pub sync_required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct RenderedViewport {
    pub width: u16,
    pub height: u16,
    pub rows: Vec<String>,
    pub visible_champions: Vec<String>,
    pub visible_towns: Vec<String>,
    pub visible_resources: Vec<String>,
    pub visible_neutrals: Vec<String>,
    pub event_summaries: Vec<String>,
    pub sync_required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum ProbeError {
    #[error("api error: {0:?}")]
    Api(ApiError),
    #[error("participant slot {slot_index} does not have an opening viewport")]
    MissingOpeningViewport { slot_index: u8 },
    #[error("no public chunk DTO covers tile ({x},{y})")]
    MissingChunkForTile { x: u16, y: u16 },
    #[error("chunk DTO missing blob cell {cell_index}")]
    MissingChunkCell { cell_index: usize },
    #[error("rendered row is not valid UTF-8")]
    InvalidRenderedRow,
    #[error("session has not been created")]
    MissingSession,
    #[error("game view has not been loaded")]
    MissingGameView,
    #[error("battle panel has no active stack")]
    MissingActiveBattleStack,
    #[error("command {command_type} failed with {code}")]
    CommandFailed { command_type: String, code: String },
    #[error("retry for {command_type} did not replay the original command")]
    RetryDidNotReplay { command_type: String },
    #[error("playable gate error: {0}")]
    Playable(PlayableError),
}

impl From<ApiError> for ProbeError {
    fn from(error: ApiError) -> Self {
        Self::Api(error)
    }
}

impl From<PlayableError> for ProbeError {
    fn from(error: PlayableError) -> Self {
        Self::Playable(error)
    }
}
