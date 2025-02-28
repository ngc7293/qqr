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
    PNG,
    SVG,
}

fn make_qrcode(content: &str, format: &OutputFormat) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let code = QrCode::new(content)?;

    let mut bytes: Vec<u8> = Vec::new();

    match format {
        OutputFormat::PNG => {
            let image = code.render::<Luma<u8>>().min_dimensions(1000, 1000).build();
            image.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)?;
        }
        OutputFormat::SVG => {
            bytes = code.render()
                .dark_color(svg::Color("#000000"))
                .light_color(svg::Color("#ffffff"))
                .min_dimensions(1000, 1000)
                .build().into();
        }
    }

    Ok(bytes)
}

fn make_and_return_qrcode<'a>(content: &str, format: &OutputFormat) -> Outcome<'a> {
    let code = make_qrcode(content, format);

    let code = match code {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error: {}", e);
            return Outcome::Error(Status::InternalServerError);
        }
    };

    let content_type = match format {
        OutputFormat::PNG => "image/png",
        OutputFormat::SVG => "image/svg+xml",
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
        Some(_) => OutputFormat::SVG,
        None => OutputFormat::PNG,
    }
}

#[derive(FromForm)]
struct Body {
    pub input: String,
}

async fn parse_post(req: &'_ Request<'_>, body: Data<'_>) -> Result<String, Status> {
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
                        Ok(RawStr::percent_decode_lossy(RawStr::new(form.input.as_str())).into())
                    }
                    Err(_) => Err(Status::BadRequest),
                }
            } else if content_type.is_plain() {
                Ok(content)
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
        r#"<!DOCTYPE html><html><head><title>qqr</title></head><body><form style="display:flex;flex-direction:column;flex;align-items:center;justify-content:center;width:100vw;height:100vh;row-gap:0.25em;" action="" method="post"><textarea name="input" style="width:50vw"></textarea><input type="submit" style="width: 25vw"/></form></body></html>"#
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
                    Ok(content) => make_and_return_qrcode(&content, &get_format_from_accept(req)),
                    Err(_) => Outcome::Error(Status::PayloadTooLarge),
                },
                _ => Outcome::Error(Status::MethodNotAllowed),
            }
        } else {
            match req.method() {
                Method::Get => {
                    let uri = req.uri().to_string();
                    let uri = uri.strip_prefix("/").unwrap_or(&uri);
                    make_and_return_qrcode(uri, &get_format_from_accept(req))
                }
                _ => Outcome::Error(Status::MethodNotAllowed),
            }
        }
    }
}

impl Into<Vec<Route>> for Server {
    fn into(self) -> Vec<Route> {
        vec![
            Route::new(Method::Get, "/<path..>", self.clone()),
            Route::new(Method::Post, "/", self.clone()),
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
