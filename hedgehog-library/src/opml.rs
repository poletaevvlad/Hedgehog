use crate::NewFeedMetadata;
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

    #[error("The file is not a valid OPML")]
    NotOpmlFile,

    #[error(transparent)]
    Utf8(#[from] std::str::Utf8Error),
}

pub fn build_opml<W: io::Write, D: DataProvider>(write: W, data: &D) -> Result<(), Error> {
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
            let element = BytesStart::borrowed_name(b"outline").with_attributes(attrs);
            writer.write_event(Event::Empty(element))?;
        }
        writer.write_event(Event::End(BytesEnd::borrowed(b"body")))?;
    }

    writer.write_event(Event::End(BytesEnd::borrowed(b"opml")))?;
    Ok(())
}

pub fn parse_opml<R: io::BufRead>(reader: R) -> Result<OpmlEntries<R>, Error> {
    let mut reader = quick_xml::Reader::from_reader(reader);
    let mut buf = Vec::new();
    let mut depth: u64 = 0;
    loop {
        let event = reader.read_event(&mut buf)?;

        let (bytes_start, is_empty) = match event {
            Event::Start(start) => (start, false),
            Event::End(_) => {
                depth = depth.saturating_sub(1);
                continue;
            }
            Event::Empty(start) => (start, true),
            Event::Eof => return Err(Error::NotOpmlFile),
            _ => continue,
        };

        if depth == 0 && bytes_start.name() != b"opml" {
            return Err(Error::NotOpmlFile);
        } else if depth == 1 && bytes_start.name() == b"body" {
            return Ok(OpmlEntries {
                reader: if is_empty { None } else { Some(reader) },
                depth: 0,
                buf,
            });
        }

        if !is_empty {
            depth += 1;
        }
    }
}

pub fn import_opml<R: io::BufRead, D: DataProvider>(reader: R, data: &D) -> Result<(), Error> {
    let entries = parse_opml(reader)?;
    for entry in entries {
        data.create_feed_pending(&entry?)?;
    }
    Ok(())
}

pub struct OpmlEntries<R: io::BufRead> {
    reader: Option<quick_xml::Reader<R>>,
    depth: u64,
    buf: Vec<u8>,
}

impl<R: io::BufRead> Iterator for OpmlEntries<R> {
    type Item = Result<NewFeedMetadata, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let event = match self.reader {
                Some(ref mut reader) => reader.read_event(&mut self.buf),
                None => return None,
            };
            let bytes_start = match event {
                Ok(Event::Start(start)) => {
                    self.depth += 1;
                    start
                }
                Ok(Event::End(_)) => {
                    if self.depth == 0 {
                        return None;
                    }
                    self.depth -= 1;
                    continue;
                }
                Ok(Event::Empty(start)) => start,
                Ok(_) => continue,
                Err(err) => return Some(Err(err.into())),
            };

            if bytes_start.name() == b"outline" {
                let mut title = None;
                let mut html_feed = None;
                let mut xml_feed = None;
                let mut is_rss = false;
                for attr in bytes_start.attributes() {
                    let attr = match attr {
                        Ok(attr) => attr,
                        Err(err) => return Some(Err(err.into())),
                    };
                    let value = match attr.unescaped_value() {
                        Ok(value) => value,
                        Err(err) => return Some(Err(err.into())),
                    };
                    match attr.key {
                        b"type" => is_rss = (&*value) == (b"rss" as &[u8]),
                        b"xmlUrl" => {
                            xml_feed = match std::str::from_utf8(&value) {
                                Ok(xml_feed) => Some(xml_feed.to_string()),
                                Err(error) => return Some(Err(error.into())),
                            }
                        }
                        b"htmlUrl" => {
                            html_feed = match std::str::from_utf8(&value) {
                                Ok(html_feed) => Some(html_feed.to_string()),
                                Err(error) => return Some(Err(error.into())),
                            }
                        }
                        b"title" | b"text" => {
                            title = match std::str::from_utf8(&value) {
                                Ok(title) => Some(title.to_string()),
                                Err(error) => return Some(Err(error.into())),
                            }
                        }
                        _ => {}
                    }
                }

                if !is_rss {
                    continue;
                }
                if let Some(xml_feed) = xml_feed {
                    return Some(Ok(NewFeedMetadata::new(xml_feed)
                        .with_title(title)
                        .with_link(html_feed)));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::build_opml;
    use crate::datasource::{DataProvider, NewFeedMetadata};
    use crate::metadata::FeedMetadata;
    use crate::opml::parse_opml;
    use crate::SqliteDataProvider;
    use std::io::Cursor;

    #[test]
    fn test_build_opml_empty() {
        let data_provider = SqliteDataProvider::connect(":memory:").unwrap();

        let mut buffer = Vec::<u8>::new();
        build_opml(Cursor::new(&mut buffer), &data_provider).unwrap();

        let xml = String::from_utf8(buffer).unwrap();
        assert_eq!(&xml, include_str!("./test_data/opml/empty.opml").trim_end());
    }

    #[test]
    fn test_build_opml() {
        let mut data_provider = SqliteDataProvider::connect(":memory:").unwrap();
        data_provider
            .create_feed_pending(&NewFeedMetadata::new(
                "https://example.com/source_not_fetched".to_string(),
            ))
            .unwrap()
            .unwrap();

        let feed2_id = data_provider
            .create_feed_pending(&NewFeedMetadata::new(
                "https://example.com/source_2".to_string(),
            ))
            .unwrap()
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
            .create_feed_pending(&NewFeedMetadata::new(
                "https://example.com/source_3".to_string(),
            ))
            .unwrap()
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
        assert_eq!(
            &xml,
            include_str!("./test_data/opml/with-feeds.opml").trim_end()
        );
    }

    #[test]
    fn parse_opml_empty() {
        let reader = Cursor::new(include_str!("./test_data/opml/empty.opml"));
        let mut parser = parse_opml(reader).unwrap();
        assert!(parser.next().is_none());
    }

    #[test]
    fn parse_ompl_non_empty() {
        let reader = Cursor::new(include_str!("./test_data/opml/with-feeds.opml"));
        let mut parser = parse_opml(reader).unwrap();
        assert_eq!(
            parser.next().unwrap().unwrap(),
            NewFeedMetadata::new("https://example.com/source_not_fetched".to_string())
        );
        assert_eq!(
            parser.next().unwrap().unwrap(),
            NewFeedMetadata::new("https://example.com/source_2".to_string())
                .with_title("Feed title".to_string())
                .with_link("http://example.com/podcast2.html".to_string())
        );
        assert_eq!(
            parser.next().unwrap().unwrap(),
            NewFeedMetadata::new("https://example.com/source_3".to_string())
                .with_title("\"Second\" <fetched> podcast".to_string())
                .with_link("http://example.com/podcast3.html".to_string())
        );
    }

    #[test]
    fn parse_ompl_with_invalid_nodes() {
        let reader = Cursor::new(include_str!("./test_data/opml/with-invalid-nodes.opml"));
        let parser = parse_opml(reader).unwrap();
        let resource: Vec<String> = parser.map(|entry| entry.unwrap().source).collect();
        assert_eq!(
            resource,
            vec![
                "https://example.com/source_1".to_string(),
                "https://example.com/source_2".to_string(),
                "https://example.com/source_3".to_string()
            ]
        );
    }
}
