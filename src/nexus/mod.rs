//! Nexus Mods API integration

pub mod graphql;
pub mod populate;
pub mod rest;

pub use graphql::{
    DownloadLink, ModFile, ModRequirement, ModSearchPage, ModSearchParams, ModSearchResult,
    ModUpdateInfo, NexusClient, SortBy,
};

pub use populate::{CatalogPopulator, PopulateOptions, PopulateStats};
pub use rest::{ModInfo, NexusRestClient};
