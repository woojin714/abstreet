use std::collections::BTreeSet;

use geom::Time;
use map_model::{IntersectionID, Map, PathStep, Position, Traversable};
use synthpop::{IndividTrip, PersonSpec, Scenario, TripEndpoint, TripMode, TripPurpose};

use crate::{AgentID, DrivingSimState, Event, TripID, VehicleType};

/// Records trips beginning and ending at a specified set of intersections. This can be used to
/// capture and reproduce behavior in a gridlock-prone chunk of the map, without simulating
/// everything.
#[derive(Clone)]
pub(crate) struct TrafficRecorder {
    capture_points: BTreeSet<IntersectionID>,
    // TODO The RNG will determine vehicle length, so this won't be a perfect capture. Hopefully
    // good enough.
    trips: Vec<IndividTrip>,
    seen_trips: BTreeSet<TripID>,
}

impl TrafficRecorder {
    pub fn new(capture_points: BTreeSet<IntersectionID>) -> TrafficRecorder {
        TrafficRecorder {
            capture_points,
            trips: Vec::new(),
            seen_trips: BTreeSet::new(),
        }
    }

    pub fn handle_event(&mut self, time: Time, ev: &Event, map: &Map, driving: &DrivingSimState) {
        if let Event::AgentEntersTraversable(AgentID::Car(car), Some(trip), on, _) = ev {
            if self.seen_trips.contains(trip) {
                return;
            }
            if let Traversable::Lane(l) = on {
                if self.capture_points.contains(&map.get_l(*l).src_i) {
                    // Where do they exit?
                    for step in driving.get_path(*car).unwrap().get_steps() {
                        if let PathStep::Turn(t) = step {
                            if self.capture_points.contains(&t.parent) {
                                self.trips.push(IndividTrip::new(
                                    time,
                                    TripPurpose::Shopping,
                                    TripEndpoint::SuddenlyAppear(Position::start(*l)),
                                    TripEndpoint::Border(t.parent),
                                    if car.vehicle_type == VehicleType::Bike {
                                        TripMode::Bike
                                    } else {
                                        TripMode::Drive
                                    },
                                ));
                                self.seen_trips.insert(*trip);
                                return;
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn num_recorded_trips(&self) -> usize {
        self.trips.len()
    }

    pub fn save(mut self, map: &Map) {
        let mut people = Vec::new();
        for trip in self.trips.drain(..) {
            people.push(PersonSpec {
                orig_id: None,
                trips: vec![trip],
            });
        }
        Scenario {
            scenario_name: "recorded".to_string(),
            map_name: map.get_name().clone(),
            people,
            only_seed_buses: None,
        }
        .save();
    }
}
