use std::io::Cursor;
use std::net::Ipv4Addr;

use image::{ImageFormat, Luma};
use qrcode::render::svg;
use qrcode::QrCode;
use rocket::data::ToByteUnit;
use rocket::form::Form;
use rocket::http::{Header, Method, RawStr, Status};
use rocket::route::{Handler, Outcome};
use rocket::{Config, Data, FromForm, Request, Response, Route};

enum OutputFormat {
    Png,
    Svg,
}

fn make_qrcode(content: &str, format: &OutputFormat, rounded: bool) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let code = QrCode::new(content)?;
    let corner_radius = if rounded { 0.25  } else { 0.0 };

    let mut bytes: Vec<u8> = Vec::new();

    match format {
        OutputFormat::Png => {
            let image = code.render::<Luma<u8>>().corner_radius(corner_radius).min_dimensions(1000, 1000).build();
            image.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)?;
        }
        OutputFormat::Svg => {
            bytes = code.render()
                .dark_color(svg::Color("#000000"))
                .light_color(svg::Color("#ffffff"))
                .corner_radius(corner_radius)
                .min_dimensions(1000, 1000)
                .build().into();
        }
    }

    Ok(bytes)
}

fn make_and_return_qrcode<'a>(content: &str, format: &OutputFormat, rounded: bool) -> Outcome<'a> {
    let code = make_qrcode(content, format, rounded);

    let code = match code {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error: {}", e);
            return Outcome::Error(Status::InternalServerError);
        }
    };

    let content_type = match format {
        OutputFormat::Png => "image/png",
        OutputFormat::Svg => "image/svg+xml",
    };

    Outcome::Success(
        Response::build()
            .header(Header::new("Content-Type", content_type))
            .sized_body(code.len(), Cursor::new(code))
            .finalize(),
    )
}

fn get_format_from_accept(req: &'_ Request<'_>) -> OutputFormat {
    match req.headers().get("Accept").find(|&x| x == "image/svg+xml") {
        Some(_) => OutputFormat::Svg,
        None => OutputFormat::Png,
    }
}

#[derive(FromForm, Debug)]
struct Body {
    pub input: String,
    pub rounded: bool,
}

async fn parse_post(req: &'_ Request<'_>, body: Data<'_>) -> Result<(String, bool), Status> {
    let content = match body.open(2.megabytes()).into_string().await {
        Ok(content) => content.into_inner(),
        Err(_) => return Err(Status::PayloadTooLarge),
    };

    match req.content_type() {
        Some(content_type) => {
            // Why can't I match content_type {}?
            if content_type.is_form() {
                match Form::<Body>::parse(&content) {
                    Ok(form) => {
                        Ok((RawStr::percent_decode_lossy(RawStr::new(form.input.as_str())).into(), form.rounded))
                    }
                    Err(_) => Err(Status::BadRequest),
                }
            } else if content_type.is_plain() {
                Ok((content, false))
            } else {
                Err(Status::UnsupportedMediaType)
            }
        }
        None => Err(Status::BadRequest),
    }
}

#[derive(Clone)]
struct Server;

impl Server {
    fn index() -> &'static str {
        include_str!("index.html")
    }
}

#[rocket::async_trait]
impl Handler for Server {
    async fn handle<'r>(&self, req: &'r Request<'_>, data: Data<'r>) -> Outcome<'r> {
        if req.uri().path() == "/" {
            match req.method() {
                Method::Get => {
                    let html = Server::index();
                    Outcome::Success(
                        Response::build()
                            .header(Header::new("Content-Type", "text/html; charset=utf-8"))
                            .sized_body(html.len(), Cursor::new(html))
                            .finalize(),
                    )
                }
                Method::Post => match parse_post(req, data).await {
                    Ok((content, rounded)) => make_and_return_qrcode(&content, &get_format_from_accept(req), rounded),
                    Err(_) => Outcome::Error(Status::PayloadTooLarge),
                },
                _ => Outcome::Error(Status::MethodNotAllowed),
            }
        } else {
            match req.method() {
                Method::Get => {
                    let uri = req.uri().to_string();
                    let uri = uri.strip_prefix("/").unwrap_or(&uri);
                    make_and_return_qrcode(uri, &get_format_from_accept(req), false)
                }
                _ => Outcome::Error(Status::MethodNotAllowed),
            }
        }
    }
}

impl From<Server> for Vec<Route> {
    fn from(value: Server) -> Self {
        vec![
            Route::new(Method::Get, "/<path..>", value.clone()),
            Route::new(Method::Post, "/", value.clone()),
        ]
    }
}

#[rocket::launch]
fn rocket() -> _ {
    let config = Config {
        address: Ipv4Addr::new(0, 0, 0, 0).into(),
        ..Config::debug_default()
    };
    rocket::custom(config).mount("/", Server {})
}
