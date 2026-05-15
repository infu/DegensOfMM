use crate::content::{
    FIRST_PLAYABLE_CHUNK_SIZE, FIRST_PLAYABLE_MAP_HEIGHT, FIRST_PLAYABLE_MAP_WIDTH,
};

use super::types::{FirstPlayableMapState, MapError, MapOccupancyRecord};

impl FirstPlayableMapState {
    pub fn insert_occupancy_footprint(
        &mut self,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        layer: &str,
        occupant_kind: &str,
        occupant_id_text: &str,
        blocking: bool,
        last_command_id: Option<String>,
    ) -> Result<Vec<MapOccupancyRecord>, MapError> {
        let mut rows = Vec::with_capacity(usize::from(width) * usize::from(height));
        for dy in 0..height {
            for dx in 0..width {
                let tile_x = x + dx;
                let tile_y = y + dy;
                let occupant_cell_index = dy * width + dx;
                ensure_in_bounds(tile_x, tile_y)?;
                self.ensure_occupancy_available(
                    tile_x,
                    tile_y,
                    layer,
                    occupant_kind,
                    occupant_id_text,
                    occupant_cell_index,
                )?;
                rows.push(make_occupancy_row(
                    &self.session_id,
                    tile_x,
                    tile_y,
                    layer,
                    occupant_kind,
                    occupant_id_text,
                    occupant_cell_index,
                    blocking,
                    last_command_id.clone(),
                ));
            }
        }

        self.occupancy_rows.extend(rows.clone());
        Ok(rows)
    }

    pub fn cleanup_occupancy_by_subject(
        &mut self,
        occupant_kind: &str,
        occupant_id_text: &str,
    ) -> usize {
        let before = self.occupancy_rows.len();
        self.occupancy_rows.retain(|row| {
            row.occupant_kind != occupant_kind || row.occupant_id_text != occupant_id_text
        });
        before - self.occupancy_rows.len()
    }

    fn ensure_occupancy_available(
        &self,
        x: u16,
        y: u16,
        layer: &str,
        occupant_kind: &str,
        occupant_id_text: &str,
        occupant_cell_index: u16,
    ) -> Result<(), MapError> {
        if self
            .occupancy_rows
            .iter()
            .any(|row| row.x == x && row.y == y && row.layer == layer)
        {
            return Err(MapError::OccupancyTileCollision {
                x,
                y,
                layer: layer.to_string(),
            });
        }
        if self.occupancy_rows.iter().any(|row| {
            row.occupant_kind == occupant_kind
                && row.occupant_id_text == occupant_id_text
                && row.occupant_cell_index == occupant_cell_index
        }) {
            return Err(MapError::OccupancyCellCollision {
                occupant_kind: occupant_kind.to_string(),
                occupant_id_text: occupant_id_text.to_string(),
                occupant_cell_index,
            });
        }
        Ok(())
    }
}

fn make_occupancy_row(
    session_id: &str,
    x: u16,
    y: u16,
    layer: &str,
    occupant_kind: &str,
    occupant_id_text: &str,
    occupant_cell_index: u16,
    blocking: bool,
    last_command_id: Option<String>,
) -> MapOccupancyRecord {
    MapOccupancyRecord {
        occupancy_id: format!(
            "occ:{session_id}:{occupant_kind}:{occupant_id_text}:{occupant_cell_index}"
        ),
        session_id: session_id.to_string(),
        x,
        y,
        chunk_x: chunk_coord(x),
        chunk_y: chunk_coord(y),
        layer: layer.to_string(),
        occupant_kind: occupant_kind.to_string(),
        occupant_id_text: occupant_id_text.to_string(),
        occupant_cell_index,
        blocking,
        last_command_id,
    }
}

fn ensure_in_bounds(x: u16, y: u16) -> Result<(), MapError> {
    if x >= FIRST_PLAYABLE_MAP_WIDTH || y >= FIRST_PLAYABLE_MAP_HEIGHT {
        return Err(MapError::OutOfBounds { x, y });
    }
    Ok(())
}

pub(crate) fn chunk_coord(value: u16) -> u16 {
    value / u16::from(FIRST_PLAYABLE_CHUNK_SIZE)
}
