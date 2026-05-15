use crate::map::{FirstPlayableMapState, SubjectViewResult};

use super::types::{
    NeutralArmyView, NeutralArmyViewResult, NeutralState, strength_label_for_quantity,
};

impl NeutralState {
    #[must_use]
    pub fn neutral_army_view_for(
        &self,
        map: &FirstPlayableMapState,
        participant_id: &str,
        neutral_army_id: &str,
        scouting: bool,
    ) -> NeutralArmyViewResult {
        let Some(army) = self
            .armies
            .iter()
            .find(|army| army.neutral_army_id == neutral_army_id)
        else {
            return NeutralArmyViewResult::NotFound {
                neutral_army_id: neutral_army_id.to_string(),
            };
        };
        let strength_label = strength_label_for_quantity(self.quantity_for(neutral_army_id));
        match map.subject_view(participant_id, "neutral_army", neutral_army_id) {
            SubjectViewResult::Visible(_) => NeutralArmyViewResult::Visible(NeutralArmyView {
                neutral_army_id: neutral_army_id.to_string(),
                visibility: "visible".to_string(),
                x: army.x,
                y: army.y,
                state: army.state.clone(),
                aggression: army.aggression.clone(),
                strength_label: strength_label.to_string(),
                exact_stacks: scouting
                    .then(|| self.stacks_for(neutral_army_id))
                    .unwrap_or_default(),
                redacted: !scouting,
            }),
            SubjectViewResult::LastKnown(view) => {
                NeutralArmyViewResult::LastKnown(NeutralArmyView {
                    neutral_army_id: neutral_army_id.to_string(),
                    visibility: "last_known".to_string(),
                    x: view.x,
                    y: view.y,
                    state: "last_known".to_string(),
                    aggression: "unknown".to_string(),
                    strength_label: strength_label.to_string(),
                    exact_stacks: Vec::new(),
                    redacted: true,
                })
            }
            SubjectViewResult::NotVisible { visibility, .. } => NeutralArmyViewResult::Hidden {
                neutral_army_id: neutral_army_id.to_string(),
                visibility,
            },
            SubjectViewResult::NotFound { .. } => NeutralArmyViewResult::NotFound {
                neutral_army_id: neutral_army_id.to_string(),
            },
        }
    }
}
