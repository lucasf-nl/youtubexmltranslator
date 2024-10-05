use std::io::{BufReader, BufWriter};

use xml::{
    common::XmlVersion, reader::XmlEvent, writer::XmlEvent as WXmlEvent, EmitterConfig,
    ParserConfig,
};

enum ParserState {
    // This is in chronological order of the parsing process
    None,
    Header,
    ChannelInfo,
    VideoEntries,
    VideoEntry,
}

pub fn translate(input: String, base_url: &str) -> Result<String, Box<dyn std::error::Error>> {
    let data = BufReader::new(input.as_bytes());
    let mut reader = ParserConfig::default()
        .ignore_root_level_whitespace(true)
        .ignore_comments(true)
        .cdata_to_characters(true)
        .coalesce_characters(true)
        .create_reader(data);

    let output = Vec::new();
    let bufwriter = BufWriter::new(output);
    let writer = EmitterConfig::default();
    let mut writer = writer.create_writer(bufwriter);

    let mut state = ParserState::None;

    loop {
        let reader_event = reader.next()?;

        match reader_event {
            XmlEvent::EndDocument => break,
            XmlEvent::StartDocument {
                version,
                encoding,
                standalone: _,
            } => {
                if version != XmlVersion::Version10 {
                    eprintln!("Attempting to parse XML file with untested version: {version}");
                }

                assert_eq!(encoding, "UTF-8".to_string());

                state = ParserState::Header;
            }
            XmlEvent::StartElement {
                name,
                namespace: _,
                attributes,
            } => {
                match state {
                    ParserState::None => {
                        return Err("Unexpected start element during None state".into());
                    }
                    ParserState::Header => {
                        assert_eq!(name.local_name, "feed".to_string());

                        // Write the entire header neccessary for a RSS stream
                        writer.write(WXmlEvent::StartDocument {
                            version: XmlVersion::Version10,
                            encoding: Some("UTF-8"),
                            standalone: Some(true),
                        })?;

                        writer.write("\n")?;

                        writer.write(
                            WXmlEvent::start_element("rss")
                                .attr("version", "2.0")
                                .attr("xmlns:atom", "http://www.w3.org/2005/Atom"),
                        )?;

                        writer.write("\n")?;

                        state = ParserState::ChannelInfo;
                    }
                    ParserState::ChannelInfo => match name.local_name.as_str() {
                        "link" => {
                            // Check if rel is self or alternate, self is the XML url
                            // alternate is the actual channel page
                            let rel = attributes
                                .iter()
                                .find(|attr| attr.name.local_name == "rel")
                                .unwrap()
                                .value
                                .clone();

                            let href = attributes
                                .iter()
                                .find(|attr| attr.name.local_name == "href")
                                .unwrap()
                                .value
                                .clone();

                            if rel == "self" {
                                // Start of the channel
                                writer.write(WXmlEvent::start_element("channel"))?;
                                writer.write("\n")?;
                                writer.write(
                                    WXmlEvent::start_element("atom:link")
                                        .attr(
                                            "href",
                                            &format!(
                                                "{base_url}/channel/{}",
                                                href.split("=").last().unwrap()
                                            ),
                                        )
                                        .attr("rel", "self")
                                        .attr("type", "application/rss+xml"),
                                )?;
                                writer.write(WXmlEvent::end_element())?;
                                writer.write("\n")?;
                            }

                            if rel == "alternate" {
                                // Primary link to the channel
                                writer.write(WXmlEvent::start_element("link"))?;
                                writer.write(WXmlEvent::characters(&href))?;
                                writer.write(WXmlEvent::end_element())?;
                                writer.write("\n")?;
                            }
                        }

                        "title" => {
                            let characters = reader.next()?;
                            writer.write(WXmlEvent::start_element("title"))?;
                            writer.write(characters.as_writer_event().unwrap())?;
                            writer.write(WXmlEvent::end_element())?;
                            writer.write("\n")?;

                            writer.write(WXmlEvent::start_element("description"))?;
                            writer.write(characters.as_writer_event().unwrap())?;
                            writer.write(WXmlEvent::end_element())?;
                            writer.write("\n")?;
                        }

                        "published" => {
                            // Grab to ignore
                            let characters = reader.next()?;
                            let _ = match characters {
                                XmlEvent::Characters(s) => s,
                                _ => panic!("Unexpected event type"),
                            };

                            // current date
                            let date = chrono::Utc::now();
                            let date = date.format("%a, %d %b %Y %H:%M:%S %z").to_string();

                            writer.write(WXmlEvent::start_element("lastBuildDate"))?;
                            writer.write(WXmlEvent::characters(&date))?;
                            writer.write(WXmlEvent::end_element())?;
                            writer.write("\n")?;

                            // This is the end of the channel info section, now come the video entries
                            state = ParserState::VideoEntries;
                        }

                        // Explicitly ignored, these do not have a place in the XML file
                        "channelId" => {}
                        "id" => {}
                        "author" => {}

                        // Part of author
                        "name" => {}
                        "uri" => {}

                        _ => {
                            println!(
                                "Received unknown channelinfo attribute: {}",
                                name.local_name
                            );
                        }
                    },
                    ParserState::VideoEntries => {
                        if name.local_name == "entry" {
                            // Video entry started
                            writer.write(WXmlEvent::start_element("item"))?;
                            writer.write("\n")?;
                            state = ParserState::VideoEntry;
                        }
                    }
                    ParserState::VideoEntry => match name.local_name.as_str() {
                        "id" => {}

                        "title" => {
                            // media:group also has a media:title, which should be ignored
                            if name.prefix != Some("media".to_string()) {
                                let characters = reader.next()?;
                                writer.write(WXmlEvent::start_element("title"))?;
                                writer.write(characters.as_writer_event().unwrap())?;
                                writer.write(WXmlEvent::end_element())?;
                                writer.write("\n")?;
                            }
                        }

                        "link" => {
                            let href = attributes
                                .iter()
                                .find(|attr| attr.name.local_name == "href")
                                .unwrap()
                                .value
                                .clone();

                            writer.write(WXmlEvent::start_element("link"))?;
                            writer.write(WXmlEvent::characters(&href))?;
                            writer.write(WXmlEvent::end_element())?;
                            writer.write("\n")?;
                        }

                        "published" => {
                            let characters = reader.next()?;
                            let characters = match characters {
                                XmlEvent::Characters(s) => s,
                                _ => panic!("Unexpected event type"),
                            };

                            // Convert to RFC-822 format
                            let date = chrono::DateTime::parse_from_rfc3339(&characters).unwrap();
                            let date = date.format("%a, %d %b %Y %H:%M:%S %z").to_string();

                            writer.write(WXmlEvent::start_element("pubDate"))?;
                            writer.write(WXmlEvent::characters(&date))?;
                            writer.write(WXmlEvent::end_element())?;
                            writer.write("\n")?;
                        }

                        "content" => {
                            // Skip content end and a whitespace
                            reader.next()?;
                            reader.next()?;

                            // Thumbnail has to go first for some mobile apps
                            let thumbnail = reader.next()?;
                            let mut thumbnail_url = "".to_string();
                            let mut thumbnail_width = 0;
                            let mut thumbnail_height = 0;

                            // Get url, width and height attributes
                            match thumbnail {
                                XmlEvent::StartElement {
                                    name,
                                    attributes,
                                    namespace: _,
                                } => {
                                    if name.local_name == "content" {
                                        for attr in attributes {
                                            if attr.name.local_name == "url" {
                                                thumbnail_url = attr.value.clone();
                                            }
                                            if attr.name.local_name == "width" {
                                                thumbnail_width = attr.value.parse().unwrap();
                                            }
                                            if attr.name.local_name == "height" {
                                                thumbnail_height = attr.value.parse().unwrap();
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }

                            // Skip thumbnail end tag, a whitespace and description header
                            reader.next()?;
                            reader.next()?;
                            reader.next()?;
                            // Get the description characters
                            let description = reader.next()?;
                            let description = match description {
                                XmlEvent::Characters(characters) => characters,
                                _ => "".to_string(),
                            };

                            // Get the content url
                            let url = attributes
                                .iter()
                                .find(|attr| attr.name.local_name == "url")
                                .unwrap()
                                .value
                                .clone();

                            // Get the id
                            let video_id =
                                url.split("/").last().unwrap().split("?").next().unwrap();

                            // Add the URL as guid
                            writer.write(WXmlEvent::start_element("guid"))?;
                            writer.write(WXmlEvent::characters(&url.split("?").next().unwrap()))?;
                            writer.write(WXmlEvent::end_element())?;
                            writer.write("\n")?;

                            // Write the HTML post body
                            writer.write(WXmlEvent::start_element("description"))?;
                            writer.write(WXmlEvent::Characters(format!(r#"
                            <img src="{thumbnail_url}" alt="YouTube thumbnail" class="" loading="lazy" width="{thumbnail_width}" height="{thumbnail_height}" />
                            <p>{description}</p>
                            <iframe
                                allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; web-share"
                                allowfullscreen="allowfullscreen"
                                loading="eager"
                                referrerpolicy="strict-origin-when-cross-origin"
                                src="https://www.youtube.com/embed/{video_id}?autoplay=0&controls=1&end=0&loop=0&mute=0&start=0"
                                style="position: absolute; top: 0; left: 0; width: 100%; height: 100%; border:0;"
                                title="YouTube video">
                            </iframe>
                            "#).as_str()))?;
                            writer.write(WXmlEvent::end_element())?;
                            writer.write("\n")?;

                            // End the item
                            writer.write(WXmlEvent::end_element())?;

                            // Return state to video entries
                            state = ParserState::VideoEntries;
                        }

                        // Explicitly ignored, these do not have a place in the RSS feed
                        "updated" => {}
                        "videoId" => {}
                        "channelId" => {}
                        "author" => {}
                        // media:group, some data is important but the header is not
                        "group" => {}

                        // Part of author
                        "name" => {}
                        "uri" => {}

                        // Part of media
                        "community" => {}
                        "starRating" => {}
                        "statistics" => {}

                        _ => {
                            println!(
                                "Received unknown video entry attribute: {}",
                                name.local_name
                            );
                        }
                    },
                }
            }
            _ => {
                // Ignore other events
            }
        }
    }

    // End remaining elements
    writer.write(WXmlEvent::end_element())?;
    writer.write(WXmlEvent::end_element())?;

    // Return the result as a string
    Ok(writer
        .into_inner()
        .into_inner()
        .map(|bytes| String::from_utf8(bytes).unwrap())
        .unwrap())
}
