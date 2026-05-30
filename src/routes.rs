pub mod auth;
pub mod files;
pub mod shares;
pub mod users;

pub use auth::routes as auth_routes;
pub use files::routes as files_routes;
pub use shares::routes as shares_routes;
pub use users::routes as users_routes;