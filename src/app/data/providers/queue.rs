use std::ops::Range;

use im::Vector;
use yandex_music::model::track::Track;

use super::super::{DataSource, FetchState};
use crate::framework::reactive::{Update, With, create_effect};
use crate::framework::signals::Signal;

pub struct QueueDataSource {
    queue: Signal<Vector<Track>>,
    changed: Signal<u64>,
}

impl QueueDataSource {
    pub fn new(queue: Signal<Vector<Track>>) -> Self {
        let changed: Signal<u64> = Signal::new(0);

        create_effect({
            let queue = queue.clone();
            let changed = changed.clone();
            move |_| {
                let _len = With::with(&queue, |q| q.len());
                Update::update(&changed, |v| *v += 1);
            }
        });

        Self { queue, changed }
    }
}

impl DataSource<Track> for QueueDataSource {
    fn total(&self) -> Option<usize> {
        Some(self.queue.with(|q| q.len()))
    }

    fn range(&self, range: Range<usize>) -> Vector<Track> {
        self.queue.with(|queue| {
            let start = range.start.min(queue.len());
            let end = range.end.min(queue.len());
            queue.clone().slice(start..end)
        })
    }

    fn is_loaded(&self, _range: Range<usize>) -> bool {
        true
    }

    fn request_range(&self, _range: Range<usize>) {}

    fn fetch_state(&self) -> FetchState {
        FetchState::Loaded
    }

    fn changed_signal(&self) -> Signal<u64> {
        self.changed.clone()
    }

    fn refresh(&self) {}
}
