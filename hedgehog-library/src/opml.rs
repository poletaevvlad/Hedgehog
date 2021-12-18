use crate::{datasource::DataProvider, QueryError};
use quick_xml::escape::escape;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use std::io;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Xml(#[from] quick_xml::Error),

    #[error(transparent)]
    Db(#[from] QueryError),
}

fn build_opml<W: io::Write, D: DataProvider>(write: W, data: &D) -> Result<(), Error> {
    let mut writer = quick_xml::Writer::new_with_indent(write, b' ', 2);
    writer.write_event(Event::Decl(BytesDecl::new(b"1.0", Some(b"utf-8"), None)))?;
    writer.write_event(Event::Start(
        BytesStart::borrowed_name(b"opml")
            .with_attributes([(b"version" as &[u8], b"1.0" as &[u8])]),
    ))?;

    writer.write_event(Event::Start(BytesStart::borrowed_name(b"head")))?;
    writer.write_event(Event::Start(BytesStart::borrowed_name(b"title")))?;
    writer.write_event(Event::Text(BytesText::from_plain(b"Podcast Subscriptions")))?;
    writer.write_event(Event::End(BytesEnd::borrowed(b"title")))?;
    writer.write_event(Event::End(BytesEnd::borrowed(b"head")))?;

    let items = data.get_feed_opml_entries()?;
    if items.is_empty() {
        writer.write_event(Event::Empty(BytesStart::borrowed_name(b"body")))?;
    } else {
        writer.write_event(Event::Start(BytesStart::borrowed_name(b"body")))?;
        for item in items {
            let xml_url = escape(item.feed_source.as_bytes());
            let title = item.title.as_ref().map(|title| escape(title.as_bytes()));
            let link = item.link.as_ref().map(|link| escape(link.as_bytes()));

            let mut attrs: Vec<(&[u8], &[u8])> = vec![(b"type", b"rss"), (b"xmlUrl", &xml_url)];
            if let Some(link) = link.as_ref() {
                attrs.push((b"htmlUrl", link));
            }
            if let Some(title) = title.as_ref() {
                attrs.push((b"title", title));
                attrs.push((b"text", title));
            }
            let element = BytesStart::borrowed_name(b"ouline").with_attributes(attrs);
            writer.write_event(Event::Empty(element))?;
        }
        writer.write_event(Event::End(BytesEnd::borrowed(b"body")))?;
    }

    writer.write_event(Event::End(BytesEnd::borrowed(b"opml")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::build_opml;
    use crate::{
        datasource::{DataProvider, EpisodeWriter, WritableDataProvider},
        metadata::FeedMetadata,
        SqliteDataProvider,
    };
    use std::io::Cursor;

    #[test]
    fn test_build_opml_empty() {
        let data_provider = SqliteDataProvider::connect(":memory:").unwrap();

        let mut buffer = Vec::<u8>::new();
        build_opml(Cursor::new(&mut buffer), &data_provider).unwrap();

        let xml = String::from_utf8(buffer).unwrap();
        assert_eq!(&xml, include_str!("./test_data/empty.opml").trim_end());
    }

    #[test]
    fn test_build_opml() {
        let mut data_provider = SqliteDataProvider::connect(":memory:").unwrap();
        data_provider
            .create_feed_pending("https://example.com/source_not_fetched")
            .unwrap();

        let feed2_id = data_provider
            .create_feed_pending("https://example.com/source_2")
            .unwrap();
        let mut writer = data_provider.writer(feed2_id).unwrap();
        writer
            .set_feed_metadata(&FeedMetadata {
                title: "Feed title",
                link: "http://example.com/podcast2.html",
                description: "Podcast #2",
                author: None,
                copyright: None,
            })
            .unwrap();
        writer.close().unwrap();

        let feed3_id = data_provider
            .create_feed_pending("https://example.com/source_3")
            .unwrap();
        let mut writer = data_provider.writer(feed3_id).unwrap();
        writer
            .set_feed_metadata(&FeedMetadata {
                title: "\"Second\" <fetched> podcast",
                link: "http://example.com/podcast3.html",
                description: "Podcast #3",
                author: None,
                copyright: None,
            })
            .unwrap();
        writer.close().unwrap();

        let mut buffer = Vec::<u8>::new();
        build_opml(Cursor::new(&mut buffer), &data_provider).unwrap();

        let xml = String::from_utf8(buffer).unwrap();
        assert_eq!(&xml, include_str!("./test_data/with_feeds.opml").trim_end());
    }
}
