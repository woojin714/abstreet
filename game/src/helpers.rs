use std::collections::BTreeSet;

use abstutil::MapName;
use geom::{Duration, Polygon, Pt2D};
use map_model::{AreaID, BuildingID, BusStopID, IntersectionID, LaneID, Map, ParkingLotID, RoadID};
use sim::{AgentID, AgentType, CarID, PedestrianID, TripMode, TripPhaseType};
use widgetry::{Btn, Checkbox, Color, EventCtx, GfxCtx, Key, Line, Text, TextSpan, Widget};

use crate::app::{App, PerMap};

// Aside from Road and Trip, everything here can actually be selected.
#[derive(Clone, Hash, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum ID {
    Road(RoadID),
    Lane(LaneID),
    Intersection(IntersectionID),
    Building(BuildingID),
    ParkingLot(ParkingLotID),
    Car(CarID),
    Pedestrian(PedestrianID),
    PedCrowd(Vec<PedestrianID>),
    BusStop(BusStopID),
    Area(AreaID),
}

impl ID {
    pub fn from_agent(id: AgentID) -> ID {
        match id {
            AgentID::Car(id) => ID::Car(id),
            AgentID::Pedestrian(id) => ID::Pedestrian(id),
            AgentID::BusPassenger(_, bus) => ID::Car(bus),
        }
    }

    pub fn agent_id(&self) -> Option<AgentID> {
        match *self {
            ID::Car(id) => Some(AgentID::Car(id)),
            ID::Pedestrian(id) => Some(AgentID::Pedestrian(id)),
            // PedCrowd doesn't map to a single agent.
            _ => None,
        }
    }

    pub fn canonical_point(&self, primary: &PerMap) -> Option<Pt2D> {
        match *self {
            ID::Road(id) => primary.map.maybe_get_r(id).map(|r| r.center_pts.first_pt()),
            ID::Lane(id) => primary.map.maybe_get_l(id).map(|l| l.first_pt()),
            ID::Intersection(id) => primary.map.maybe_get_i(id).map(|i| i.polygon.center()),
            ID::Building(id) => primary.map.maybe_get_b(id).map(|b| b.polygon.center()),
            ID::ParkingLot(id) => primary.map.maybe_get_pl(id).map(|pl| pl.polygon.center()),
            ID::Car(id) => primary
                .sim
                .canonical_pt_for_agent(AgentID::Car(id), &primary.map),
            ID::Pedestrian(id) => primary
                .sim
                .canonical_pt_for_agent(AgentID::Pedestrian(id), &primary.map),
            ID::PedCrowd(ref members) => primary
                .sim
                .canonical_pt_for_agent(AgentID::Pedestrian(members[0]), &primary.map),
            ID::BusStop(id) => primary
                .map
                .maybe_get_bs(id)
                .map(|bs| bs.sidewalk_pos.pt(&primary.map)),
            ID::Area(id) => primary.map.maybe_get_a(id).map(|a| a.polygon.center()),
        }
    }
}

pub fn list_names<F: Fn(TextSpan) -> TextSpan>(txt: &mut Text, styler: F, names: BTreeSet<String>) {
    let len = names.len();
    for (idx, n) in names.into_iter().enumerate() {
        if idx != 0 {
            if idx == len - 1 {
                if len == 2 {
                    txt.append(Line(" and "));
                } else {
                    txt.append(Line(", and "));
                }
            } else {
                txt.append(Line(", "));
            }
        }
        txt.append(styler(Line(n)));
    }
}

// TODO Associate this with maps, but somehow avoid reading the entire file when listing them.
pub fn nice_map_name(name: &MapName) -> &str {
    match (name.city.as_ref(), name.map.as_ref()) {
        ("seattle", "ballard") => "Ballard",
        ("seattle", "downtown") => "Downtown Seattle",
        ("seattle", "huge_seattle") => "Seattle (entire area)",
        ("seattle", "lakeslice") => "Lake Washington corridor",
        ("seattle", "montlake") => "Montlake and Eastlake",
        ("seattle", "south_seattle") => "South Seattle",
        ("seattle", "udistrict") => "University District",
        ("seattle", "west_seattle") => "West Seattle",
        ("berlin", "center") => "Berlin (city center)",
        ("krakow", "center") => "Kraków (city center)",
        ("leeds", "center") => "Leeds (city center)",
        ("london", "southbank") => "London (Southbank)",
        ("tel_aviv", "center") => "Tel Aviv (city center)",
        ("xian", "center") => "Xi'an (city center)",
        _ => &name.map,
    }
}

// Shorter is better
pub fn cmp_duration_shorter(app: &App, after: Duration, before: Duration) -> Vec<TextSpan> {
    if after.epsilon_eq(before) {
        vec![Line("same")]
    } else if after < before {
        vec![
            Line((before - after).to_string(&app.opts.units)).fg(Color::GREEN),
            Line(" faster"),
        ]
    } else if after > before {
        vec![
            Line((after - before).to_string(&app.opts.units)).fg(Color::RED),
            Line(" slower"),
        ]
    } else {
        unreachable!()
    }
}

pub fn color_for_mode(app: &App, m: TripMode) -> Color {
    match m {
        TripMode::Walk => app.cs.unzoomed_pedestrian,
        TripMode::Bike => app.cs.unzoomed_bike,
        TripMode::Transit => app.cs.unzoomed_bus,
        TripMode::Drive => app.cs.unzoomed_car,
    }
}

pub fn color_for_agent_type(app: &App, a: AgentType) -> Color {
    match a {
        AgentType::Pedestrian => app.cs.unzoomed_pedestrian,
        AgentType::Bike => app.cs.unzoomed_bike,
        AgentType::Bus | AgentType::Train => app.cs.unzoomed_bus,
        AgentType::TransitRider => app.cs.bus_trip,
        AgentType::Car => app.cs.unzoomed_car,
    }
}

pub fn color_for_trip_phase(app: &App, tpt: TripPhaseType) -> Color {
    match tpt {
        TripPhaseType::Driving => app.cs.unzoomed_car,
        TripPhaseType::Walking => app.cs.unzoomed_pedestrian,
        TripPhaseType::Biking => app.cs.bike_trip,
        TripPhaseType::Parking => app.cs.parking_trip,
        TripPhaseType::WaitingForBus(_, _) => app.cs.bus_layer,
        TripPhaseType::RidingBus(_, _, _) => app.cs.bus_trip,
        TripPhaseType::Cancelled | TripPhaseType::Finished => unreachable!(),
        TripPhaseType::DelayedStart => Color::YELLOW,
        TripPhaseType::Remote => Color::PINK,
    }
}

pub fn amenity_type(a: &str) -> Option<&'static str> {
    // NOTE: names are used in amenities function in other file
    // TODO: create categories for:
    // hairdresser beauty chemist
    // car_repair
    // laundry
    if a == "supermarket" || a == "convenience" {
        Some("groceries")
    } else if a == "restaurant"
        || a == "cafe"
        || a == "fast_food"
        || a == "food_court"
        || a == "ice_cream"
        || a == "pastry"
        || a == "deli"
        || a == "greengrocer"
        || a == "bakery"
        || a == "butcher"
        || a == "confectionery"
        || a == "beverages"
        || a == "alcohol"
    {
        Some("food")
    } else if a == "pub" || a == "bar" || a == "nightclub" || a == "lounge" {
        Some("bar")
    } else if a == "doctors"
        || a == "dentist"
        || a == "clinic"
        || a == "hospital"
        || a == "pharmacy"
        || a == "chiropractor"
        || a == "optician"
    {
        Some("medical")
    } else if a == "place_of_worship" {
        Some("church / temple")
    } else if a == "college" || a == "school" || a == "university" {
        Some("education")
    } else if a == "bank" || a == "post_office" {
        Some("bank / post office")
    } else if a == "theatre"
        || a == "arts_centre"
        || a == "library"
        || a == "cinema"
        || a == "art_gallery"
        || a == "museum"
    {
        Some("culture")
    } else if a == "childcare" || a == "kindergarten" {
        Some("childcare")
    } else if a == "second_hand"
        || a == "clothes"
        || a == "furniture"
        || a == "shoes"
        || a == "department_store"
        || a == "car"
        || a == "kiosk"
        || a == "hardware"
        || a == "mobile_phone"
        || a == "florist"
        || a == "electronics"
        || a == "car_parts"
        || a == "doityourself"
        || a == "jewelry"
        || a == "variety_store"
        || a == "gift"
        || a == "bicycle"
        || a == "books"
        || a == "sports"
        || a == "travel_agency"
        || a == "stationery"
        || a == "pet"
        || a == "computer"
        || a == "tyres"
        || a == "newsagent"
    {
        Some("shopping")
    } else {
        None
    }
}

// TODO Well, there goes the nice consolidation of stuff in BtnBuilder. :\
pub fn hotkey_btn<I: Into<String>>(ctx: &EventCtx, app: &App, label: I, key: Key) -> Widget {
    let label = label.into();
    let mut txt = Text::new();
    txt.append(key.txt(ctx));
    txt.append(Line(format!(" - {}", label)));
    Btn::text_bg(label, txt, app.cs.section_bg, app.cs.hovering).build_def(ctx, key)
}

pub fn intersections_from_roads(roads: &BTreeSet<RoadID>, map: &Map) -> BTreeSet<IntersectionID> {
    let mut results = BTreeSet::new();
    for r in roads {
        let r = map.get_r(*r);
        for i in vec![r.src_i, r.dst_i] {
            if results.contains(&i) {
                continue;
            }
            if map.get_i(i).roads.iter().all(|r| roads.contains(r)) {
                results.insert(i);
            }
        }
    }
    results
}

pub fn checkbox_per_mode(
    ctx: &mut EventCtx,
    app: &App,
    current_state: &BTreeSet<TripMode>,
) -> Widget {
    let mut filters = Vec::new();
    for m in TripMode::all() {
        filters.push(
            Checkbox::colored(
                ctx,
                m.ongoing_verb(),
                color_for_mode(app, m),
                current_state.contains(&m),
            )
            .margin_right(24),
        );
    }
    Widget::custom_row(filters)
}

pub fn open_browser(url: String) {
    let _ = webbrowser::open(&url);
}

pub fn loading_tips() -> Text {
    Text::from_multiline(vec![
        Line("Recent changes (November 8)"),
        Line(""),
        Line("- Download more cities from within the game"),
        Line("- You can now click agents while zoomed out"),
        Line("- New OpenStreetMap viewer, open it from the splash screen"),
        Line("- A web version has launched!"),
        Line("- Slow segments of a trip shown in the info panel"),
        Line("- Alleyways are now included in the map"),
        Line("- Check out the trip tables and summary changes (press 'q')"),
        Line("- Try out the new traffic signal editor!"),
    ])
}

/// Make it clear the map can't be interacted with right now.
pub fn grey_out_map(g: &mut GfxCtx, app: &App) {
    g.fork_screenspace();
    // TODO - OSD height
    g.draw_polygon(
        app.cs.fade_map_dark,
        Polygon::rectangle(g.canvas.window_width, g.canvas.window_height),
    );
    g.unfork();
}