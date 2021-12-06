use crate::responder::TemplateToPubResponse;
use actix_web::{get, App, HttpServer, Responder};
use askama_actix::Template;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Start HTTP server
    HttpServer::new(move || {
        App::new()
            .wrap(middleware::AppendContext {})
            .service(view_index)
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}

#[derive(Template)]
#[template(path = "table.html")]
struct TableTemplate {
    numbers: Vec<i32>,
}

#[get("/")]
pub async fn view_index() -> impl Responder {
    TableTemplate {
        numbers: (0..10).collect(),
    }
    .to_pub_response()
}

mod middleware {
    use actix_web::dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform};
    use actix_web::Error;
    use std::future::{ready, Ready};
    use std::time::Instant;

    /// Contextual information passed to the page container.
    /// Initialized in Middleware. Passed in a Handler.
    #[derive(Debug, Clone)]
    pub struct Context {
        pub request_start: Instant,
        pub secret_word: String,
    }

    impl Context {
        /// Returns human readable request time in microseconds.
        pub fn request_time(&self) -> u128 {
            (Instant::now() - self.request_start).as_micros()
        }
    }

    /// Middleware struct for appending the Context to the HttpRequest's extensions jar.
    #[derive(Debug, Clone, Copy)]
    pub struct AppendContext {}

    impl<S, B> Transform<S, ServiceRequest> for AppendContext
    where
        S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
        S::Future: 'static,
    {
        type Response = ServiceResponse<B>;
        type Error = Error;
        type Transform = AppendContextMiddleware<S>;
        type InitError = ();
        type Future = Ready<Result<Self::Transform, Self::InitError>>;

        fn new_transform(&self, service: S) -> Self::Future {
            ready(Ok(AppendContextMiddleware { service }))
        }
    }

    pub struct AppendContextMiddleware<S> {
        service: S,
    }

    impl<S, B> Service<ServiceRequest> for AppendContextMiddleware<S>
    where
        S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
        S::Future: 'static,
    {
        type Response = ServiceResponse<B>;
        type Error = Error;
        type Future = S::Future;

        forward_ready!(service);

        fn call(&self, mut req: ServiceRequest) -> Self::Future {
            // get mut HttpRequest from ServiceRequest
            let (httpreq, _payload) = req.parts_mut();

            // insert data into extensions if enabled
            httpreq.extensions_mut().insert(Context {
                request_start: Instant::now(),
                secret_word: "Popsicle".to_owned(),
            });

            self.service.call(req)
        }
    }
}

mod responder {
    use crate::middleware::Context;
    use actix_web::{error, Error, HttpRequest, HttpResponse};
    use askama_actix::{Template, TemplateToResponse};

    /// Page container to wrap public views.
    #[derive(Template)]
    #[template(path = "public.html")]
    struct PublicTemplate<'a> {
        context: &'a Context,
        content: &'a str,
    }

    pub trait TemplateToPubResponse {
        fn to_pub_response(&self) -> Result<PublicResponse, Error>;
    }

    /// Produces an actix-web HttpResponse with a partial template that will be inset with the public container.
    impl<T: askama::Template> TemplateToPubResponse for T {
        fn to_pub_response(&self) -> Result<PublicResponse, Error> {
            let mut buffer = String::new();
            if self.render_into(&mut buffer).is_err() {
                return Err(error::ErrorInternalServerError("Template parsing error"));
            }

            Ok(PublicResponse { content: buffer })
        }
    }

    /// PublicResponder wraps content from an inner template for the outer public Page Container.
    /// It implements the actix_web::Responder trait so that it can be returned as a result in actix_web functions.
    /// When returned to actix_web as the result of controller logic, it can access the HttpRequest and its extensions and pass it as context to the PublicTemplate.
    pub struct PublicResponse {
        content: String,
    }

    impl actix_web::Responder for PublicResponse {
        fn respond_to(self, req: &HttpRequest) -> HttpResponse {
            if !req.extensions().contains::<Context>() {
                return error::ErrorInternalServerError(
                    "Failed to pass context to container template",
                )
                .error_response();
            }

            PublicTemplate {
                content: &self.content,
                context: req.extensions().get::<Context>().unwrap(),
            }
            .to_response()
        }
    }
}
