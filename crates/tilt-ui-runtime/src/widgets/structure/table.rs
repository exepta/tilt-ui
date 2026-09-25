//! Grid-backed table layout derived from authored rows and cells.

use bevy::{
    ecs::{component::Component, entity::Entity, hierarchy::Children, world::World},
    ui::{Display, GridPlacement, Node, RepeatedGridTrack},
};

use crate::widgets::structure::table_cell::TableCellInfo;

/// Number of columns inferred from the widest authored row.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableInfo {
    pub columns: usize,
    pub rows: usize,
}

pub(crate) fn finish(world: &mut World, table: Entity) {
    let cells = world
        .get::<Children>(table)
        .map(|children| {
            children
                .iter()
                .copied()
                .filter_map(|entity| {
                    world
                        .get::<TableCellInfo>(entity)
                        .copied()
                        .map(|info| (entity, info))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let columns = cells
        .iter()
        .filter_map(|(_, cell)| cell.column.map(|column| column + 1))
        .max()
        .unwrap_or(cells.len());
    let rows = cells
        .iter()
        .filter_map(|(_, cell)| cell.row.map(|row| row + 1))
        .max()
        .unwrap_or(usize::from(!cells.is_empty()));
    if let Some(mut node) = world.get_mut::<Node>(table) {
        node.display = Display::Grid;
        node.grid_template_columns = vec![RepeatedGridTrack::flex(
            u16::try_from(columns.max(1)).unwrap_or(u16::MAX),
            1.0,
        )];
    }
    world.entity_mut(table).insert(TableInfo { columns, rows });
    for (index, (entity, cell)) in cells.into_iter().enumerate() {
        if let Some(mut node) = world.get_mut::<Node>(entity) {
            let column = cell.column.unwrap_or(index % columns.max(1));
            let row = cell.row.unwrap_or(index / columns.max(1));
            node.grid_column = GridPlacement::start(i16::try_from(column + 1).unwrap_or(i16::MAX));
            node.grid_row = GridPlacement::start(i16::try_from(row + 1).unwrap_or(i16::MAX));
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        ecs::world::World,
        ui::{Display, GridPlacement, Node},
    };

    use super::*;

    #[test]
    fn widest_row_sets_columns_and_cells_keep_explicit_positions() {
        let mut world = World::new();
        let table = world.spawn(Node::default()).id();
        let first = world
            .spawn((
                Node::default(),
                TableCellInfo {
                    row: Some(0),
                    column: Some(0),
                    ..Default::default()
                },
            ))
            .id();
        let second = world
            .spawn((
                Node::default(),
                TableCellInfo {
                    row: Some(0),
                    column: Some(1),
                    ..Default::default()
                },
            ))
            .id();
        let third = world
            .spawn((
                Node::default(),
                TableCellInfo {
                    row: Some(1),
                    column: Some(0),
                    ..Default::default()
                },
            ))
            .id();
        world
            .entity_mut(table)
            .add_children(&[first, second, third]);

        finish(&mut world, table);

        assert_eq!(world.get::<TableInfo>(table).unwrap().columns, 2);
        assert_eq!(world.get::<TableInfo>(table).unwrap().rows, 2);
        assert_eq!(world.get::<Node>(table).unwrap().display, Display::Grid);
        assert_eq!(
            world.get::<Node>(second).unwrap().grid_column,
            GridPlacement::start(2)
        );
        assert_eq!(
            world.get::<Node>(third).unwrap().grid_row,
            GridPlacement::start(2)
        );
    }
}
