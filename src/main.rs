use std::collections::{HashMap};
use std::error::{Error};
use std::io::{Write};
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

/// An sRGB colour.
#[derive(Debug, Copy, Clone)]
struct Colour(u8, u8, u8);

impl std::fmt::Display for Colour {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{},{},{}", self.0, self.1, self.2)
    }
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
    type Output = Colour;

    fn add(self, rhs: Delta) -> Self::Output {
        Colour(
            ((self.0 as i32) + (rhs.0 as i32)) as u8,
            ((self.1 as i32) + (rhs.1 as i32)) as u8,
            ((self.2 as i32) + (rhs.2 as i32)) as u8,
        )
    }
}

impl std::ops::Sub<Delta> for Centre {
    type Output = Colour;

    fn sub(self, rhs: Delta) -> Self::Output { self + -rhs }
}

// ----------------------------------------------------------------------------

struct Ocularity {
    /// Web server.
    pub server: tiny_http::Server,

    /// The external URL of the server.
    pub base_url: Url,

    /// Results file for experimental results.
    pub results: File,
}

impl Ocularity {
    fn new(addr: &str, base_url: &str, results_filename: &str) -> Self {
        let server = Self {
            server: tiny_http::Server::http(addr)
                .expect("Could not create the web server"),
            base_url: url::Url::parse(base_url)
                .expect("Could not parse the base URL"),
            results: std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(results_filename)
                .expect("Could not open the results file"),
        };
        println!("Listening on http://{}", server.server.server_addr());
        server
    }

    /// Handle requests for ever.
    fn handle_requests(&self) {
        for request in self.server.incoming_requests() {
            match self.handle_request(&request) {
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
    }

    /// Handle a single request.
    fn handle_request(&self, request: &Request) -> Result<HttpOkay, HttpError> {
        match request.method() {
            Method::Get => {},
            _ => return Err(HttpError::Invalid),
        }

        let url = request.url();
        let url = url_escape::decode(url).into_owned();
        let url = self.base_url.join(&url)?;
        println!("{:?}", url);
        let params: HashMap<String, String> = url.query_pairs().map(
            |(key, value)| (key.into_owned(), value.into_owned())
        ).collect();
        println!("{:?}", params);
        let mut path = url.path_segments().unwrap();
        match path.next() {
            Some("static") => Self::static_file(path, params),
            Some("image.png") => Self::image(path, params),
            Some("question") => Self::question(path, params),
            Some("submit") => self.submit(request.remote_addr().unwrap(), params),
            _ => Err(HttpError::NotFound),
        }
    }


    const STYLESHEET: &[u8] = include_bytes!("stylesheet.css");

    /// Serve a static file.
    fn static_file(mut path: Split<char>, _params: HashMap<String, String>) -> Result<HttpOkay, HttpError> {
        match path.next() {
            Some("stylesheet.css") => Ok(HttpOkay::Static(Self::STYLESHEET, "text/css")),
            _ => Err(HttpError::Invalid),
        }
    }

    /// Parses a URL parameter representing an RGB colour.
    fn parse_colour(input: &str) -> Result<Colour, HttpError> {
        let mut input = input.split(',');
        let r = input.next().ok_or(HttpError::Invalid)?.parse::<u8>()?;
        let g = input.next().ok_or(HttpError::Invalid)?.parse::<u8>()?;
        let b = input.next().ok_or(HttpError::Invalid)?.parse::<u8>()?;
        if let Some(_) = input.next() { Err(HttpError::Invalid)? }
        Ok(Colour(r, g, b))
    }

    /// The test pattern (black-and-white version).
    const TEST_PATTERN: &[u8] = include_bytes!("test-pattern-grey.png");

    /// Serve an image file.
    pub fn image(_path: Split<char>, params: HashMap<String, String>) -> Result<HttpOkay, HttpError> {
        let bg = Self::parse_colour(params.get("bg").ok_or(HttpError::Invalid)?)?;
        let fg = Self::parse_colour(params.get("fg").ok_or(HttpError::Invalid)?)?;

        // Construct the palette.
        let mut palette = Vec::new();
        for i in 0..256 {
            let f = (i as f32) / 255.0;
            palette.push(((bg.0 as f32) + f * ((fg.0 as f32) - (bg.0 as f32))) as u8);
            palette.push(((bg.1 as f32) + f * ((fg.1 as f32) - (bg.1 as f32))) as u8);
            palette.push(((bg.2 as f32) + f * ((fg.2 as f32) - (bg.2 as f32))) as u8);
        }

        // Read the input image.
        let decoder = png::Decoder::new(Self::TEST_PATTERN);
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

    /// Generates two similar colours at random.
    fn random_colour_pair() -> (Colour, Colour) {
        let centre = random_centre();
        let delta = random_delta();
        (centre + delta, centre - delta)
    }

    /// Construct a `<form>` element containing an `<input type="image">`.
    ///
    /// - which - `1` for the first image and `2` for the second.
    /// - win1 - the background colour for this image.
    /// - win2 - the foreground colour for this image.
    /// - lose1 - the background colour for the other image.
    /// - lose2 - the foreground colour for the other image.
    fn form_element(which: usize, win: (Colour, Colour), lose: (Colour, Colour)) -> String {
        format!(
            r#"
                                    <form action="/submit">
                                        <input type="hidden" name="which" value="{}">
                                        <input type="hidden" name="win1" value="{}"/>
                                        <input type="hidden" name="win2" value="{}"/>
                                        <input type="hidden" name="lose1" value="{}"/>
                                        <input type="hidden" name="lose2" value="{}"/>
                                        <input type="image" src="/image.png?bg={}&fg={}"/>
                                    </form>
            "#,
            which,
            win.0, win.1,
            lose.0, lose.1,
            win.0, win.1,
        )
    }

    /// Returns a question comparing two images.
    pub fn question(_path: Split<char>, _params: HashMap<String, String>) -> Result<HttpOkay, HttpError> {
        let pair1 = Self::random_colour_pair();
        let pair2 = Self::random_colour_pair();
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
            Self::form_element(1, pair1, pair2),
            Self::form_element(2, pair2, pair1),
        )))
    }

    /// Log the answer to a `question()`.
    pub fn submit(
        &self,
        remote_addr: &std::net::SocketAddr,
        params: HashMap<String, String>,
    ) -> Result<HttpOkay, HttpError> {
        let which = params.get("which").ok_or(HttpError::Invalid)?.parse::<u8>()?;
        let is_first = which == 1;
        let win1 = Self::parse_colour(params.get("win1").ok_or(HttpError::Invalid)?)?;
        let win2 = Self::parse_colour(params.get("win2").ok_or(HttpError::Invalid)?)?;
        let lose1 = Self::parse_colour(params.get("lose1").ok_or(HttpError::Invalid)?)?;
        let lose2 = Self::parse_colour(params.get("lose2").ok_or(HttpError::Invalid)?)?;
        writeln!(&self.results, "{}, {}, {}, {}, {}, {}, {}",
            remote_addr.ip(),
            chrono::Utc::now(),
            which,
            win1, win2,
            lose1, lose2,
        )?;
        Ok(HttpOkay::Text(format!(
            "is_first={:?}, win1={:?}, win2={:?}, lose1={:?}, lose2={:?}",
            is_first,
            win1, win2,
            lose1, lose2,
        )))
    }
}

// ----------------------------------------------------------------------------

/// The path where the experimental results are written.
const RESULT_FILENAME: &'static str = "/tmp/ocularity-results.log";

/// The externally visible URL of the server.
const BASE_URL: &'static str = "https://www.minworks.co.uk/ocularity";

fn main() {
    let server = Ocularity::new("127.0.0.1:8081", BASE_URL, RESULT_FILENAME);
    server.handle_requests();
}
