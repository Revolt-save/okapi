#![forbid(missing_docs)]
#![forbid(unsafe_code)]
#![deny(clippy::all)]

//! This projects serves to enable automatic rendering of `openapi.json` files, and provides
//! facilities to also serve the documentation alongside your api.
//!
//! # Usage
//! First, add the following lines to your `Cargo.toml`
//! ```toml
//! [dependencies]
//! rocket = { version = "0.5.0-rc.1", default-features = false, features = ["json"] }
//! schemars = "0.8"
//! okapi = { version = "0.6.0-alpha-1" }
//! revolt_rocket_okapi = { version = "0.9.1", features = ["swagger"] }
//! ```
//! To add documentation to a set of endpoints, a couple of steps are required. The request and
//! response types of the endpoint must implement `JsonSchema`. Secondly, the function must be
//! marked with `#[openapi]`. After that, you can simply replace `routes!` with
//! `openapi_get_routes!`. This will append an additional route to the resulting `Vec<Route>`,
//! which contains the `openapi.json` file that is required by swagger. Now that we have the json
//! file that we need, we can serve the swagger web interface at another endpoint, and we should be
//! able to load the example in the browser!
//! ### Example
//! ```rust, no_run
//! use rocket::get;
//! use rocket::serde::json::Json;
//! use revolt_rocket_okapi::{openapi, openapi_get_routes, JsonSchema};
//! use revolt_rocket_okapi::swagger_ui::{make_swagger_ui, SwaggerUIConfig};
//!
//! #[derive(serde::Serialize, JsonSchema)]
//! struct Response {
//!     reply: String,
//! }
//!
//! #[openapi]
//! #[get("/")]
//! fn my_controller() -> Json<Response> {
//!     Json(Response {
//!         reply: "show me the docs!".to_string(),
//!     })
//! }
//!
//! fn get_docs() -> SwaggerUIConfig {
//!     use revolt_rocket_okapi::settings::UrlObject;
//!
//!     SwaggerUIConfig {
//!         url: "/my_resource/openapi.json".to_string(),
//!         ..Default::default()
//!     }
//! }
//!
//! fn main() {
//!     rocket::build()
//!         .mount("/my_resource", openapi_get_routes![my_controller])
//!         .mount("/swagger", make_swagger_ui(&get_docs()))
//!         .launch();
//! }
//! ```
//!
//! This crate exposes a few macros that can be used to generate and serve routes and OpenApi objects.
//! - `mount_endpoints_and_merged_docs!{...}`: Mount endpoints and mount merged OpenAPI documentation.
//! - `openapi_get_routes![...]`: To generate and add the `openapi.json` route.
//! - `openapi_get_routes_spec![...]`: To generate and return a list of routes and the openapi spec.
//! - `openapi_get_spec![...]`: To generate and return the openapi spec.
//!
//! The last 3 macros have very similar behavior, but differ in what they return.
//! Here is a list of the marcos and what they return:
//! - `openapi_get_routes![...]`: `Vec<rocket::Route>` (adds route for `openapi.json`)
//! - `openapi_get_routes_spec![...]`: `(Vec<rocket::Route>, okapi::openapi3::OpenApi)`
//! - `openapi_get_spec![...]`: `okapi::openapi3::OpenApi`
//!

mod error;

/// Contains the `Generator` struct, which you can use to manually control the way a struct is
/// represented in the documentation.
pub mod gen;
/// Contains several `Rocket` `Handler`s, which are used for serving the json files and the swagger
/// interface.
pub mod handlers;
/// Contains the functions and structs required to display the RapiDoc UI.
#[cfg(feature = "rapidoc")]
pub mod rapidoc;
/// This module contains several traits that correspond to the `Rocket` traits pertaining to request
/// guards and responses
pub mod request;
/// Contains the trait `OpenApiResponder`, meaning that a response implementing this trait can be
/// documented.
pub mod response;
/// Contains then `OpenApiSettings` struct, which can be used to customize the behavior of a
/// `Generator`.
pub mod settings;
/// Contains the functions and structs required to display the Swagger UI.
#[cfg(feature = "swagger")]
pub mod swagger_ui;
/// Assorted function that are used throughout the application.
pub mod util;

pub use error::*;
/// Re-export Okapi
pub use revolt_okapi;
pub use revolt_rocket_okapi_codegen::*;
pub use schemars::JsonSchema;

/// Contains information about an endpoint.
pub struct OperationInfo {
    /// The path of the endpoint
    pub path: String,
    /// The HTTP Method of this endpoint.
    pub method: rocket::http::Method,
    /// Contains information to be showed in the documentation about this endpoint.
    pub operation: revolt_okapi::openapi3::Operation,
}

/// Convert OpenApi object to routable endpoint.
///
/// Used to serve an `OpenApi` object as an `openapi.json` file in Rocket.
pub fn get_openapi_route(
    spec: revolt_okapi::openapi3::OpenApi,
    settings: &settings::OpenApiSettings,
) -> rocket::Route {
    handlers::OpenApiHandler::new(spec).into_route(&settings.json_path)
}

/// Mount endpoints and mount merged OpenAPI documentation.
///
/// This marco just makes to code look cleaner and improves readability
/// for bigger codebases.
///
/// The macro expects the following arguments:
/// - rocket_builder: `Rocket<Build>`,
/// - base_path: `&str`, `String` or [`Uri`](rocket::http::uri::Uri). (Anything that implements `ToString`)
/// Anything accepted by [`mount()`](https://docs.rs/rocket/0.5.0-rc.1/rocket/struct.Rocket.html#method.mount)
/// - openapi_settings: `OpenApiSettings` (use `OpenApiSettings::default()` if default settings are okay for you),
/// - List of (0 or more):
///   - path:  `&str`, `String` or [`Uri`](rocket::http::uri::Uri).
///   Anything accepted by `mount()` (`base_path` should not be included).
///   - `=>`: divider
///   - route_and_docs: `(Vec<rocket::Route>, OpenApi)`
///
/// Example:
/// ```rust,ignore
/// let custom_route_spec = (vec![], custom_spec());
/// mount_endpoints_and_merged_docs! {
///     building_rocket, "/v1".to_owned(),
///     "/" => custom_route_spec,
///     "/post" => post::get_routes_and_docs(),
///     "/message" => message::get_routes_and_docs(),
/// };
/// ```
///
#[macro_export]
macro_rules! mount_endpoints_and_merged_docs {
    ($rocket_builder:ident, $base_path:expr, $openapi_settings:ident,
     $($path:expr => $route_and_docs:expr),* $(,)*) => {{
        let base_path = $base_path.to_string();
        assert!(base_path == "/" || !base_path.ends_with("/"), "`base_path` should not end with an `/`.");
        let mut openapi_list: Vec<(_, revolt_rocket_okapi::revolt_okapi::openapi3::OpenApi)> = Vec::new();
        $({
            let (routes, openapi) = $route_and_docs;
            $rocket_builder = $rocket_builder.mount(format!("{}{}", base_path, $path), routes);
            openapi_list.push(($path, openapi));
        })*
        // Combine all OpenApi documentation into one struct.
        let openapi_docs = match revolt_rocket_okapi::revolt_okapi::merge::marge_spec_list(&openapi_list){
            Ok(docs) => docs,
            Err(err) => panic!("Could not merge OpenAPI spec: {}", err),
        };
        // Add OpenApi route
        $rocket_builder = $rocket_builder.mount(
            $base_path,
            vec![revolt_rocket_okapi::get_openapi_route(
                openapi_docs,
                &$openapi_settings,
            )],
        );
    }};
}

/// A replacement macro for `rocket::routes`. This also takes a optional settings object.
///
/// The key differences are that this macro will add an additional element to the
/// resulting `Vec<rocket::Route>`, which serves a static file called
/// `openapi.json`. This file can then be used to display the routes in the Swagger/RapiDoc UI.
///
/// Example:
/// ```rust,ignore
/// use revolt_okapi::openapi3::OpenApi;
/// let settings = revolt_rocket_okapi::settings::OpenApiSettings::new();
/// let routes: Vec<rocket::Route> =
///     openapi_get_routes![settings: create_message, get_message];
/// ```
/// Or
/// ```rust,ignore
/// use revolt_okapi::openapi3::OpenApi;
/// let routes: Vec<rocket::Route> =
///     openapi_get_routes![create_message, get_message];
/// ```
#[macro_export]
macro_rules! openapi_get_routes {
    // With settings
    ($settings:ident :
     $($route:expr),* $(,)*) => {{
        let spec = revolt_rocket_okapi::openapi_spec![$($route),*](&$settings);
        let routes = revolt_rocket_okapi::openapi_routes![$($route),*](Some(spec), &$settings);
        routes
    }};

    // Without settings
    ($($route:expr),* $(,)*) => {{
        let settings = revolt_rocket_okapi::settings::OpenApiSettings::new();
        revolt_rocket_okapi::openapi_get_routes![settings: $($route),*]
    }};
}

/// A replacement macro for `rocket::routes`. This parses the routes and provides
/// a tuple with 2 parts `(Vec<rocket::Route>, OpenApi)`:
/// - `Vec<rocket::Route>`: A list of all the routes that `rocket::routes![]` would have provided.
/// - `OpenApi`: The `okapi::openapi3::OpenApi` spec for all the routes.
///
/// NOTE: This marco is different from `openapi_get_routes` in that this does not add
/// the `openapi.json` file to the list of routes. This is done so the `OpenApi` spec can be changed
/// before serving it.
///
/// Example:
/// ```rust,ignore
/// use revolt_okapi::openapi3::OpenApi;
/// let settings = revolt_rocket_okapi::settings::OpenApiSettings::new();
/// let (routes, spec): (Vec<rocket::Route>, OpenApi) =
///     openapi_get_routes_spec![settings: create_message, get_message];
/// ```
/// Or
/// ```rust,ignore
/// use revolt_okapi::openapi3::OpenApi;
/// let (routes, spec): (Vec<rocket::Route>, OpenApi) =
///     openapi_get_routes_spec![create_message, get_message];
/// ```
#[macro_export]
macro_rules! openapi_get_routes_spec {
    // With settings
    ($settings:ident :
     $($route:expr),* $(,)*) => {{
        let spec = revolt_rocket_okapi::openapi_spec![$($route),*](&$settings);
        let routes = revolt_rocket_okapi::openapi_routes![$($route),*](None, &$settings);
        (routes, spec)
    }};

    // Without settings
    ($($route:expr),* $(,)*) => {{
        let settings = revolt_rocket_okapi::settings::OpenApiSettings::new();
        revolt_rocket_okapi::openapi_get_routes_spec![settings: $($route),*]
    }};
}

/// Generate `OpenApi` spec only, does not generate routes.
/// This can be used in cases where you are only interested in the openAPI spec, but not in the routes.
/// A use case could be inside of `build.rs` scripts or where you want to alter OpenAPI object
/// at runtime.
///
/// Example:
/// ```rust,ignore
/// use revolt_okapi::openapi3::OpenApi;
/// let settings = revolt_rocket_okapi::settings::OpenApiSettings::new();
/// let spec: OpenApi = openapi_get_spec![settings: create_message, get_message];
/// ```
/// Or
/// ```rust,ignore
/// use revolt_okapi::openapi3::OpenApi;
/// let spec: OpenApi = openapi_get_spec![create_message, get_message];
/// ```
#[macro_export]
macro_rules! openapi_get_spec {
    // With settings
    ($settings:ident :
     $($route:expr),* $(,)*) => {{
        let spec = revolt_rocket_okapi::openapi_spec![$($route),*](&$settings);
        spec
    }};

    // Without settings
    ($($route:expr),* $(,)*) => {{
        let settings = revolt_rocket_okapi::settings::OpenApiSettings::new();
        revolt_rocket_okapi::openapi_get_spec![settings: $($route),*]
    }};
}
