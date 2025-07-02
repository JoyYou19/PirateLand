use warp::Filter;
use reqwest::Client;
use warp::http::StatusCode;
use warp::hyper::Body;
use std::convert::Infallible;

pub async fn start_proxy() {
    let client = Client::new();

    let image_proxy = warp::path("proxy")
        .and(warp::query::<std::collections::HashMap<String, String>>())
        .and_then(move |params: std::collections::HashMap<String, String>| {
            let client = client.clone();
            async move {
                if let Some(image_url) = params.get("url") {
                    //println!("Received request to proxy image: {}", image_url);

                    // Fetch the image from the URL
                    let res = client
                        .get(image_url)
                        .header(
                            "User-Agent",
                            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
                        )
                        .header("Referer", "https://online-fix.me")
                        .send()
                    .await;

                    match res {
                        Ok(response) => {
                            //println!("Successfully fetched image from: {}", image_url);

                            // Clone the headers before consuming the response
                            let headers_map = response.headers().clone();
                            //println!("Response headers: {:?}", headers_map);

                            // Consume the response to get the body
                            let body = match response.bytes().await {
                                Ok(bytes) => {
                                    //println!("Successfully read response body");
                                    bytes
                                }
                                Err(err) => {
                                    //println!("Failed to read response body: {}", err);
                                    return Ok::<_, Infallible>(warp::http::Response::builder()
                                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                                        .body(Body::from("Failed to read image data"))
                                        .unwrap());
                                }
                            };

                            // Create a Warp HeaderMap and populate it
                            let mut headers = warp::http::HeaderMap::new();

                            if let Some(content_type) = headers_map.get(reqwest::header::CONTENT_TYPE) {
                                headers.insert(
                                    warp::http::header::CONTENT_TYPE,
                                    content_type.to_str().unwrap_or("application/octet-stream").parse().unwrap(),
                                );
                            }
                            // Add CORS headers
                            headers.insert("Access-Control-Allow-Origin", "*".parse().unwrap());
                            headers.insert("Access-Control-Allow-Methods", "GET, OPTIONS".parse().unwrap());
                            headers.insert("Access-Control-Allow-Headers", "*".parse().unwrap());

                            // Construct the response
                            let mut builder = warp::http::Response::builder()
                                .status(StatusCode::OK);

                            for (key, value) in headers.iter() {
                                builder = builder.header(key, value);
                            }

                            let response = builder.body(Body::from(body)).unwrap();
                            Ok::<_, Infallible>(warp::reply::Response::from(response))
                        }
                        Err(err) => {
                            //println!("Failed to fetch image from: {}. Error: {}", image_url, err);

                            let error_response = warp::http::Response::builder()
                                .status(StatusCode::BAD_GATEWAY)
                                .body(Body::from("Failed to fetch image"))
                                .unwrap();
                            Ok::<_, Infallible>(warp::reply::Response::from(error_response))
                        }
                    }
                } else {
                    //println!("Missing 'url' parameter in request");
                    let error_response = warp::http::Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .body(Body::from("Missing 'url' parameter"))
                        .unwrap();
                    Ok::<_, Infallible>(warp::reply::Response::from(error_response))
                }
            }
        });

    //println!("Starting proxy server on http://127.0.0.1:3030");
    warp::serve(image_proxy).run(([127, 0, 0, 1], 3030)).await;
}
