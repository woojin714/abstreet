use std::collections::{BTreeMap, BTreeSet};

use maplit::btreeset;

use abstutil::prettyprint_usize;
use geom::{Duration, Time};
use map_gui::tools::{
    checkbox_per_mode, grey_out_map, ChooseSomething, CityPicker, PopupMsg, URLManager,
};
use sim::SlidingWindow;
use synthpop::{ScenarioModifier, TripMode};
use widgetry::{
    lctrl, Choice, Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, LinePlot, Outcome,
    Panel, PlotOptions, Series, SimpleState, Slider, Spinner, State, Text, TextExt,
    VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::edit::EditMode;
use crate::sandbox::gameplay::freeform::ChangeScenario;
use crate::sandbox::gameplay::{GameplayMode, GameplayState};
use crate::sandbox::{Actions, SandboxControls, SandboxMode, TimeWarpScreen};

pub struct PlayScenario {
    top_right: Panel,
    scenario_name: String,
    modifiers: Vec<ScenarioModifier>,
}

impl PlayScenario {
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &App,
        name: &str,
        modifiers: Vec<ScenarioModifier>,
    ) -> Box<dyn GameplayState> {
        URLManager::update_url_free_param(
            // For dynamiclly generated scenarios like "random" and "home_to_work", this winds up
            // making up a filename that doesn't actually exist. But if you pass that in, it winds
            // up working, because we call abstio::parse_scenario_path() on the other side.
            abstio::path_scenario(app.primary.map.get_name(), name)
                .strip_prefix(&abstio::path(""))
                .unwrap()
                .to_string(),
        );

        Box::new(PlayScenario {
            top_right: Panel::empty(ctx),
            scenario_name: name.to_string(),
            modifiers,
        })
    }
}

impl GameplayState for PlayScenario {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        _: &mut SandboxControls,
        _: &mut Actions,
    ) -> Option<Transition> {
        // This should really happen in the constructor once, but the old PlayScenario's
        // on_destroy can wipe this out.
        app.primary.has_modified_trips = !self.modifiers.is_empty();

        match self.top_right.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "change map" => {
                    let scenario = self.scenario_name.clone();
                    Some(Transition::Push(CityPicker::new_state(
                        ctx,
                        app,
                        Box::new(move |_, app| {
                            // Try to load a scenario with the same name if it exists
                            let mode = if abstio::file_exists(abstio::path_scenario(
                                app.primary.map.get_name(),
                                &scenario,
                            )) {
                                GameplayMode::PlayScenario(
                                    app.primary.map.get_name().clone(),
                                    scenario,
                                    Vec::new(),
                                )
                            } else {
                                GameplayMode::Freeform(app.primary.map.get_name().clone())
                            };
                            Transition::Multi(vec![
                                Transition::Pop,
                                Transition::Replace(SandboxMode::simple_new(app, mode)),
                            ])
                        }),
                    )))
                }
                "change scenario" => Some(Transition::Push(ChangeScenario::new_state(
                    ctx,
                    app,
                    &self.scenario_name,
                ))),
                "edit map" => Some(Transition::Push(EditMode::new_state(
                    ctx,
                    app,
                    GameplayMode::PlayScenario(
                        app.primary.map.get_name().clone(),
                        self.scenario_name.clone(),
                        self.modifiers.clone(),
                    ),
                ))),
                "edit traffic patterns" => {
                    Some(Transition::Push(EditScenarioModifiers::new_state(
                        ctx,
                        self.scenario_name.clone(),
                        self.modifiers.clone(),
                    )))
                }
                "save scenario" => {
                    let mut s = app.primary.scenario.as_ref().unwrap().clone();
                    // If the name happens to be random, home_to_work, or census (the 3
                    // dynamically generated cases), it'll get covered up. So to be safe, rename
                    // it.
                    s.scenario_name = format!("saved_{}", s.scenario_name);
                    s.save();
                    Some(Transition::Push(PopupMsg::new_state(
                        ctx,
                        "Saved",
                        vec![format!("Scenario '{}' saved", s.scenario_name)],
                    )))
                }
                "When do trips start?" => {
                    Some(Transition::Push(DepartureSummary::new_state(ctx, app)))
                }
                _ => unreachable!(),
            },
            _ => None,
        }
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.top_right.draw(g);
    }

    fn on_destroy(&self, app: &mut App) {
        app.primary.has_modified_trips = false;
    }

    fn recreate_panels(&mut self, ctx: &mut EventCtx, app: &App) {
        let mut extra = Vec::new();
        if self.scenario_name != "empty" {
            extra.push(Widget::row(vec![
                ctx.style()
                    .btn_plain
                    .icon("system/assets/tools/info.svg")
                    .build_widget(ctx, "When do trips start?")
                    .centered_vert(),
                ctx.style()
                    .btn_plain
                    .icon("system/assets/tools/pencil.svg")
                    .build_widget(ctx, "edit traffic patterns")
                    .centered_vert(),
                format!("{} modifications to traffic patterns", self.modifiers.len())
                    .text_widget(ctx)
                    .centered_vert(),
            ]));
        }
        if !abstio::file_exists(abstio::path_scenario(
            app.primary.map.get_name(),
            &self.scenario_name,
        )) && app.primary.scenario.is_some()
        {
            extra.push(
                ctx.style()
                    .btn_plain
                    .icon("system/assets/tools/save.svg")
                    .label_text("save scenario")
                    .build_def(ctx),
            );
        }

        let rows = vec![
            Widget::custom_row(vec![
                Line("Sandbox")
                    .small_heading()
                    .into_widget(ctx)
                    .margin_right(18),
                map_gui::tools::change_map_btn(ctx, app).margin_right(8),
                ctx.style()
                    .btn_popup_icon_text("system/assets/tools/calendar.svg", &self.scenario_name)
                    .hotkey(Key::S)
                    .build_widget(ctx, "change scenario")
                    .margin_right(8),
                ctx.style()
                    .btn_outline
                    .icon_text("system/assets/tools/pencil.svg", "Edit map")
                    .hotkey(lctrl(Key::E))
                    .build_widget(ctx, "edit map")
                    .margin_right(8),
            ])
            .centered(),
            if extra.is_empty() {
                Widget::nothing()
            } else {
                Widget::row(extra).centered_horiz()
            },
        ];

        self.top_right = Panel::new_builder(Widget::col(rows))
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
            .build(ctx);
    }
}

struct EditScenarioModifiers {
    scenario_name: String,
    modifiers: Vec<ScenarioModifier>,
    panel: Panel,
}

impl EditScenarioModifiers {
    pub fn new_state(
        ctx: &mut EventCtx,
        scenario_name: String,
        modifiers: Vec<ScenarioModifier>,
    ) -> Box<dyn State<App>> {
        let mut rows = vec![
            Line("Modify traffic patterns")
                .small_heading()
                .into_widget(ctx),
            Text::from(
                "This scenario determines the exact trips everybody takes, when they leave, where \
                 they go, and how they choose to get there. You can modify those patterns here. \
                 The modifications apply in order.",
            )
            .wrap_to_pct(ctx, 50)
            .into_widget(ctx),
        ];
        for (idx, m) in modifiers.iter().enumerate() {
            rows.push(
                Widget::row(vec![
                    m.describe().text_widget(ctx).centered_vert(),
                    ctx.style()
                        .btn_solid_destructive
                        .icon("system/assets/tools/trash.svg")
                        .build_widget(ctx, format!("delete modifier {}", idx + 1))
                        .align_right(),
                ])
                .padding(10)
                .outline(ctx.style().section_outline),
            );
        }
        rows.push(
            ctx.style()
                .btn_outline
                .text("Change trip mode")
                .build_def(ctx),
        );
        rows.push(
            ctx.style()
                .btn_outline
                .text("Add extra new trips")
                .build_def(ctx),
        );
        rows.push(Widget::row(vec![
            Spinner::widget(ctx, "repeat_days", (2, 14), 2_usize, 1),
            ctx.style()
                .btn_outline
                .text("Repeat schedule multiple days")
                .build_def(ctx),
        ]));
        rows.push(Widget::horiz_separator(ctx, 1.0));
        rows.push(
            Widget::row(vec![
                ctx.style()
                    .btn_solid_primary
                    .text("Apply")
                    .hotkey(Key::Enter)
                    .build_def(ctx),
                ctx.style()
                    .btn_solid_destructive
                    .text("Discard changes")
                    .hotkey(Key::Escape)
                    .build_def(ctx),
            ])
            .centered(),
        );

        Box::new(EditScenarioModifiers {
            scenario_name,
            modifiers,
            panel: Panel::new_builder(Widget::col(rows))
                .exact_size_percent(80, 80)
                .build(ctx),
        })
    }
}

impl State<App> for EditScenarioModifiers {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "Discard changes" => {
                    return Transition::Pop;
                }
                "Apply" => {
                    info!("To apply these modifiers in the future:");
                    info!(
                        "--scenario_modifiers='{}'",
                        abstutil::to_json_terse(&self.modifiers)
                    );

                    return Transition::Multi(vec![
                        Transition::Pop,
                        Transition::Replace(SandboxMode::simple_new(
                            app,
                            GameplayMode::PlayScenario(
                                app.primary.map.get_name().clone(),
                                self.scenario_name.clone(),
                                self.modifiers.clone(),
                            ),
                        )),
                    ]);
                }
                "Change trip mode" => {
                    return Transition::Push(ChangeMode::new_state(
                        ctx,
                        app,
                        self.scenario_name.clone(),
                        self.modifiers.clone(),
                    ));
                }
                "Add extra new trips" => {
                    return Transition::Push(ChooseSomething::new_state(
                        ctx,
                        "Which trips do you want to add in?",
                        // TODO Exclude weekday?
                        Choice::strings(abstio::list_all_objects(abstio::path_all_scenarios(
                            app.primary.map.get_name(),
                        ))),
                        Box::new(|name, _, _| {
                            Transition::Multi(vec![
                                Transition::Pop,
                                Transition::ConsumeState(Box::new(|state, ctx, _| {
                                    let mut state =
                                        state.downcast::<EditScenarioModifiers>().ok().unwrap();
                                    state.modifiers.push(ScenarioModifier::AddExtraTrips(name));
                                    vec![EditScenarioModifiers::new_state(
                                        ctx,
                                        state.scenario_name,
                                        state.modifiers,
                                    )]
                                })),
                            ])
                        }),
                    ));
                }
                "Repeat schedule multiple days" => {
                    self.modifiers.push(ScenarioModifier::RepeatDays(
                        self.panel.spinner("repeat_days"),
                    ));
                    return Transition::Replace(EditScenarioModifiers::new_state(
                        ctx,
                        self.scenario_name.clone(),
                        self.modifiers.clone(),
                    ));
                }
                x => {
                    if let Some(x) = x.strip_prefix("delete modifier ") {
                        self.modifiers.remove(x.parse::<usize>().unwrap() - 1);
                        return Transition::Replace(EditScenarioModifiers::new_state(
                            ctx,
                            self.scenario_name.clone(),
                            self.modifiers.clone(),
                        ));
                    } else {
                        unreachable!()
                    }
                }
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        grey_out_map(g, app);
        self.panel.draw(g);
    }
}

struct ChangeMode {
    panel: Panel,
    scenario_name: String,
    modifiers: Vec<ScenarioModifier>,
    count_trips: CountTrips,
}

impl ChangeMode {
    fn new_state(
        ctx: &mut EventCtx,
        app: &App,
        scenario_name: String,
        modifiers: Vec<ScenarioModifier>,
    ) -> Box<dyn State<App>> {
        let mut state = ChangeMode {
            scenario_name,
            modifiers,
            count_trips: CountTrips::new(app),
            panel: Panel::new_builder(Widget::col(vec![
                Line("Change trip mode").small_heading().into_widget(ctx),
                Widget::row(vec![
                    "Percent of people to modify:"
                        .text_widget(ctx)
                        .centered_vert(),
                    Spinner::widget(ctx, "pct_ppl", (1, 100), 50_usize, 1),
                ]),
                "Types of trips to convert:".text_widget(ctx),
                checkbox_per_mode(ctx, app, &btreeset! { TripMode::Drive }),
                Widget::row(vec![
                    "Departing from:".text_widget(ctx),
                    Slider::area(ctx, 0.25 * ctx.canvas.window_width, 0.0, "depart from"),
                ]),
                Widget::row(vec![
                    "Departing until:".text_widget(ctx),
                    Slider::area(ctx, 0.25 * ctx.canvas.window_width, 0.3, "depart to"),
                ]),
                "Matching trips:".text_widget(ctx).named("count"),
                Widget::horiz_separator(ctx, 1.0),
                Widget::row(vec![
                    "Change to trip type:".text_widget(ctx),
                    Widget::dropdown(ctx, "to_mode", Some(TripMode::Bike), {
                        let mut choices = vec![Choice::new("cancel trip", None)];
                        for m in TripMode::all() {
                            choices.push(Choice::new(m.ongoing_verb(), Some(m)));
                        }
                        choices
                    }),
                ]),
                Widget::row(vec![
                    ctx.style()
                        .btn_solid_primary
                        .text("Apply")
                        .hotkey(Key::Enter)
                        .build_def(ctx),
                    ctx.style()
                        .btn_solid_destructive
                        .text("Discard changes")
                        .hotkey(Key::Escape)
                        .build_def(ctx),
                ])
                .centered(),
            ]))
            .exact_size_percent(80, 80)
            .build(ctx),
        };
        state.recalc_count(ctx, app);
        Box::new(state)
    }

    fn get_filters(&self, app: &App) -> (BTreeSet<TripMode>, (Time, Time)) {
        let to_mode = self.panel.dropdown_value::<Option<TripMode>, _>("to_mode");
        let (p1, p2) = (
            self.panel.slider("depart from").get_percent(),
            self.panel.slider("depart to").get_percent(),
        );
        let departure_filter = (
            app.primary.sim.get_end_of_day().percent_of(p1),
            app.primary.sim.get_end_of_day().percent_of(p2),
        );
        let mut from_modes = TripMode::all()
            .into_iter()
            .filter(|m| self.panel.is_checked(m.ongoing_verb()))
            .collect::<BTreeSet<_>>();
        if let Some(ref m) = to_mode {
            from_modes.remove(m);
        }
        (from_modes, departure_filter)
    }

    fn recalc_count(&mut self, ctx: &mut EventCtx, app: &App) {
        let (modes, (t1, t2)) = self.get_filters(app);
        let mut cnt = 0;
        for m in modes {
            cnt += self.count_trips.count(m, t1, t2);
        }
        let pct_ppl: usize = self.panel.spinner("pct_ppl");
        let mut txt = Text::from(format!("Matching trips: {}", prettyprint_usize(cnt)));
        let adjusted_cnt = ((cnt as f64) * (pct_ppl as f64) / 100.0) as usize;
        txt.append(
            Line(format!(
                " ({}% is {})",
                pct_ppl,
                prettyprint_usize(adjusted_cnt)
            ))
            .secondary(),
        );
        let label = txt.into_widget(ctx);
        self.panel.replace(ctx, "count", label);
    }
}

impl State<App> for ChangeMode {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "Discard changes" => Transition::Pop,
                "Apply" => {
                    let (from_modes, departure_filter) = self.get_filters(app);
                    let to_mode = self.panel.dropdown_value::<Option<TripMode>, _>("to_mode");
                    let pct_ppl = self.panel.spinner("pct_ppl");
                    if from_modes.is_empty() {
                        return Transition::Push(PopupMsg::new_state(
                            ctx,
                            "Error",
                            vec!["You have to select at least one mode to convert from"],
                        ));
                    }
                    if departure_filter.0 >= departure_filter.1 {
                        return Transition::Push(PopupMsg::new_state(
                            ctx,
                            "Error",
                            vec!["Your time range is backwards"],
                        ));
                    }

                    let mut mods = self.modifiers.clone();
                    mods.push(ScenarioModifier::ChangeMode {
                        to_mode,
                        pct_ppl,
                        departure_filter,
                        from_modes,
                    });
                    Transition::Multi(vec![
                        Transition::Pop,
                        Transition::Replace(EditScenarioModifiers::new_state(
                            ctx,
                            self.scenario_name.clone(),
                            mods,
                        )),
                    ])
                }
                _ => unreachable!(),
            },
            Outcome::Changed(_) => {
                self.recalc_count(ctx, app);
                Transition::Keep
            }
            _ => Transition::Keep,
        }
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        grey_out_map(g, app);
        self.panel.draw(g);
    }
}

pub struct DepartureSummary {
    first_trip: Time,
}

impl DepartureSummary {
    pub fn new_state(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        // Get all departure times, sorted so the sliding window works
        let mut departure_times: Vec<Time> = app
            .primary
            .scenario
            .as_ref()
            .unwrap()
            .people
            .iter()
            .flat_map(|person| person.trips.iter().map(|t| t.depart))
            .collect();
        departure_times.sort();
        let first_trip = departure_times
            .get(0)
            .cloned()
            .unwrap_or(Time::START_OF_DAY);

        let mut pts = vec![(Time::START_OF_DAY, 0)];
        let mut window = SlidingWindow::new(Duration::minutes(15));
        for time in departure_times {
            let count = window.add(time);
            pts.push((time, count));
        }
        window.close_off_pts(&mut pts, app.primary.sim.get_end_of_day());

        let panel = Panel::new_builder(Widget::col(vec![
            Widget::row(vec![
                Line("Trip departure times")
                    .small_heading()
                    .into_widget(ctx),
                ctx.style().btn_close_widget(ctx),
            ]),
            LinePlot::new_widget(
                ctx,
                "trip starts",
                vec![Series {
                    label: "When do trips start?".to_string(),
                    color: Color::RED,
                    pts,
                }],
                PlotOptions::fixed(),
                app.opts.units,
            )
            .section(ctx),
            if first_trip - app.primary.sim.time() > Duration::minutes(15) {
                ctx.style()
                    .btn_outline
                    .text(format!(
                        "Jump to first trip, at {}",
                        first_trip.ampm_tostring()
                    ))
                    .build_widget(ctx, "Jump to first trip")
            } else {
                Widget::nothing()
            },
            ctx.style()
                .btn_outline
                .text("Commuter patterns")
                .build_def(ctx),
        ]))
        .build(ctx);
        <dyn SimpleState<_>>::new_state(panel, Box::new(DepartureSummary { first_trip }))
    }
}

impl SimpleState<App> for DepartureSummary {
    fn on_click(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        x: &str,
        _: &mut Panel,
    ) -> Transition {
        match x {
            "close" => Transition::Pop,
            "Commuter patterns" => Transition::Replace(
                crate::sandbox::dashboards::CommuterPatterns::new_state(ctx, app),
            ),
            "Jump to first trip" => {
                Transition::Replace(TimeWarpScreen::new_state(ctx, app, self.first_trip, None))
            }
            _ => unreachable!(),
        }
    }
}

struct CountTrips {
    // Times are sorted
    departures_per_mode: BTreeMap<TripMode, Vec<Time>>,
}

impl CountTrips {
    fn new(app: &App) -> CountTrips {
        let mut departures_per_mode = BTreeMap::new();
        for m in TripMode::all() {
            departures_per_mode.insert(m, Vec::new());
        }
        for person in &app.primary.scenario.as_ref().unwrap().people {
            for trip in &person.trips {
                departures_per_mode
                    .get_mut(&trip.mode)
                    .unwrap()
                    .push(trip.depart);
            }
        }
        for list in departures_per_mode.values_mut() {
            list.sort();
        }
        CountTrips {
            departures_per_mode,
        }
    }

    fn count(&self, mode: TripMode, t1: Time, t2: Time) -> usize {
        // TODO Could binary search or even have a cursor to remember the position between queries.
        // Be careful with binary_search_by_key; it might miss multiple trips with the same time.
        let mut cnt = 0;
        for t in &self.departures_per_mode[&mode] {
            if *t >= t1 && *t <= t2 {
                cnt += 1;
            }
            if *t > t2 {
                break;
            }
        }
        cnt
    }
}
