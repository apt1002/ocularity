use std::collections::{HashMap};
use std::error::{Error};
use std::fs::{File};
use std::str::{Split};

use tiny_http::{Method, Request, Response, Header};
use url::{Url};

// ----------------------------------------------------------------------------

/// A "200 OK" HTTP response.
#[derive(Debug)]
pub enum HttpOkay {
    File(File),
    Text(String),
    Html(String),
    Data(Vec<u8>),
    Static(&'static [u8], &'static str),
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
impl_from_for_error!(png::DecodingError);

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
    println!("Listening on http://{}", server.server_addr());
    for request in server.incoming_requests() {
        match handle_request(&request) {
            Ok(HttpOkay::File(file)) => {
                request.respond(Response::from_file(file))
            },
            Ok(HttpOkay::Text(text)) => {
                request.respond(Response::from_string(text))
            },
            Ok(HttpOkay::Html(text)) => {
                let header = header("Content-Type", "text/html");
                request.respond(Response::from_string(text).with_header(header))
            },
            Ok(HttpOkay::Data(data)) => {
                let header = header("Content-Type", "image/png");
                request.respond(Response::from_data(data).with_header(header))
            },
            Ok(HttpOkay::Static(data, content_type)) => {
                let header = header("Content-Type", content_type);
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
        Some("static") => static_file(path, params),
        Some("question") => question(path, params),
        Some("image.png") => image(path, params),
        _ => Err(HttpError::NotFound),
    }
}

// ----------------------------------------------------------------------------

const STYLESHEET: &[u8] = include_bytes!("stylesheet.css");
const QUESTION: &[u8] = include_bytes!("question.html");

fn static_file(mut path: Split<char>, _params: HashMap<String, String>) -> Result<HttpOkay, HttpError> {
    match path.next() {
        Some("stylesheet.css") => Ok(HttpOkay::Static(STYLESHEET, "text/css")),
        Some("question.html") => Ok(HttpOkay::Static(QUESTION, "text/html")),
        _ => Err(HttpError::Invalid),
    }
}

// ----------------------------------------------------------------------------

/// The test pattern (black-and-white version).
const TEST_PATTERN: &[u8] = include_bytes!("test-pattern-grey.png");

fn image(_path: Split<char>, params: HashMap<String, String>) -> Result<HttpOkay, HttpError> {
    let r1 = params.get("r1").ok_or(HttpError::Invalid)?.parse::<u8>()? as f32;
    let g1 = params.get("g1").ok_or(HttpError::Invalid)?.parse::<u8>()? as f32;
    let b1 = params.get("b1").ok_or(HttpError::Invalid)?.parse::<u8>()? as f32;
    let r2 = params.get("r2").ok_or(HttpError::Invalid)?.parse::<u8>()? as f32;
    let g2 = params.get("g2").ok_or(HttpError::Invalid)?.parse::<u8>()? as f32;
    let b2 = params.get("b2").ok_or(HttpError::Invalid)?.parse::<u8>()? as f32;

    // Construct the palette.
    let mut palette = Vec::new();
    for i in 0..256 {
        let f = (i as f32) / 255.0;
        palette.push((r1 + f * (r2 - r1)) as u8);
        palette.push((g1 + f * (g2 - g1)) as u8);
        palette.push((b1 + f * (b2 - b1)) as u8);
    }

    // Read the input image.
    let decoder = png::Decoder::new(TEST_PATTERN);
    let mut reader = decoder.read_info()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let input_info = reader.next_frame(&mut buf).unwrap();
    assert_eq!(input_info.color_type, png::ColorType::Grayscale);
    let pixel_data = &buf[..input_info.buffer_size()];

    // Generate the output image.
    let mut output_bytes: Vec<u8> = Vec::new();
    let mut encoder = png::Encoder::new(&mut output_bytes, input_info.width, input_info.height);
    encoder.set_color(png::ColorType::Indexed);
    encoder.set_palette(palette);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(pixel_data)?;
    writer.finish()?;

    Ok(HttpOkay::Data(output_bytes))
}

// ----------------------------------------------------------------------------

#[derive(Debug, Copy, Clone)]
struct Delta(i8, i8, i8);

const DELTAS: [Delta; 6] = [
    Delta(10, 10, 0), Delta(10, -10, 0),
    Delta(10, 0, 10), Delta(10, 0, -10),
    Delta(0, 10, 10), Delta(0, 10, -10),
];

impl std::ops::Neg for Delta {
    type Output = Self;

    fn neg(self) -> Self::Output { Self(-self.0, -self.1, -self.2) }
}

/// Return a random element of `DELTAS`.
fn random_delta() -> Delta {
    let delta = DELTAS[rand::random_range(0..DELTAS.len())];
    if rand::random() { delta } else { -delta }
}

// ----------------------------------------------------------------------------

#[derive(Debug, Copy, Clone)]
struct Centre(u8, u8, u8);

const CENTRES: [Centre; 35] = [
    Centre( 25,  25,  25), Centre(127,  25,  25), Centre(229,  25,  25),
    Centre( 25, 127,  25), Centre(127, 127,  25), Centre(229, 127,  25),
    Centre( 25, 229,  25), Centre(127, 229,  25), Centre(229, 229,  25),

    Centre( 25,  25, 127), Centre(127,  25, 127), Centre(229,  25, 127),
    Centre( 25, 127, 127), Centre(127, 127, 127), Centre(229, 127, 127),
    Centre( 25, 229, 127), Centre(127, 229, 127), Centre(229, 229, 127),

    Centre( 25,  25, 229), Centre(127,  25, 229), Centre(229,  25, 229),
    Centre( 25, 127, 229), Centre(127, 127, 229), Centre(229, 127, 229),
    Centre( 25, 229, 229), Centre(127, 229, 229), Centre(229, 229, 229),

    Centre( 76,  76,  76), Centre(178,  76,  76),
    Centre( 76, 178,  76), Centre(178, 178,  76),

    Centre( 76,  76, 178), Centre(178,  76, 178),
    Centre( 76, 178, 178), Centre(178, 178, 178),
];

/// Return a random element of `CENTRES`.
fn random_centre() -> Centre { CENTRES[rand::random_range(0..CENTRES.len())] }

impl std::ops::Add<Delta> for Centre {
    type Output = (u8, u8, u8);

    fn add(self, rhs: Delta) -> Self::Output {
        (
            ((self.0 as i32) + (rhs.0 as i32)) as u8,
            ((self.1 as i32) + (rhs.1 as i32)) as u8,
            ((self.2 as i32) + (rhs.2 as i32)) as u8,
        )
    }
}

impl std::ops::Sub<Delta> for Centre {
    type Output = (u8, u8, u8);

    fn sub(self, rhs: Delta) -> Self::Output { self + -rhs }
}

// ----------------------------------------------------------------------------

fn image_element(centre: Centre, delta: Delta) -> String {
    let c1 = centre + delta;
    let c2 = centre - delta;
    format!(
        r#"<img src="/image.png?r1={}&g1={}&b1={}&r2={}&g2={}&b2={}"/>"#,
        c1.0, c1.1, c1.2,
        c2.0, c2.1, c2.2,
    )
}

fn question(_path: Split<char>, _params: HashMap<String, String>) -> Result<HttpOkay, HttpError> {
    Ok(HttpOkay::Html(format!(
        r#"
            <!DOCTYPE html>
            <html>
                <head>
                    <title>Click on the one that is most visible</title>
                    <link rel="stylesheet" href="/static/stylesheet.css">
                </head>
                <body>
                    <div class="box">
                        <p class="instruction">Click on the image where the text is most easily visible.</p>
                        <div class="question">
                            <div class="images">
                                {}
                                {}
                            </div>
                        </div>
                    </div>
                </body>
            </html>
        "#,
        image_element(random_centre(), random_delta()),
        image_element(random_centre(), random_delta()),
    )))
}
