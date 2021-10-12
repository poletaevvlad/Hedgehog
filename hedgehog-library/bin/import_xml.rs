use hedgehog_library::datasource::{EpisodesDao, FeedsDao, SqliteDataProvider};
use hedgehog_library::metadata::{EpisodeMetadata, FeedMetadata};
use hedgehog_library::rss::Channel;
use std::convert::TryFrom;
use std::env;
use std::fs::File;
use std::io::BufReader;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() != 2 {
        println!("Usage: <xml path> <feed url>");
        return;
    }

    let mut data_provider = SqliteDataProvider::connect_default_path().unwrap();
    let transaction = data_provider.transaction().unwrap();

    let feed_id = transaction.feeds().create_pending(&args[1]).unwrap();
    let file = File::open(&args[0]).unwrap();
    let channel = Channel::read_from(BufReader::new(file)).unwrap();
    let channel_metadata = FeedMetadata::from(channel.clone());
    transaction
        .feeds()
        .update_metadata(feed_id, &channel_metadata)
        .unwrap();

    for item in channel.items {
        let metadata = EpisodeMetadata::try_from(item).unwrap();
        transaction
            .episodes()
            .sync_metadata(feed_id, &metadata)
            .unwrap();
    }

    transaction.commit().unwrap();
}
