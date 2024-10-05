use ascii::AsciiString;
use tiny_http::{Header, HeaderField};
use translator::translate;

mod translator;

#[tokio::main]
async fn main() {
    let listen = std::env::var("LISTENING_ADDRESS").expect("Set env variable LISTENING_ADDRESS to an IP and port combination, for example: 0.0.0.0:8080");
    let base_url = std::env::var("BASE_URL").expect("Set env variable BASE_URL to the base url");

    let server = tiny_http::Server::http(listen).unwrap();

    loop {
        let request = match server.recv() {
            Ok(rq) => rq,
            Err(e) => {
                println!("http server error: {}", e);
                break;
            }
        };

        let url = request.url();
        let parts = url.split("/").collect::<Vec<&str>>();

        // Remove first element, which is an empty string
        let parts = &parts[1..];

        println!("Request: {:?}", parts);

        if parts.len() < 2 {
            request
                .respond(tiny_http::Response::from_string("Invalid URL"))
                .unwrap();
            continue;
        }

        match parts[0] {
            "channel" => {
                let channel_id = parts[1];

                let videos = reqwest::get(format!(
                    "https://www.youtube.com/feeds/videos.xml?channel_id={channel_id}"
                ))
                .await;

                let videos = match videos {
                    Ok(v) => v,
                    Err(e) => {
                        request
                            .respond(tiny_http::Response::from_string(format!("Error: {}", e)))
                            .unwrap();
                        continue;
                    }
                };

                let videos = videos.text().await.unwrap();

                let response = match translate(videos, &base_url) {
                    Ok(r) => r,
                    Err(e) => format!("Error: {}", e),
                };

                request
                    .respond(
                        tiny_http::Response::from_string(response).with_header(Header {
                            field: HeaderField::from_bytes("Content-Type".to_string().as_bytes())
                                .unwrap(),
                            value: AsciiString::from_ascii("application/rss+xml".to_string())
                                .unwrap(),
                        }),
                    )
                    .unwrap();
            }
            _ => {
                request
                    .respond(tiny_http::Response::from_string("Invalid URL"))
                    .unwrap();
            }
        }
    }
}
