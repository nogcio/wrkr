mod collector;
mod offline;
mod server;
mod templates;
mod types;

pub(crate) use collector::DashboardCollector;
pub(crate) use offline::write_offline_report;
pub(crate) use server::{DashboardServer, DashboardServerConfig};
