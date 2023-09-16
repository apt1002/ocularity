use std::collections::{HashMap};
use std::error::{Error};
use std::fs::{File};
use std::path::{Path};
use std::str::{Split};

use tiny_http::{Method, Request, Response, Header};
use url::{Url};

// ----------------------------------------------------------------------------

/// A "200 OK" HTTP response.
#[derive(Debug)]
pub enum HttpOkay {
    File(File),
    Text(String),
    Data(Vec<u8>),
}

// An erroneous HTTP response.
#[derive(Debug)]
pub enum HttpError {
    Invalid,
    NotFound,
    Error(Box<dyn Error>),
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for HttpError {}

macro_rules! impl_from_for_error {
    ($e:ty) => {
        impl From<$e> for HttpError {
            fn from(e: $e) -> Self { HttpError::Error(e.into()) }
        }
    };
}

impl_from_for_error!(std::io::Error);
impl_from_for_error!(std::num::ParseIntError);
impl_from_for_error!(url::ParseError);
impl_from_for_error!(png::EncodingError);

fn header(key: &str, value: &str) -> tiny_http::Header {
    let key_b = key.as_bytes();
    let val_b = value.as_bytes();
    Header::from_bytes(
        key_b, val_b)
        .unwrap() // depends only on data fixed at compile time
}

// ----------------------------------------------------------------------------

fn main() -> Result<(), Box<dyn Error>> {
    let server = tiny_http::Server::http("127.0.0.1:8081").unwrap();
    for request in server.incoming_requests() {
        match handle_request(&request) {
            Ok(HttpOkay::File(file)) => {
                request.respond(Response::from_file(file))
            },
            Ok(HttpOkay::Text(text)) => {
                request.respond(Response::from_string(text))
            },
            Ok(HttpOkay::Data(data)) => {
                let header = header("Content-Type", "image/png");
                request.respond(Response::from_data(data).with_header(header))
            },
            Err(HttpError::Invalid) => {
                request.respond(Response::from_string("Invalid request").with_status_code(400))
            },
            Err(HttpError::NotFound) => {
                request.respond(Response::from_string("Not found").with_status_code(404))
            },
            Err(e) => {
                println!("Error: {}", e);
                request.respond(Response::from_string("Internal error").with_status_code(500))
            },
        }.unwrap_or_else(|e2| println!("IO Error: {}", e2));
    }
    Ok(())
}

const BASE_URL: &'static str = "https://www.minworks.co.uk";

fn handle_request(request: &Request) -> Result<HttpOkay, HttpError> {
    match request.method() {
        Method::Get => {},
        _ => return Err(HttpError::Invalid),
    }

    let url = request.url();
    let url = url_escape::decode(url).into_owned();
    let url = Url::parse(BASE_URL).unwrap().join(&url)?;
    println!("{:?}", url);
    let params: HashMap<String, String> = url.query_pairs().map(
        |(key, value)| (key.into_owned(), value.into_owned())
    ).collect();
    println!("{:?}", params);
    let mut path = url.path_segments().unwrap();
    match path.next() {
        Some("hello") => Ok(HttpOkay::Text("Hello, Martin!".to_owned())),
        Some("static") => static_file(path, params),
        Some("image.png") => image(path, params),
        _ => Err(HttpError::NotFound),
    }
    
}

// ----------------------------------------------------------------------------

fn static_file(mut path: Split<char>, _params: HashMap<String, String>) -> Result<HttpOkay, HttpError> {
    if let Some(name) = path.next() {
        if name != ".." {
            return Ok(HttpOkay::File(File::open(&Path::new(name))?));
        }
    }
    Err(HttpError::Invalid)
}

// ----------------------------------------------------------------------------

fn image(_path: Split<char>, params: HashMap<String, String>) -> Result<HttpOkay, HttpError> {
    let r = params.get("r").ok_or(HttpError::Invalid)?.parse::<u8>()?;
    let g = params.get("g").ok_or(HttpError::Invalid)?.parse::<u8>()?;
    let b = params.get("b").ok_or(HttpError::Invalid)?.parse::<u8>()?;
    let mut buf: Vec<u8> = Vec::new();
    let mut encoder = png::Encoder::new(&mut buf, 1, 1);
    encoder.set_color(png::ColorType::Rgb);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&[r, g, b])?;
    writer.finish()?;
    Ok(HttpOkay::Data(buf))
}
