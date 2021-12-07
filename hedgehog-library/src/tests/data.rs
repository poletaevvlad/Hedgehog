use crate::model::EpisodeSummary;
use chrono::{TimeZone, Utc};
use std::time::Duration;

pub(super) struct EpisodeSummaryFixture<'a> {
    episode_number: Option<i64>,
    season_number: Option<i64>,
    title: Option<&'a str>,
    feed_title: Option<&'a str>,
    duration: Option<Duration>,
    publication_date: Option<(i32, u32, u32, u32, u32, u32)>,
}

impl<'a> EpisodeSummaryFixture<'a> {
    pub(super) fn assert_equals(&self, summary: &EpisodeSummary) {
        assert_eq!(self.episode_number, summary.episode_number);
        assert_eq!(self.season_number, summary.season_number);
        assert_eq!(self.title, summary.title.as_deref());
        assert_eq!(self.feed_title, summary.feed_title.as_deref());
        assert_eq!(self.duration, summary.duration);
        assert_eq!(
            self.publication_date
                .map(|(y, m, d, h, min, s)| Utc.ymd(y, m, d).and_hms(h, min, s),),
            summary.publication_date
        );
    }
}

pub(super) mod feed1 {
    use super::*;

    pub(in super::super) const EPISODE_1: EpisodeSummaryFixture = EpisodeSummaryFixture {
        episode_number: None,
        season_number: None,
        title: Some("Episode #1"),
        feed_title: Some("Sample Podcast"),
        duration: None,
        publication_date: Some((2021, 12, 18, 12, 0, 0)),
    };
    pub(in super::super) const EPISODE_2: EpisodeSummaryFixture = EpisodeSummaryFixture {
        episode_number: Some(2),
        season_number: Some(1),
        title: Some("Episode #2"),
        feed_title: Some("Sample Podcast"),
        duration: Some(Duration::from_secs(180)),
        publication_date: Some((2021, 12, 19, 12, 0, 0)),
    };
    pub(in super::super) const EPISODE_3: EpisodeSummaryFixture = EpisodeSummaryFixture {
        episode_number: Some(3),
        season_number: None,
        title: Some("Episode #3"),
        feed_title: Some("Sample Podcast"),
        duration: Some(Duration::from_secs(170)),
        publication_date: Some((2021, 12, 20, 12, 0, 0)),
    };
    pub(in super::super) const EPISODE_3_UPDATED: EpisodeSummaryFixture = EpisodeSummaryFixture {
        episode_number: Some(3),
        season_number: None,
        title: Some("Episode #3 (updated)"),
        feed_title: Some("Sample Podcast"),
        duration: Some(Duration::from_secs(170)),
        publication_date: Some((2021, 12, 20, 12, 0, 0)),
    };
    pub(in super::super) const EPISODE_4: EpisodeSummaryFixture = EpisodeSummaryFixture {
        episode_number: None,
        season_number: Some(1),
        title: Some("Episode #4"),
        feed_title: Some("Sample Podcast"),
        duration: Some(Duration::from_secs(160)),
        publication_date: Some((2021, 12, 21, 12, 0, 0)),
    };
    pub(in super::super) const EPISODE_5: EpisodeSummaryFixture = EpisodeSummaryFixture {
        episode_number: None,
        season_number: None,
        title: Some("Episode #5"),
        feed_title: Some("Sample Podcast"),
        duration: Some(Duration::from_secs(150)),
        publication_date: Some((2021, 12, 22, 12, 0, 0)),
    };
    pub(in super::super) const EPISODE_6: EpisodeSummaryFixture = EpisodeSummaryFixture {
        episode_number: None,
        season_number: None,
        title: Some("Episode #6"),
        feed_title: Some("Sample Podcast"),
        duration: Some(Duration::from_secs(140)),
        publication_date: Some((2021, 12, 23, 12, 0, 0)),
    };
}
