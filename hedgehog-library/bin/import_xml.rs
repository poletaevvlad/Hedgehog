use hedgehog_library::{
    datasource::{DataProvider, EpisodeWriter, WritableDataProvider},
    metadata::{EpisodeMetadata, FeedMetadata},
    SqliteDataProvider,
};
use rss::Channel;
use std::{env, fs::File, io::BufReader};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() != 2 {
        println!("Usage: <xml path> <feed url>");
        return;
    }

    let mut data_provider = SqliteDataProvider::connect_default_path().unwrap();
    let feed_id = data_provider.create_feed_pending(&args[1]).unwrap();
    let mut writer = data_provider.writer(feed_id).unwrap();

    let file = File::open(&args[0]).unwrap();
    let channel = Channel::read_from(BufReader::new(file)).unwrap();
    let channel_metadata = FeedMetadata::from_rss_channel(&channel);
    writer.set_feed_metadata(&channel_metadata).unwrap();

    for item in channel.items {
        if let Some(metadata) = EpisodeMetadata::from_rss_item(&item) {
            writer.set_episode_metadata(&metadata).unwrap();
        }
    }

    writer.close().unwrap();
}
