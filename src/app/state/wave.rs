use std::sync::Arc;

use flume::Sender;
use im::Vector;
use std::borrow::Cow;

use crate::{
    app::components::FuzzyItem, event::events::Event, framework::signals::Signal, http::ApiService,
};

#[derive(Clone, PartialEq, Debug)]
pub struct StationCategory {
    pub title: String,
    pub items: Vector<StationItem>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct StationItem {
    pub label: String,
    pub seed: String,
}

impl FuzzyItem for StationItem {
    fn label(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.label)
    }
}

pub struct WaveSessionState {
    pub waves: Signal<Vector<StationCategory>>,
    pub is_loading: Signal<bool>,
    api: Arc<ApiService>,
    event_tx: Sender<Event>,
}

impl WaveSessionState {
    pub fn new(api: Arc<ApiService>, event_tx: Sender<Event>) -> Self {
        let state = Self {
            waves: Signal::new(Vector::new()),
            is_loading: Signal::new(false),
            api,
            event_tx,
        };
        state.fetch();
        state
    }

    pub fn fetch(&self) {
        self.is_loading.set(true);
        let api = self.api.clone();
        let waves = self.waves.clone();
        let loading = self.is_loading.clone();
        tokio::spawn(async move {
            match api.fetch_stations().await {
                Ok(w) => {
                    let mut grouped: std::collections::HashMap<String, Vec<StationItem>> =
                        std::collections::HashMap::new();
                    for rotor in w {
                        let station = rotor.station;
                        if station.id.item_type.starts_with("mix") {
                            continue;
                        }

                        let seed = format!("{}:{}", station.id.item_type, station.id.tag);
                        let item = StationItem {
                            label: station.name.clone(),
                            seed,
                        };
                        grouped
                            .entry(station.id.item_type.clone())
                            .or_default()
                            .push(item);
                    }

                    let mut cats = Vec::new();
                    for (k, mut v) in grouped {
                        let mut chars = k.chars();
                        let title = match chars.next() {
                            None => String::new(),
                            Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
                        };
                        v.sort_by_key(|i| i.label.clone());
                        cats.push(StationCategory {
                            title,
                            items: Vector::from(v),
                        });
                    }
                    cats.sort_by_key(|c| c.title.clone());

                    waves.set(Vector::from(cats));
                    loading.set(false);
                }
                Err(e) => {
                    tracing::error!("Failed to fetch stations: {}", e);
                    loading.set(false);
                }
            }
        });
    }

    pub fn start_with_seeds(&self, seeds: Vec<String>) {
        self.is_loading.set(true);
        let api = self.api.clone();
        let tx = self.event_tx.clone();
        let loading = self.is_loading.clone();
        tokio::spawn(async move {
            match api.create_session(seeds).await {
                Ok(session) => {
                    let tracks = session.sequence.iter().map(|s| s.track.clone()).collect();
                    let _ = tx.send(Event::WaveReady(session, tracks));
                }
                Err(e) => {
                    let _ = tx.send(Event::FetchError(e.to_string()));
                }
            }
            loading.set(false);
        });
    }
}
