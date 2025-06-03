use std::collections::{HashMap};
use std::error::{Error};
use std::io::{Write};
use std::fs::{File};

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
    Redirect(String),
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

// ----------------------------------------------------------------------------

/// Represent HTTP request parameters.
#[derive(Debug)]
pub struct Params(HashMap<String, String>);

impl Params {
    fn get(&self, key: &str) -> Result<&String, HttpError> {
        self.0.get(key).ok_or(HttpError::Invalid)
    }
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
    Delta(2, 2, -3), Delta(5, -5, 0),
    Delta(2, -3, 2), Delta(5, 0, -5),
    Delta(-3, 2, 2), Delta(0, 5, -5),
];

const SCALES: [i8; 10] = [-5, -4, -3, -2, -1, 1, 2, 3, 4, 5];

impl std::ops::Mul<i8> for Delta {
    type Output = Self;

    fn mul(self, rhs: i8) -> Self::Output { Self(self.0 * rhs, self.1 * rhs, self.2 * rhs) }
}

impl std::ops::Neg for Delta {
    type Output = Self;

    fn neg(self) -> Self::Output { self * -1 }
}

/// Return a random element of `DELTAS`.
fn random_delta() -> Delta {
    DELTAS[rand::random_range(0..DELTAS.len())] * SCALES[rand::random_range(0..SCALES.len())]
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
        server
    }

    /// Construct an HTTP header.
    fn header(key: &str, value: &str) -> tiny_http::Header {
        let key_b = key.as_bytes();
        let val_b = value.as_bytes();
        Header::from_bytes(
            key_b, val_b)
            .unwrap() // depends only on data fixed at compile time
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
                    let header = Self::header("Content-Type", "text/html");
                    request.respond(Response::from_string(text).with_header(header))
                },
                Ok(HttpOkay::Data(data)) => {
                    let header = Self::header("Content-Type", "image/png");
                    request.respond(Response::from_data(data).with_header(header))
                },
                Ok(HttpOkay::Static(data, content_type)) => {
                    let header = Self::header("Content-Type", content_type);
                    request.respond(Response::from_data(data).with_header(header))
                },
                Ok(HttpOkay::Redirect(relative_url)) => {
                    let header = Self::header("Location", self.base_url.join(&relative_url).unwrap().as_str());
                    request.respond(Response::from_string("Moved Permanently").with_status_code(301).with_header(header))
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

    const STYLESHEET: &[u8] = include_bytes!("stylesheet.css");
    const INTRO: &[u8] = include_bytes!("intro.html");

    /// Handle a single request.
    fn handle_request(&self, request: &Request) -> Result<HttpOkay, HttpError> {
        match request.method() {
            Method::Get => {},
            _ => return Err(HttpError::Invalid),
        }

        let url = request.url();
        let url = url_escape::decode(url).into_owned();
        let url = self.base_url.join(&url)?;
        println!("{} {}", request.remote_addr().unwrap().ip(), url);
        let params = Params(url.query_pairs().map(
            |(key, value)| (key.into_owned(), value.into_owned())
        ).collect());
        let mut path = url.path_segments().unwrap();
        match path.next() {
            None | Some("") | Some("index.html") => Ok(HttpOkay::Redirect("intro.html".into())),
            Some("stylesheet.css") => Ok(HttpOkay::Static(Self::STYLESHEET, "text/css")),
            Some("intro.html") => Ok(HttpOkay::Static(Self::INTRO, "text/html")),
            Some("image.png") => Self::image(params),
            Some("question") => Self::question(params),
            Some("start") => self.start(params),
            Some("submit") => self.submit(request.remote_addr().unwrap(), params),
            p => { println!("Not found: {:?}", p); Err(HttpError::NotFound) },
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
    pub fn image(params: Params) -> Result<HttpOkay, HttpError> {
        let bg = Self::parse_colour(params.get("bg")?)?;
        let fg = Self::parse_colour(params.get("fg")?)?;

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
    fn form_element(questionnaire: &str, which: usize, win: (Colour, Colour), lose: (Colour, Colour)) -> String {
        format!(
            r#"
                <form action="/submit">
                    <input type="hidden" name="q" value="{}">
                    <input type="hidden" name="which" value="{}">
                    <input type="hidden" name="win1" value="{}"/>
                    <input type="hidden" name="win2" value="{}"/>
                    <input type="hidden" name="lose1" value="{}"/>
                    <input type="hidden" name="lose2" value="{}"/>
                    <input type="image" src="/image.png?bg={}&fg={}"/>
                </form>
            "#,
            questionnaire,
            which,
            win.0, win.1,
            lose.0, lose.1,
            win.0, win.1,
        )
    }

    /// Returns a question comparing two images.
    pub fn question(params: Params) -> Result<HttpOkay, HttpError> {
        let questionnaire = params.get("q")?;
        let pair1 = Self::random_colour_pair();
        let pair2 = Self::random_colour_pair();
        Ok(HttpOkay::Html(format!(
            r#"
                <!DOCTYPE html>
                <html>
                    <head>
                        <title>Click on the one that is most visible</title>
                        <link rel="stylesheet" href="/stylesheet.css">
                    </head>
                    <body class="grey">
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
            Self::form_element(questionnaire, 1, pair1, pair2),
            Self::form_element(questionnaire, 2, pair2, pair1),
        )))
    }

    /// Start the experiment.
    pub fn start(&self, params: Params) -> Result<HttpOkay, HttpError> {
        let questionnaire = format!(
            "{}{}{}{}{}{}{}{}{}{}{}{}",
            params.get("age")?,
            params.get("sex")?,
            params.get("eye_colour")?,
            params.get("cvd")?,
            params.get("eyewear")?,
            params.get("surgery")?,
            params.get("where")?,
            params.get("light")?,
            params.get("company")?,
            params.get("device")?,
            params.get("screen")?,
            params.get("monochrome")?,
        );
        Ok(HttpOkay::Redirect(format!("/question?q={}", questionnaire)))
    }

    /// Log the answer to a `question()`.
    pub fn submit(&self, remote_addr: &std::net::SocketAddr, params: Params) -> Result<HttpOkay, HttpError> {
        let questionnaire = params.get("q")?;
        let which = params.get("which")?.parse::<u8>()?;
        let is_first = which == 1;
        let win1 = Self::parse_colour(params.get("win1")?)?;
        let win2 = Self::parse_colour(params.get("win2")?)?;
        let lose1 = Self::parse_colour(params.get("lose1")?)?;
        let lose2 = Self::parse_colour(params.get("lose2")?)?;
        writeln!(&self.results, "{}, {}, {}, {}, {}, {}, {}, {}",
            remote_addr.ip(),
            chrono::Utc::now(),
            questionnaire,
            is_first,
            win1, win2,
            lose1, lose2,
        )?;
        Ok(HttpOkay::Redirect(format!("/question?q={}", questionnaire)))
    }
}

// ----------------------------------------------------------------------------

/// The path where the experimental results are written.
const RESULT_FILENAME: &'static str = "/tmp/ocularity-results.log";

/// The server address and port to listen on.
const SERVER_ADDRESS: &'static str = "127.0.0.1:8081";

fn main() {
    let server_url = format!("http://{}", SERVER_ADDRESS);
    let base_url = std::env::var("OCULARITY_BASE_URL").unwrap_or_else(|_| server_url.clone());
    let server = Ocularity::new(SERVER_ADDRESS, &base_url, RESULT_FILENAME);
    println!("Listening on {}", server_url);
    server.handle_requests();
}
