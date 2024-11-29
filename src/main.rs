use std::io::Cursor;
use std::net::Ipv4Addr;

use image::{ImageFormat, Luma};
use qrcode::QrCode;
use rocket::http::{Header, Method, Status};
use rocket::route::{Handler, Outcome};
use rocket::{Config, Data, Request, Response, Route};


fn make_qrcode(slug: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let code = QrCode::new(slug)?;
    let image = code.render::<Luma<u8>>().min_dimensions(1000, 1000).build();

    let mut bytes: Vec<u8> = Vec::new();
    image.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)?;

    Ok(bytes)
}


#[derive(Clone)]
struct Server;

impl Server {
    fn index() -> &'static str {
        r#"<!DOCTYPE html><html><head><title>qqr</title></head><body style="display:flex;flex-direction:column;flex;align-items:center;justify-content:center;width:100vw;height:100vh;row-gap:0.25em;"><input id="input" style="width:50vw"/><button onclick="console.log(document.getElementById('input').value);window.location.href='https://davidbourgault.ca/qr/'+encodeURIComponent(document.getElementById('input').value)" style="width: 25vw">Generate</button></body></html>"#
    }
}

#[rocket::async_trait]
impl Handler for Server {
    async fn handle<'r>(&self, req: &'r Request<'_>, _: Data<'r>) -> Outcome<'r> {
        if req.uri().path() == "/" {
            let html = Server::index();
            return Outcome::Success(Response::build()
                .header(Header::new("Content-Type", "text/html; charset=utf-8"))
                .sized_body(html.len(), Cursor::new(html))
                .finalize()
            )
        }

        let uri = req.uri().to_string();
        let uri = uri.strip_prefix("/").unwrap_or(&uri);
        let code = make_qrcode(uri);

        let code = match code {
            Ok(code) => code,
            Err(e) => {
                eprintln!("Error: {}", e);
                return Outcome::Error(Status::InternalServerError);
            }
        };

        Outcome::Success(Response::build()
            .header(Header::new("Content-Type", "image/png"))
            .sized_body(code.len(), Cursor::new(code))
            .finalize()
        )
    }
}

impl Into<Vec<Route>> for Server {
    fn into(self) -> Vec<Route> {
        vec![
            Route::new(Method::Get, "/<path..>", self)
        ]
    }
}

#[rocket::launch]
fn rocket() -> _ {
    let config = Config {
        address: Ipv4Addr::new(0, 0, 0, 0).into(),
        ..Config::debug_default()
    };
    rocket::custom(config).mount("/", Server{})
}
