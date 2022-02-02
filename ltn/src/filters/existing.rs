use abstutil::{Tags, Timer};
use geom::Distance;
use map_gui::render::DrawMap;
use map_model::{osm, Direction, LaneSpec, LaneType, Map, Road};
use widgetry::EventCtx;

use crate::App;

/// Detect roads that're modelled in OSM as cycleways, but really are regular roads with modal
/// filters. Transform them into normal roads, and instead use this tool's explicit representation
/// for filters.
pub fn transform_existing_filters(ctx: &EventCtx, app: &mut App, timer: &mut Timer) {
    let mut edits = app.map.get_edits().clone();
    for r in detect_filters(&app.map) {
        edits.commands.push(app.map.edit_road_cmd(r.id, |new| {
            // Use a fixed [sidewalk, driving, driving, sidewalk] configuration
            let tags = Tags::empty();
            let fwd = vec![
                LaneSpec {
                    lt: LaneType::Driving,
                    dir: Direction::Fwd,
                    width: LaneSpec::typical_lane_widths(LaneType::Driving, &tags)[0].0,
                },
                LaneSpec {
                    lt: LaneType::Sidewalk,
                    dir: Direction::Fwd,
                    width: LaneSpec::typical_lane_widths(LaneType::Sidewalk, &tags)[0].0,
                },
            ];
            let back = vec![
                LaneSpec {
                    lt: LaneType::Driving,
                    dir: Direction::Back,
                    width: LaneSpec::typical_lane_widths(LaneType::Driving, &tags)[0].0,
                },
                LaneSpec {
                    lt: LaneType::Sidewalk,
                    dir: Direction::Back,
                    width: LaneSpec::typical_lane_widths(LaneType::Sidewalk, &tags)[0].0,
                },
            ];
            new.lanes_ltr = LaneSpec::assemble_ltr(fwd, back, app.map.get_config().driving_side);
        }));

        // Don't call before_edit; this transformation happens before the user starts editing
        app.session
            .modal_filters
            .roads
            .insert(r.id, r.length() / 2.0);
    }
    if edits.commands.is_empty() {
        return;
    }

    // TODO This is some of game's apply_map_edits
    let effects = app.map.must_apply_edits(edits, timer);
    app.draw_map.draw_all_unzoomed_roads_and_intersections =
        DrawMap::regenerate_unzoomed_layer(&app.map, &app.cs, ctx, timer);
    for r in effects.changed_roads {
        let road = app.map.get_r(r);
        app.draw_map.recreate_road(road, &app.map);
    }

    for i in effects.changed_intersections {
        app.draw_map.recreate_intersection(i, &app.map);
    }

    // The old pathfinder will not let driving paths cross the roads we just transformed. Why is it
    // valid to avoid recalculating? Look at all places where pathfinding is called:
    //
    // 1) In the pathfinding UI tool, both the 'before' and 'after' explicitly override
    //    RoutingParams, so we weren't using the built-in pathfinder anyway.
    // 2) The rat run detector also overrides RoutingParams with the current set of filters
    // 3) The impact tool does use the contraction hierarchy for the "before" count. This should be
    //    fine -- the situation represented before the roads are transformed is what we want.
    app.map.keep_pathfinder_despite_edits();
}

fn detect_filters(map: &Map) -> Vec<&Road> {
    let mut filtered_roads = Vec::new();
    'ROAD: for r in map.all_roads() {
        // A/B Street currently treats most footpaths as cycle-focused. Don't look at the lane
        // configuration; just look for this one tag. For example,
        // https://www.openstreetmap.org/way/392685069 is a highway=footway that is NOT a filtered
        // road.
        if !r.osm_tags.is(osm::HIGHWAY, "cycleway") {
            continue;
        }
        // A one-way cycleway is usually part of a complicated junction, like
        // https://www.openstreetmap.org/way/1002273098
        if r.osm_tags.is("oneway", "yes") {
            continue;
        }
        // Long cycleways are probably not physically driveable. Like
        // https://www.openstreetmap.org/way/174529602
        if r.length() > Distance::meters(20.0) {
            continue;
        }
        // Make sure both ends connect a driveable road, to avoid connections like
        // https://www.openstreetmap.org/way/881433973
        for i in [r.src_i, r.dst_i] {
            if !map
                .get_i(i)
                .roads
                .iter()
                .any(|r| map.get_r(*r).is_driveable())
            {
                continue 'ROAD;
            }
        }

        filtered_roads.push(r);
    }
    filtered_roads
}
